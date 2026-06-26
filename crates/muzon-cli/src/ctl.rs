// SPDX-License-Identifier: MIT OR Apache-2.0
//! `muzonctl` — CLI client for the headless core.
//!
//! Subcommand tree that talks to a running `muzon --headless`
//! over the Unix socket. If no core is running, exits with a
//! clear error message and a non-zero exit code.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;
use muzon_ipc::{
    default_socket_path, IpcClient, IpcRequest, IpcResponse, LibraryRequest, PlaybackRequest,
    QueueRequest, RepeatMode, SkinRequest,
};

#[derive(Debug, Subcommand)]
pub enum CtlCommand {
    /// Liveness probe.
    Ping,
    /// Print the current playback status.
    Status,
    /// Start playback of a single file.
    Play { path: String },
    /// Pause the current track.
    Pause,
    /// Resume the current track.
    Resume,
    /// Stop playback.
    Stop,
    /// Seek to a position in milliseconds.
    Seek { ms: u64 },
    /// Set the output volume in [0, 100].
    Volume { percent: u32 },
    /// Append a track to the queue.
    #[command(name = "queue-add")]
    QueueAdd { path: String },
    /// Print the queue.
    #[command(name = "queue-list")]
    QueueList,
    /// Clear the queue.
    #[command(name = "queue-clear")]
    QueueClear,
    /// Set the repeat mode (off|one|all).
    #[command(name = "repeat")]
    Repeat { mode: String },
    /// Toggle shuffle on or off.
    Shuffle { on: bool },
    /// List the installed skins.
    #[command(name = "list-skins")]
    ListSkins,
    /// Set the active skin by id.
    #[command(name = "set-skin")]
    SetSkin { id: String },
    /// List the tracks in the library.
    #[command(name = "list-tracks")]
    ListTracks,
}

pub fn run(socket: Option<PathBuf>, cmd: CtlCommand) -> ExitCode {
    let socket = socket.unwrap_or_else(default_socket_path);
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("muzonctl: failed to build tokio runtime: {err}");
            return ExitCode::from(5);
        }
    };

    runtime.block_on(async move {
        let mut client = match IpcClient::connect(&socket).await {
            Ok(c) => c,
            Err(err) => {
                eprintln!(
                    "muzonctl: no muzon-core running at {}; start one with `muzon --headless` (error: {err})",
                    socket.display()
                );
                return ExitCode::from(3);
            }
        };
        match cmd {
            CtlCommand::Ping => {
                let resp = client.call(IpcRequest::Ping { nonce: 1 }).await;
                print_response(resp, "ping");
            }
            CtlCommand::Status => {
                let resp = client.call(IpcRequest::Playback(PlaybackRequest::Status)).await;
                print_response(resp, "status");
            }
            CtlCommand::Play { path } => {
                let resp = client
                    .call(IpcRequest::Playback(PlaybackRequest::Play { path }))
                    .await;
                print_response(resp, "play");
            }
            CtlCommand::Pause => {
                let resp = client.call(IpcRequest::Playback(PlaybackRequest::Pause)).await;
                print_response(resp, "pause");
            }
            CtlCommand::Resume => {
                let resp = client.call(IpcRequest::Playback(PlaybackRequest::Resume)).await;
                print_response(resp, "resume");
            }
            CtlCommand::Stop => {
                let resp = client.call(IpcRequest::Playback(PlaybackRequest::Stop)).await;
                print_response(resp, "stop");
            }
            CtlCommand::Seek { ms } => {
                let resp = client
                    .call(IpcRequest::Playback(PlaybackRequest::Seek { position_ms: ms }))
                    .await;
                print_response(resp, "seek");
            }
            CtlCommand::Volume { percent } => {
                let v = (percent as f32 / 100.0).clamp(0.0, 1.0);
                let resp = client
                    .call(IpcRequest::Playback(PlaybackRequest::SetVolume { volume: v }))
                    .await;
                print_response(resp, "volume");
            }
            CtlCommand::QueueAdd { path } => {
                let resp = client
                    .call(IpcRequest::Queue(QueueRequest::Enqueue { path }))
                    .await;
                print_response(resp, "queue-add");
            }
            CtlCommand::QueueList => {
                let resp = client.call(IpcRequest::Queue(QueueRequest::Snapshot)).await;
                print_response(resp, "queue-list");
            }
            CtlCommand::QueueClear => {
                let resp = client.call(IpcRequest::Queue(QueueRequest::Clear)).await;
                print_response(resp, "queue-clear");
            }
            CtlCommand::Repeat { mode } => {
                let parsed = match mode.as_str() {
                    "off" => RepeatMode::Off,
                    "one" => RepeatMode::One,
                    "all" => RepeatMode::All,
                    other => {
                        eprintln!("muzonctl: invalid repeat mode '{other}' (expected off|one|all)");
                        return ExitCode::from(2);
                    }
                };
                let resp = client
                    .call(IpcRequest::Queue(QueueRequest::SetRepeatMode { mode: parsed }))
                    .await;
                print_response(resp, "repeat");
            }
            CtlCommand::Shuffle { on } => {
                let resp = client.call(IpcRequest::Queue(QueueRequest::SetShuffle { on })).await;
                print_response(resp, "shuffle");
            }
            CtlCommand::ListSkins => {
                let resp = client.call(IpcRequest::Skin(SkinRequest::ListInstalled)).await;
                print_response(resp, "list-skins");
            }
            CtlCommand::SetSkin { id } => {
                let resp = client
                    .call(IpcRequest::Skin(SkinRequest::SetActive { id }))
                    .await;
                print_response(resp, "set-skin");
            }
            CtlCommand::ListTracks => {
                let resp = client
                    .call(IpcRequest::Library(LibraryRequest::TrackCount))
                    .await;
                print_response(resp, "list-tracks");
            }
        }
        ExitCode::SUCCESS
    })
}

fn print_response(result: Result<IpcResponse, muzon_ipc::ClientError>, label: &str) {
    match result {
        Ok(resp) => println!("{} -> {resp:?}", label),
        Err(err) => {
            eprintln!("muzonctl: {label} failed: {err}");
        }
    }
}
