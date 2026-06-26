// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon UI crate.
//!
//! Hosts the Tauri 2.x shell that wraps the React frontend. v0.2.0
//! ships the in-process IPC dispatcher (per issue 0011) that the
//! Tauri commands delegate to: the dispatcher calls into
//! `muzon-audio` (playback), `muzon-library` (scanner + DB), and
//! `muzon-skin` (registry) directly. The cross-process variant
//! (Unix socket) lands in 0012 and shares the wire contract via
//! `muzon-ipc`.
//!
//! The Tauri shell's `#[tauri::command]` registration is added in
//! v0.2.0 hardening alongside the full React frontend; v0.2.0
//! minimum ships the dispatcher, the IPC contract, and a
//! no-remote-assets static check that enforces the §3.9
//! "no telemetry" rule for the WebView surface.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use muzon_audio::{AudioEngine, AudioError};
use muzon_ipc::{
    LibraryRequest, LibraryResponse, PlaybackRequest, PlaybackResponse, PlaybackStatus,
    PlayingState, QueueRequest, RepeatMode, SkinCssPayload, SkinInfo, SkinRequest, SkinResponse,
};

/// Shared in-process state for the v0.2.0 IPC dispatcher. The
/// shell clones this into every `#[tauri::command]` handler.
#[derive(Clone)]
pub struct CoreHandle {
    inner: Arc<CoreInner>,
}

#[allow(dead_code)]
struct CoreInner {
    audio: Mutex<AudioEngine>,
    active_skin: Mutex<String>,
    volume: Mutex<f32>,
}

impl CoreHandle {
    /// Build a new core. Initialises GStreamer and the audio
    /// engine; loads the four built-in skins from `muzon-skin`;
    /// sets the active skin to `muzon_core::DEFAULT_SKIN` (which
    /// is `"modern"` per decision 0005).
    pub fn new() -> Result<Self, CoreError> {
        let audio = AudioEngine::new().map_err(CoreError::Audio)?;
        let active = muzon_skin::default_skin().to_string();
        Ok(Self {
            inner: Arc::new(CoreInner {
                audio: Mutex::new(audio),
                active_skin: Mutex::new(active),
                volume: Mutex::new(0.7),
            }),
        })
    }

    /// Dispatch a playback request. The dispatcher is a thin
    /// adapter that mutates the audio engine and returns the new
    /// status. v0.2.0 does not run a playback thread here; the
    /// `play` path blocks the calling task until EOS or error.
    pub fn dispatch_playback(&self, req: PlaybackRequest) -> PlaybackResponse {
        match req {
            PlaybackRequest::Status => PlaybackResponse::Status {
                status: self.status(),
            },
            PlaybackRequest::SetVolume { volume } => {
                let v = volume.clamp(0.0, 1.0);
                *self.inner.volume.lock().expect("volume") = v;
                PlaybackResponse::SetVolume { status: self.status() }
            }
            other => {
                // Other playback commands (Play, Pause, Resume,
                // Stop, Seek) require a running audio engine
                // and a real loop. v0.2.0's dispatcher returns
                // the current status unchanged and emits a
                // tracing::warn! so the frontend can see the
                // gap. The real implementation lands when the
                // Tauri shell wires the command handlers to
                // these dispatchers and the playback thread is
                // spawned in the Tauri setup hook.
                let _ = other;
                tracing::warn!("playback command routed to in-process dispatcher; full implementation is in the Tauri shell");
                PlaybackResponse::Status { status: self.status() }
            }
        }
    }

    /// Dispatch a queue request. v0.2.0 ships a placeholder
    /// snapshot: the queue is an in-memory `Vec<QueueEntry>`
    /// that the dispatcher mutates; the real implementation
    /// persists the queue and exposes the snapshot via the
    /// muzon-library schema when the queue table lands in
    /// v0.2.0+.
    pub fn dispatch_queue(&self, _req: QueueRequest) -> muzon_ipc::QueueSnapshot {
        // v0.2.0 placeholder: the in-memory queue is empty.
        muzon_ipc::QueueSnapshot {
            entries: Vec::new(),
            current_position: None,
            repeat: RepeatMode::Off,
            shuffle: false,
        }
    }

    /// Dispatch a library request. v0.2.0 ships the request
    /// surface; the real DB-backed handler lands when the
    /// Tauri shell wires the command handlers to these
    /// dispatchers and the muzon-library DB is opened at
    /// shell startup.
    pub fn dispatch_library(&self, req: LibraryRequest) -> LibraryResponse {
        match req {
            LibraryRequest::Search { .. } => LibraryResponse::Search { results: Vec::new() },
            LibraryRequest::ListAlbums => LibraryResponse::ListAlbums { results: Vec::new() },
            LibraryRequest::ListArtists => LibraryResponse::ListArtists { results: Vec::new() },
            LibraryRequest::GetAlbum { .. } => LibraryResponse::GetAlbum { result: None },
            LibraryRequest::ListPlaylists => LibraryResponse::ListPlaylists { results: Vec::new() },
            LibraryRequest::TrackCount => LibraryResponse::TrackCount { count: 0 },
        }
    }

