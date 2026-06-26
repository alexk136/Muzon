// SPDX-License-Identifier: MIT OR Apache-2.0
//! `muzon` CLI binary.
//!
//! v0.1.0 subcommands:
//! - `muzon play <FILE>` — play a single audio file via GStreamer
//! - `muzon --print-paths` — print the resolved XDG paths
//! - `muzon library scan <PATH>` — scan a directory into the
//!   library database
//!
//! Global flags (apply to every subcommand):
//! - `--log-level <LEVEL>` — set the env filter
//! - `--config <PATH>` — override the config file path
//! - `--format json|human` — output format (json | human); v0.1.0
//!   accepts the flag and applies it where it has a defined effect
//!
//! The full surface (`muzon status`, `muzon queue add`, `muzon
//! playlist create`, `muzon skin install`, etc.) lands in v0.2.0+.

mod ctl;
mod headless;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use muzon_audio::AudioEngine;
use muzon_core::{init_logging, MuzonConfig, MuzonPaths};
use muzon_library::Scanner;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

#[derive(Debug, Parser)]
#[command(name = "muzon", about = "Muzon audio player", version)]
struct Cli {
    /// Log filter, e.g. `info`, `debug`, `muzon_audio=trace`. Overrides
    /// the `logging.level` config value and the `RUST_LOG` env.
    #[arg(long, global = true)]
    log_level: Option<String>,

    /// Override the config file path. Default: XDG config dir.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,

    /// Print the resolved XDG paths as `key=value` lines and exit.
    #[arg(long)]
    print_paths: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Play a single audio file end-to-end.
    Play {
        /// Path to the audio file.
        file: PathBuf,
        /// Optional max duration in seconds (for integration tests).
        #[arg(long)]
        max_duration: Option<u64>,
    },
    /// Library subcommands.
    Library {
        #[command(subcommand)]
        action: LibraryAction,
    },
    /// Run as a headless daemon. Exposes the IPC socket at the
    /// default path (or $MUZON_SOCKET_PATH); `muzonctl` and
    /// the Tauri shell attach to it. Stops on SIGINT/SIGTERM.
    Headless,
    /// Control a running headless core. Forwards each
    /// subcommand to the core over the Unix socket.
    #[command(disable_help_flag = false)]
    Ctl {
        /// Override the IPC socket path (default: $XDG_RUNTIME_DIR/muzon.sock).
        #[arg(long)]
        socket: Option<PathBuf>,
        #[command(subcommand)]
        command: ctl::CtlCommand,
    },
}

#[derive(Debug, Subcommand)]
enum LibraryAction {
    /// Scan a directory into the library database.
    Scan {
        /// Root directory to scan.
        path: PathBuf,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    // Paths first; we need them to find the config and the log file.
    let paths = match MuzonPaths::resolve() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("muzon: failed to resolve XDG paths: {err}");
            return ExitCode::from(1);
        }
    };

    if cli.print_paths {
        print_paths(&paths);
        return ExitCode::SUCCESS;
    }

    // Config: CLI flag > default path.
    let cfg_path = cli.config.clone().unwrap_or_else(|| paths.config_file());
    let mut cfg = match MuzonConfig::load(&cfg_path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("muzon: failed to load config from {}: {err}", cfg_path.display());
            return ExitCode::from(1);
        }
    };
    if let Some(level) = &cli.log_level {
        cfg.logging.level = level.clone();
    }

    // Logging.
    let _guard = match init_logging(&paths, &cfg.logging) {
        Ok(g) => g,
        Err(err) => {
            eprintln!("muzon: failed to init logging: {err}");
            return ExitCode::from(1);
        }
    };

    info!(
        "muzon: v0.1.0 starting (config={}, log={}, format={:?})",
        cfg_path.display(),
        paths.log_file().display(),
        cli.format
    );

    match cli.command {
        None => {
            eprintln!("muzon: no subcommand provided. Try `muzon --help`.");
            ExitCode::from(2)
        }
        Some(Command::Play { file, max_duration }) => {
            run_play(&file, max_duration, &paths, &cfg)
        }
        Some(Command::Library { action }) => match action {
            LibraryAction::Scan { path } => run_library_scan(&path, &paths, &cfg).await,
        },
        Some(Command::Headless) => {
            drop(_guard);
            headless::run()
        }
        Some(Command::Ctl { socket, command }) => {
            drop(_guard);
            ctl::run(socket, command)
        }
    }
}

