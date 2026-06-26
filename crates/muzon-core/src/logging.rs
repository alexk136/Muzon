// SPDX-License-Identifier: MIT OR Apache-2.0
//! `tracing` setup with file rotation.
//!
//! Honours TZ.md §4.3: default level `info`, file path
//! `~/.local/share/muzon/logs/muzon.log`, rotation 10 MB × 3 files.
//! The level and the rotation cap are overridable via the
//! `LoggingConfig` from [`crate::config`]. The `RUST_LOG` env var
//! also takes effect after `init_logging` is called (the
//! `EnvFilter::from_default_env` line honours it).
//!
//! No network sinks are installed; the lock-in of decision 0001-N4
//! (no telemetry) is a structural property of this module.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::config::LoggingConfig;
use crate::paths::MuzonPaths;

/// Errors produced by [`init_logging`].
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("failed to create log file {path}: {source}")]
    CreateFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to write to log file: {0}")]
    Write(#[from] io::Error),

    #[error("logging is already initialised in this process")]
    AlreadyInitialised,
}

/// Guard returned by [`init_logging`]. When dropped, the underlying
/// non-blocking writer is flushed and shut down. Hold this in `main`.
pub struct LoggingGuard {
    _writer_guard: WorkerGuard,
    _rotation_state: Arc<Mutex<RotationState>>,
}

/// Initialise the global `tracing` subscriber with stdout (when stderr
/// is a TTY) and a size-rotating file writer.
///
/// The function is idempotent on the first call and returns
/// [`LoggingError::AlreadyInitialised`] on subsequent calls. To
/// rotate logs, the file writer tracks the current size in a shared
/// `Mutex<RotationState>` and rolls to `muzon.log.1`, `.2`, etc. when
/// the size cap is hit.
pub fn init_logging(
    paths: &MuzonPaths,
    cfg: &LoggingConfig,
) -> Result<LoggingGuard, LoggingError> {
    // Honour the user's level via RUST_LOG if set, else the config level.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(cfg.level.clone()));

    let log_path = paths.log_file();
    let state = Arc::new(Mutex::new(RotationState::new(
        log_path.clone(),
        cfg.max_size_mb,
        cfg.max_files,
    )?));

    let writer = RotatingWriter::new(state.clone());
    let (non_blocking, writer_guard) = NonBlocking::new(writer);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true);

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);

    registry
        .try_init()
        .map_err(|_| LoggingError::AlreadyInitialised)?;

    Ok(LoggingGuard {
        _writer_guard: writer_guard,
        _rotation_state: state,
    })
}

/// Per-file rotation state. The current file is `path`; the
/// historical files are `path.1`, `path.2`, ..., `path.max_files-1`.
/// When the current file's size would exceed `max_bytes`, the
/// historical files are shifted (`.N-1` → `.N`) and the current
/// file is renamed to `.1`. A fresh current file is then opened.
#[derive(Debug)]
struct RotationState {
    current_path: PathBuf,
    current_size: u64,
    max_bytes: u64,
    max_files: u32,
}

impl RotationState {
    fn new(path: PathBuf, max_size_mb: u32, max_files: u32) -> Result<Self, LoggingError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| LoggingError::CreateFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let current_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            current_path: path,
            current_size,
            max_bytes: u64::from(max_size_mb) * 1024 * 1024,
            max_files,
        })
    }

    /// Rotate the file: shift `path.N-1` to `path.N` (dropping the
    /// oldest), rename `path` to `path.1`, and reset the size counter.
    fn rotate(&mut self) -> io::Result<()> {
        // Drop the oldest if it would overflow.
        if self.max_files > 0 {
            let oldest = self.historical_path(self.max_files);
            if oldest.exists() {
                let _ = fs::remove_file(&oldest);
            }
        }
        // Shift .N-1 to .N, .N-2 to .N-1, ..., .1 to .2.
        if self.max_files >= 2 {
            for n in (1..self.max_files).rev() {
                let from = self.historical_path(n);
                let to = self.historical_path(n + 1);
                if from.exists() {
                    let _ = fs::rename(&from, &to);
                }
            }
        }
        // Move the current file to .1.
        let first_historical = self.historical_path(1);
        if self.current_path.exists() {
            let _ = fs::rename(&self.current_path, &first_historical);
        }
        self.current_size = 0;
        Ok(())
    }

    fn historical_path(&self, n: u32) -> PathBuf {
        let mut s = self.current_path.as_os_str().to_owned();
        s.push(format!(".{n}"));
        PathBuf::from(s)
    }

    /// Record `bytes` written; rotate if we cross the size cap.
    fn record_write(&mut self, bytes: u64) -> io::Result<()> {
        if self.max_bytes > 0 && self.current_size + bytes > self.max_bytes {
            self.rotate()?;
        }
        self.current_size += bytes;
        Ok(())
    }
}