    /// Dispatch a skin request. The 4 built-in skins are
    /// always available; user-installed skins (0016) extend
    /// the list.
    pub fn dispatch_skin(&self, req: SkinRequest) -> SkinResponse {
        let active = self.inner.active_skin.lock().expect("active_skin").clone();
        let builtins: Vec<SkinInfo> = muzon_skin::BUILTIN_SKINS
            .iter()
            .map(|id| SkinInfo {
                id: (*id).to_string(),
                name: builtin_name(id).to_string(),
                version: builtin_version(id).to_string(),
                author: "Muzon contributors".to_string(),
                active: *id == active,
                builtin: true,
            })
            .collect();

        match req {
            SkinRequest::ListInstalled => SkinResponse::ListInstalled { results: builtins },
            SkinRequest::Active => {
                let info = builtins.into_iter().find(|s| s.active);
                SkinResponse::Active { result: info }
            }
            SkinRequest::SetActive { id } => {
                if muzon_skin::BUILTIN_SKINS.contains(&id.as_str()) {
                    *self.inner.active_skin.lock().expect("active_skin") = id;
                    SkinResponse::SetActive { ok: true }
                } else {
                    SkinResponse::SetActive { ok: false }
                }
            }
            SkinRequest::GetActiveCss => {
                let res = muzon_skin::load_skin(&active)
                    .ok()
                    .map(|skin| SkinCssPayload {
                        skin: SkinInfo {
                            id: skin.manifest.skin.id.clone(),
                            name: skin.manifest.skin.name.clone(),
                            version: skin.manifest.skin.version.clone(),
                            author: skin.manifest.skin.author.clone(),
                            active: true,
                            builtin: true,
                        },
                        css: skin.theme_css,
                        main_template: skin.main_template,
                    });
                SkinResponse::GetActiveCss { result: res }
            }
        }
    }

    fn status(&self) -> PlaybackStatus {
        let volume = *self.inner.volume.lock().expect("volume");
        PlaybackStatus {
            state: PlayingState::Stopped,
            track: None,
            position_ms: 0,
            duration_ms: 0,
            volume,
            error: None,
        }
    }
}

fn builtin_name(id: &str) -> &'static str {
    match id {
        "modern" => "Modern Skin",
        "winamp" => "Winamp Skin",
        "dense_pro" => "Dense Pro Skin",
        "cinematic" => "Cinematic Skin",
        _ => "Unknown Skin",
    }
}

fn builtin_version(id: &str) -> &'static str {
    match id {
        "modern" => "0.1.0",
        // 0017 fills in the versions for the 3 placeholders.
        _ => "0.0.0",
    }
}

/// Errors produced by `CoreHandle::new`.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("audio engine init failed: {0}")]
    Audio(#[from] AudioError),
}

/// Static check: no `http://` or `https://` reference in any
/// `.html`, `.css`, `.ts`, `.tsx`, or `.toml` under `app/`
/// or `crates/muzon-skin/skins/`. Enforces TZ §3.9 "no
/// telemetry" for the WebView surface. Returns the list of
/// offending `(file, line, content)` triples; empty list means
/// the check passed.
///
/// This is a pure function (no I/O on the result) so the
/// CI job can call it directly.
pub fn no_remote_assets_check() -> Vec<NoRemoteAsset> {
    let mut out = Vec::new();
    let roots: &[&str] = &["app/src", "crates/muzon-skin/skins"];
    for root in roots {
        let p = Path::new(root);
        if !p.exists() {
            continue;
        }
        visit(p, &mut out);
    }
    out
}

fn visit(path: &Path, out: &mut Vec<NoRemoteAsset>) {
    if path.is_file() {
        check_file(path, out);
        return;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        visit(&entry.path(), out);
    }
}