fn print_paths(paths: &MuzonPaths) {
    println!("config_dir={}", paths.config_dir.display());
    println!("data_dir={}", paths.data_dir.display());
    println!("cache_dir={}", paths.cache_dir.display());
    println!("log_dir={}", paths.log_dir.display());
    println!("config_file={}", paths.config_file().display());
    println!("log_file={}", paths.log_file().display());
}

fn run_play(
    file: &std::path::Path,
    max_duration: Option<u64>,
    _paths: &MuzonPaths,
    cfg: &MuzonConfig,
) -> ExitCode {
    info!(
        "muzon play: file={}, bit_perfect={}, gapless={}, replaygain={:?}, crossfade={}s",
        file.display(),
        cfg.audio.bit_perfect,
        cfg.audio.gapless,
        cfg.audio.replaygain,
        cfg.audio.crossfade_seconds
    );
    if !cfg.audio.bit_perfect {
        info!(
            "muzon play: bit-perfect is OFF; the default playbin3 path will resample to \
             the sink rate. Enable `audio.bit_perfect = true` in config.toml for bit-perfect \
             output (added in v0.2.0+)."
        );
    }

    let engine = match AudioEngine::new() {
        Ok(e) => e,
        Err(err) => {
            error!("muzon: failed to build audio engine: {err}");
            return ExitCode::from(5);
        }
    };

    // Start a Ctrl-C watcher in a background thread; on signal,
    // call engine.stop() and exit 130.
    let engine_for_ctrl_c = engine.clone();
    std::thread::spawn(move || {
        if let Ok(()) = wait_for_ctrl_c() {
            info!("muzon: Ctrl-C received; stopping engine");
            let _ = engine_for_ctrl_c.stop();
            std::process::exit(130);
        }
    });

    if let Some(secs) = max_duration {
        let engine_for_timer = engine.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            info!("muzon: --max-duration {secs}s reached; stopping engine");
            let _ = engine_for_timer.stop();
        });
    }

    match engine.play(file) {
        Ok(()) => {
            info!("muzon: playback finished");
            ExitCode::SUCCESS
        }
        Err(muzon_audio::AudioError::MissingFile(_)) => {
            error!("muzon: file not found: {}", file.display());
            ExitCode::from(3)
        }
        Err(err) => {
            error!("muzon: playback failed: {err}");
            ExitCode::from(5)
        }
    }
}

async fn run_library_scan(
    path: &std::path::Path,
    paths: &MuzonPaths,
    _cfg: &MuzonConfig,
) -> ExitCode {
    info!("muzon library scan: root={}", path.display());
    if !path.exists() {
        error!("muzon: scan path does not exist: {}", path.display());
        return ExitCode::from(3);
    }
    let lib = match muzon_library::Library::open(paths).await {
        Ok(l) => l,
        Err(err) => {
            error!("muzon: failed to open library: {err}");
            return ExitCode::from(5);
        }
    };
    let scanner = Scanner::new(lib, muzon_core::LibraryConfig::default());
    let cancel = CancellationToken::new();
    let total = match scanner.scan_all(path, cancel).await {
        Ok(n) => n,
        Err(err) => {
            error!("muzon: scan failed: {err}");
            return ExitCode::from(5);
        }
    };
    info!("muzon library scan: {total} tracks written");
    ExitCode::SUCCESS
}

fn wait_for_ctrl_c() -> std::io::Result<()> {
    // Use the simple blocking approach: a self-pipe + signal-hook
    // would be the production path, but for v0.1.0 a single-thread
    // listener is enough.
    let mut signals =
        signal_hook::iterator::Signals::new([signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM])?;
    if signals.forever().next().is_some() {
        Ok(())
    } else {
        warn!("muzon: signal iterator returned no signals; treating as no-op");
        Ok(())
    }
}