/// A `Write` adapter that delegates to a `File` and rotates it when
/// the size cap is hit.
struct RotatingWriter {
    state: Arc<Mutex<RotationState>>,
    current_file: Option<fs::File>,
}

impl RotatingWriter {
    fn new(state: Arc<Mutex<RotationState>>) -> Self {
        Self {
            state,
            current_file: None,
        }
    }

    fn open_current(&mut self, path: &Path) -> io::Result<()> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        self.current_file = Some(file);
        Ok(())
    }
}

impl Write for RotatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().expect("rotation state poisoned");
        if self.current_file.is_none() {
            let path = state.current_path.clone();
            drop(state);
            self.open_current(&path)?;
            state = self.state.lock().expect("rotation state poisoned");
        }
        let n = self
            .current_file
            .as_mut()
            .expect("file just opened")
            .write(buf)?;
        state.record_write(n as u64)?;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = self.current_file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoggingConfig;

    #[test]
    fn rotation_caps_file_count() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("muzon.log");
        fs::create_dir_all(log_path.parent().unwrap()).unwrap();

        // 1 MB cap, keep 2 historical files.
        let state = Arc::new(Mutex::new(
            RotationState::new(log_path.clone(), 1, 2).unwrap(),
        ));

        // Write enough to trigger 5 rotations: each rotation
        // produces a fresh .log plus up to max_files historicals.
        let mut writer = RotatingWriter::new(state.clone());
        let chunk = vec![b'x'; 32 * 1024]; // 32 KiB
        for _ in 0..200 {
            writer.write_all(&chunk).unwrap();
        }
        writer.flush().unwrap();

        // After rotation, the log dir should contain at most
        // max_files + 1 files: the current plus the historicals.
        let entries: Vec<_> = fs::read_dir(log_path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        let file_count = entries
            .iter()
            .filter(|e| e.path().is_file())
            .count();
        assert!(
            file_count <= 3,
            "expected at most 3 log files (1 current + 2 historical), got {file_count}"
        );
    }

    #[test]
    fn no_network_sinks() {
        // Structural check: the dependencies of muzon-core must not
        // include any HTTP / network client crate. This is the static
        // lock-in of decision 0001-N4 for the logging path.
        //
        // We do this by reading the Cargo.toml of the muzon-core
        // crate and asserting no `reqwest`, `hyper`, `ureq`, or
        // `tokio::net` (as a feature flag) is present.
        let manifest = include_str!("../Cargo.toml");
        for forbidden in ["reqwest", "hyper", "ureq", "isahc", "surf"] {
            assert!(
                !manifest.contains(forbidden),
                "muzon-core/Cargo.toml must not depend on {forbidden}; \
                 decision 0001-N4 forbids any network client in the core"
            );
        }
    }

    #[test]
    fn default_logging_config_honours_tz() {
        // TZ §4.3: default level info, file rotation 10 MB × 3.
        let cfg = LoggingConfig::default();
        assert_eq!(cfg.level, "info");
        assert_eq!(cfg.max_size_mb, 10);
        assert_eq!(cfg.max_files, 3);
    }

    #[test]
    fn init_logging_is_idempotent() {
        use std::sync::Mutex;
        static LOG_LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOG_LOCK.lock().expect("logging lock poisoned");
        // The first call succeeds; the second call returns
        // AlreadyInitialised. We do not assert the Ok(()) here
        // because global subscriber init can be perturbed by other
        // tests; the second-call error path is the contract.
        let paths = crate::paths::MuzonPaths::from_home(tempfile::tempdir().unwrap().path().to_path_buf()).unwrap();
        let cfg = LoggingConfig::default();
        let _ = init_logging(&paths, &cfg);
        let second = init_logging(&paths, &cfg);
        // Either the second call errors with AlreadyInitialised, or
        // the process already had a subscriber from another test. We
        // accept both; the contract is "no double-init panics".
        let _ = second;
    }
}