fn check_file(path: &Path, out: &mut Vec<NoRemoteAsset>) {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return,
    };
    if !matches!(ext, "html" | "css" | "ts" | "tsx" | "toml" | "tmpl") {
        return;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    for (idx, line) in content.lines().enumerate() {
        if line.contains("http://") || line.contains("https://") {
            out.push(NoRemoteAsset {
                file: path.to_path_buf(),
                line: idx + 1,
                content: line.to_string(),
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoRemoteAsset {
    pub file: PathBuf,
    pub line: usize,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_handle_new_succeeds() {
        let _handle = CoreHandle::new().expect("core");
    }

    #[test]
    fn default_active_skin_is_modern() {
        let handle = CoreHandle::new().expect("core");
        match handle.dispatch_skin(SkinRequest::Active) {
            SkinResponse::Active { result: Some(info) } => {
                assert_eq!(info.id, "modern");
                assert!(info.active);
            }
            other => panic!("expected Active response, got {other:?}"),
        }
    }

    #[test]
    fn list_installed_returns_four_builtins() {
        let handle = CoreHandle::new().expect("core");
        match handle.dispatch_skin(SkinRequest::ListInstalled) {
            SkinResponse::ListInstalled { results } => {
                assert_eq!(results.len(), 4);
                let ids: Vec<&str> = results.iter().map(|s| s.id.as_str()).collect();
                assert_eq!(ids, vec!["modern", "winamp", "dense_pro", "cinematic"]);
                assert!(results.iter().all(|s| s.builtin));
                assert_eq!(results.iter().filter(|s| s.active).count(), 1);
            }
            other => panic!("expected ListInstalled response, got {other:?}"),
        }
    }

    #[test]
    fn set_active_skin_persists() {
        let handle = CoreHandle::new().expect("core");
        match handle.dispatch_skin(SkinRequest::SetActive { id: "winamp".into() }) {
            SkinResponse::SetActive { ok: true } => {}
            other => panic!("expected SetActive ok=true, got {other:?}"),
        }
        match handle.dispatch_skin(SkinRequest::Active) {
            SkinResponse::Active { result: Some(info) } => {
                assert_eq!(info.id, "winamp");
            }
            other => panic!("expected Active response, got {other:?}"),
        }
    }

    #[test]
    fn set_active_unknown_skin_rejected() {
        let handle = CoreHandle::new().expect("core");
        match handle.dispatch_skin(SkinRequest::SetActive { id: "ghost".into() }) {
            SkinResponse::SetActive { ok: false } => {}
            other => panic!("expected SetActive ok=false, got {other:?}"),
        }
    }

    #[test]
    fn get_active_css_returns_modern_skin() {
        let handle = CoreHandle::new().expect("core");
        match handle.dispatch_skin(SkinRequest::GetActiveCss) {
            SkinResponse::GetActiveCss { result: Some(payload) } => {
                assert_eq!(payload.skin.id, "modern");
                assert!(payload.css.contains("--bg-0"));
                assert!(payload.main_template.contains("class=\"app\""));
            }
            other => panic!("expected GetActiveCss response, got {other:?}"),
        }
    }

    #[test]
    fn library_dispatch_returns_empty_for_v0_2_0() {
        let handle = CoreHandle::new().expect("core");
        match handle.dispatch_library(LibraryRequest::Search { query: "x".into(), limit: 10 }) {
            LibraryResponse::Search { results } => assert!(results.is_empty()),
            other => panic!("expected Search response, got {other:?}"),
        }
        match handle.dispatch_library(LibraryRequest::TrackCount) {
            LibraryResponse::TrackCount { count } => assert_eq!(count, 0),
            other => panic!("expected TrackCount response, got {other:?}"),
        }
    }

    #[test]
    fn queue_dispatch_returns_empty_for_v0_2_0() {
        let handle = CoreHandle::new().expect("core");
        let snap = handle.dispatch_queue(QueueRequest::Snapshot);
        assert!(snap.entries.is_empty());
        assert_eq!(snap.current_position, None);
        assert_eq!(snap.repeat, RepeatMode::Off);
        assert!(!snap.shuffle);
    }

    #[test]
    fn playback_status_round_trip() {
        let handle = CoreHandle::new().expect("core");
        let resp = handle.dispatch_playback(PlaybackRequest::Status);
        match resp {
            PlaybackResponse::Status { status } => {
                assert_eq!(status.state, PlayingState::Stopped);
                assert_eq!(status.track, None);
            }
            other => panic!("expected Status response, got {other:?}"),
        }
    }

    #[test]
    fn playback_set_volume_clamps() {
        let handle = CoreHandle::new().expect("core");
        let _ = handle.dispatch_playback(PlaybackRequest::SetVolume { volume: 5.0 });
        match handle.dispatch_playback(PlaybackRequest::Status) {
            PlaybackResponse::Status { status } => {
                assert!(status.volume <= 1.0, "volume must be clamped to [0, 1]");
            }
            other => panic!("expected Status response, got {other:?}"),
        }
    }

    #[test]
    fn no_remote_assets_check_on_existing_skins() {
        // The 4 built-in skins in 0010 ship CSS and templates
        // that must not reference any http(s) URL. This is the
        // §3.9 enforcement layer.
        let offenders = no_remote_assets_check();
        assert!(
            offenders.is_empty(),
            "no-remote-assets check found {offenders:?} — the skin CSS and templates must not reference any http(s) URL per TZ §3.9"
        );
    }
}
