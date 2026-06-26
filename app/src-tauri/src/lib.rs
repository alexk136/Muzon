// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tauri 2.x application shell.
//!
//! Hosts the React (or static HTML) frontend and exposes the
//! `muzon_ipc` contract as `#[tauri::command]` handlers. The
//! in-process `CoreHandle` from `muzon-ui` is the shared
//! state; each command delegates to the appropriate
//! dispatcher.
//!
//! See issue 0011 for the IPC contract and issue 0015 for
//! the Player screen that consumes the `playback`, `queue`,
//! `library`, and `skin` commands.

use muzon_core::{init_logging, MuzonConfig, MuzonPaths};
use muzon_ipc::{
    LibraryRequest, PlaybackRequest, PlaybackResponse, QueueRequest, RepeatMode,
    SkinRequest, SkinResponse,
};
use serde::Serialize;
use tauri::State;

#[derive(Clone)]
struct AppState {
    core: muzon_ui::CoreHandle,
    paths: MuzonPaths,
}

#[derive(Debug, thiserror::Error, Serialize)]
pub enum AppError {
    #[error("audio init: {0}")]
    AudioInit(String),
    #[error("ipc error: {0}")]
    Ipc(String),
}

#[tauri::command]
fn ping(nonce: u64) -> u64 {
    nonce
}

#[tauri::command]
fn get_active_skin(state: State<AppState>) -> Result<serde_json::Value, String> {
    let resp = state
        .core
        .dispatch_skin(SkinRequest::GetActiveCss);
    match resp {
        SkinResponse::GetActiveCss { result: Some(p) } => Ok(serde_json::json!({
            "id": p.skin.id,
            "name": p.skin.name,
            "version": p.skin.version,
            "css": p.css,
            "main_template": p.main_template,
        })),
        SkinResponse::GetActiveCss { result: None } => Err("no active skin".to_string()),
        other => Err(format!("unexpected: {other:?}")),
    }
}

#[tauri::command]
fn list_installed_skins(state: State<AppState>) -> Result<serde_json::Value, String> {
    let resp = state.core.dispatch_skin(SkinRequest::ListInstalled);
    match resp {
        SkinResponse::ListInstalled { results } => Ok(serde_json::to_value(results).unwrap_or_default()),
        other => Err(format!("unexpected: {other:?}")),
    }
}

#[tauri::command]
fn set_active_skin(state: State<AppState>, id: String) -> bool {
    let resp = state
        .core
        .dispatch_skin(SkinRequest::SetActive { id });
    matches!(resp, SkinResponse::SetActive { ok: true })
}

#[tauri::command]
fn playback_status(state: State<AppState>) -> serde_json::Value {
    let resp = state
        .core
        .dispatch_playback(PlaybackRequest::Status);
    match resp {
        PlaybackResponse::Status { status } => serde_json::to_value(status).unwrap_or_default(),
        _ => serde_json::Value::Null,
    }
}

#[tauri::command]
fn playback_play(state: State<AppState>, path: String) {
    let _ = state
        .core
        .dispatch_playback(PlaybackRequest::Play { path });
}

#[tauri::command]
fn playback_pause(state: State<AppState>) {
    let _ = state.core.dispatch_playback(PlaybackRequest::Pause);
}

#[tauri::command]
fn playback_resume(state: State<AppState>) {
    let _ = state.core.dispatch_playback(PlaybackRequest::Resume);
}

#[tauri::command]
fn playback_seek(state: State<AppState>, position_ms: u64) {
    let _ = state
        .core
        .dispatch_playback(PlaybackRequest::Seek { position_ms });
}

#[tauri::command]
fn playback_set_volume(state: State<AppState>, volume: f32) {
    let _ = state
        .core
        .dispatch_playback(PlaybackRequest::SetVolume { volume });
}

#[tauri::command]
fn queue_snapshot(state: State<AppState>) -> serde_json::Value {
    let resp = state.core.dispatch_queue(QueueRequest::Snapshot);
    serde_json::to_value(resp).unwrap_or_default()
}

#[tauri::command]
fn queue_set_repeat(state: State<AppState>, mode: String) {
    let parsed = match mode.as_str() {
        "off" => RepeatMode::Off,
        "one" => RepeatMode::One,
        "all" => RepeatMode::All,
        _ => return,
    };
    let _ = state
        .core
        .dispatch_queue(QueueRequest::SetRepeatMode { mode: parsed });
}

#[tauri::command]
fn queue_set_shuffle(state: State<AppState>, on: bool) {
    let _ = state
        .core
        .dispatch_queue(QueueRequest::SetShuffle { on });
}

#[tauri::command]
fn library_overview(state: State<AppState>) -> serde_json::Value {
    let resp = state.core.dispatch_library(LibraryRequest::TrackCount);
    serde_json::to_value(resp).unwrap_or_default()
}

#[tauri::command]
fn library_search(
    state: State<AppState>,
    query: String,
    limit: u32,
) -> serde_json::Value {
    let resp = state
        .core
        .dispatch_library(LibraryRequest::Search { query, limit });
    serde_json::to_value(resp).unwrap_or_default()
}

#[tauri::command]
fn get_paths(state: State<AppState>) -> serde_json::Value {
    serde_json::json!({
        "config_dir": state.paths.config_dir.display().to_string(),
        "data_dir": state.paths.data_dir.display().to_string(),
        "cache_dir": state.paths.cache_dir.display().to_string(),
        "log_dir": state.paths.log_dir.display().to_string(),
        "config_file": state.paths.config_file().display().to_string(),
        "log_file": state.paths.log_file().display().to_string(),
    })
}

pub fn run() -> tauri::Result<()> {
    let paths = MuzonPaths::resolve().map_err(|e| tauri::Error::Anyhow(e.into()))?;
    let cfg = MuzonConfig::load(&paths.config_file())
        .map_err(|e| tauri::Error::Anyhow(e.into()))?;
    let _guard = init_logging(&paths, &cfg.logging)
        .map_err(|e| tauri::Error::Anyhow(e.into()))?;
    let core = muzon_ui::CoreHandle::new()
        .map_err(|e| tauri::Error::Anyhow(e.into()))?;

    tauri::Builder::default()
        .manage(AppState {
            core: core.clone(),
            paths: paths.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            get_active_skin,
            list_installed_skins,
            set_active_skin,
            playback_status,
            playback_play,
            playback_pause,
            playback_resume,
            playback_seek,
            playback_set_volume,
            queue_snapshot,
            queue_set_repeat,
            queue_set_shuffle,
            library_overview,
            library_search,
            get_paths,
        ])
        .setup(|_app| {
            tracing::info!("muzon-app: Tauri shell ready");
            Ok(())
        })
        .run(tauri::generate_context!())
        .map_err(Into::into)
}
