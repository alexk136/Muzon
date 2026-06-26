// SPDX-License-Identifier: MIT OR Apache-2.0
//! `muzon --headless` entry point.
//!
//! Runs the core as a standalone process. The Tauri shell from
//! 0011 can attach to the running core via the Unix socket
//! instead of spawning an in-process one. `muzonctl` (in
//! `ctl.rs`) is the CLI client that talks to the running
//! core over the same socket.

use std::path::PathBuf;
use std::process::ExitCode;

use muzon_core::{init_logging, MuzonConfig, MuzonPaths};
use muzon_ipc::default_socket_path;
use tracing::{error, info};

pub fn run() -> ExitCode {
    let paths = match MuzonPaths::resolve() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("muzon --headless: failed to resolve XDG paths: {err}");
            return ExitCode::from(1);
        }
    };
    let mut cfg = match MuzonConfig::load(&paths.config_file()) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("muzon --headless: failed to load config: {err}");
            return ExitCode::from(1);
        }
    };
    if let Ok(level) = std::env::var("MUZON_LOG_LEVEL") {
        cfg.logging.level = level;
    }

    let _guard = match init_logging(&paths, &cfg.logging) {
        Ok(g) => g,
        Err(err) => {
            eprintln!("muzon --headless: failed to init logging: {err}");
            return ExitCode::from(1);
        }
    };

    let socket_path = std::env::var("MUZON_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_socket_path());

    let core = match muzon_ui::CoreHandle::new() {
        Ok(c) => c,
        Err(err) => {
            error!("muzon --headless: failed to build core: {err}");
            return ExitCode::from(5);
        }
    };

    info!("muzon --headless: socket={}", socket_path.display());

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            error!("muzon --headless: failed to build tokio runtime: {err}");
            return ExitCode::from(5);
        }
    };

    runtime.block_on(async move {
        let handler = move |req: muzon_ipc::IpcRequest| {
            let core = core.clone();
            async move { dispatch(&core, req).await }
        };
        let socket_path_clone = socket_path.clone();
        let server_task = tokio::spawn(async move {
            muzon_ipc::serve(&socket_path_clone, handler).await
        });
        let mut sigint = match tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::interrupt(),
        ) {
            Ok(s) => s,
            Err(err) => {
                error!("muzon --headless: SIGINT handler: {err}");
                return ExitCode::from(5);
            }
        };
        let mut sigterm = match tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        ) {
            Ok(s) => s,
            Err(err) => {
                error!("muzon --headless: SIGTERM handler: {err}");
                return ExitCode::from(5);
            }
        };
        tokio::select! {
            _ = sigint.recv() => info!("muzon --headless: SIGINT"),
            _ = sigterm.recv() => info!("muzon --headless: SIGTERM"),
        }
        info!("muzon --headless: shutting down");
        let _ = std::fs::remove_file(&socket_path);
        server_task.abort();
        ExitCode::SUCCESS
    })
}

async fn dispatch(
    core: &muzon_ui::CoreHandle,
    req: muzon_ipc::IpcRequest,
) -> muzon_ipc::IpcResponse {
    use muzon_ipc::{IpcRequest, IpcResponse};
    match req {
        IpcRequest::Playback(p) => IpcResponse::Playback(core.dispatch_playback(p)),
        IpcRequest::Queue(q) => IpcResponse::Queue(core.dispatch_queue(q)),
        IpcRequest::Library(l) => IpcResponse::Library(core.dispatch_library(l)),
        IpcRequest::Skin(s) => IpcResponse::Skin(core.dispatch_skin(s)),
        IpcRequest::Ping { nonce } => IpcResponse::Pong { nonce },
    }
}
