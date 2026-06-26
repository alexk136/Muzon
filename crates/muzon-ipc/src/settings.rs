// SPDX-License-Identifier: MIT OR Apache-2.0
//! Settings IPC contract.
//!
//! The Settings screen (issue 0019) uses these typed
//! requests/responses to read and write the user's
//! configuration. The wire format is serde-tagged on `op` for
//! stability across the React <-> Rust boundary.
//!
//! The SettingsSnapshot is a read-only projection of the
//! current config: the frontend uses it to populate the form,
//! and `Update` / `Reset` writes back through the CoreHandle's
//! dispatcher (which persists to `~/.config/muzon/config.toml`).

use std::collections::HashMap;

use muzon_core::{AccentColor, ThemeMode};
use serde::{Deserialize, Serialize};

/// Settings IPC request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SettingsRequest {
    /// Read the current config as a `SettingsSnapshot`.
    Get,
    /// Apply a single key=value patch and return the new
    /// snapshot. Unknown keys are rejected (the validation
    /// error is in the response).
    Update { patch: HashMap<String, String> },
    /// Reset to the default config and return the new
    /// snapshot.
    Reset,
}

/// Settings IPC response envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SettingsResponse {
    Get {
        snapshot: Box<SettingsSnapshot>,
    },
    Update {
        snapshot: Box<SettingsSnapshot>,
    },
    Reset {
        snapshot: Box<SettingsSnapshot>,
    },
    Error {
        message: String,
    },
}

/// A read-only projection of the current config. The
/// frontend renders this as the Settings form; the keys are
/// the config field paths (e.g. `theme.mode`, `audio.bit_perfect`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    /// Theme mode (Light / Dark / Auto).
    pub theme_mode: ThemeMode,
    /// Accent color (preset name or custom hex).
    pub theme_accent: String,
    /// Audio output device (empty = GStreamer default).
    pub audio_output_device: String,
    /// Bit-perfect mode (true = strip resampler).
    pub audio_bit_perfect: bool,
    /// Gapless playback.
    pub audio_gapless: bool,
    /// ReplayGain mode (none / track / album).
    pub audio_replaygain: String,
    /// Crossfade duration in seconds.
    pub audio_crossfade_seconds: u32,
    /// Library scan roots (absolute paths).
    pub library_scan_roots: Vec<String>,
    /// Watch mode (auto-rescan on FS events).
    pub library_watch: bool,
    /// Minimum file size in bytes.
    pub library_min_file_size_bytes: u64,
    /// Honour ignore markers (.nomedia, .muzonignore).
    pub library_honor_ignore_markers: bool,
    /// Follow symbolic links.
    pub library_follow_symlinks: bool,
    /// Active skin id.
    pub ui_default_skin: String,
    /// Network: every flag defaults to false per 0001-N4.
    pub network_explicit_opt_in: bool,
    pub network_anonymous_crash_reports: bool,
    pub network_llm_providers: bool,
    pub network_acoustid: bool,
    pub network_musicbrainz: bool,
    pub network_lastfm: bool,
    pub network_listenbrainz: bool,
    /// LLM provider config (URL + model name per provider).
    pub llm_provider_urls: HashMap<String, String>,
    pub llm_provider_models: HashMap<String, String>,
    /// Logging level (info, debug, etc.).
    pub logging_level: String,
    /// Logging rotation cap in MB.
    pub logging_max_size_mb: u32,
    /// Logging file count cap.
    pub logging_max_files: u32,
}

impl SettingsSnapshot {
    /// Build a snapshot from the canonical defaults. The
    /// frontend uses this for the "Reset" preview; the Rust
    /// `MuzonConfig::default()` is the source of truth.
    pub fn default_snapshot() -> Self {
        Self {
            theme_mode: ThemeMode::default(),
            theme_accent: AccentColor::Violet.hex(),
            audio_output_device: String::new(),
            audio_bit_perfect: false,
            audio_gapless: true,
            audio_replaygain: "none".into(),
            audio_crossfade_seconds: 0,
            library_scan_roots: Vec::new(),
            library_watch: true,
            library_min_file_size_bytes: 32 * 1024,
            library_honor_ignore_markers: true,
            library_follow_symlinks: true,
            ui_default_skin: "modern".into(),
            network_explicit_opt_in: false,
            network_anonymous_crash_reports: false,
            network_llm_providers: false,
            network_acoustid: false,
            network_musicbrainz: false,
            network_lastfm: false,
            network_listenbrainz: false,
            llm_provider_urls: HashMap::new(),
            llm_provider_models: HashMap::new(),
            logging_level: "info".into(),
            logging_max_size_mb: 10,
            logging_max_files: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_default_matches_documented_values() {
        let s = SettingsSnapshot::default_snapshot();
        assert_eq!(s.theme_mode, ThemeMode::Auto);
        assert_eq!(s.theme_accent, "#8b5cf6");
        assert_eq!(s.audio_bit_perfect, false);
        assert_eq!(s.audio_gapless, true);
        assert_eq!(s.audio_replaygain, "none");
        assert_eq!(s.audio_crossfade_seconds, 0);
        assert_eq!(s.library_min_file_size_bytes, 32 * 1024);
        assert_eq!(s.ui_default_skin, "modern");
        assert_eq!(s.network_explicit_opt_in, false);
        assert!(!s.network_acoustid);
        assert!(!s.network_lastfm);
        assert_eq!(s.logging_level, "info");
        assert_eq!(s.logging_max_size_mb, 10);
        assert_eq!(s.logging_max_files, 3);
    }

    #[test]
    fn settings_get_request_round_trip() {
        let req = SettingsRequest::Get;
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: SettingsRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }

    #[test]
    fn settings_update_request_round_trip() {
        let mut patch = HashMap::new();
        patch.insert("theme.mode".into(), "dark".into());
        patch.insert("audio.bit_perfect".into(), "true".into());
        let req = SettingsRequest::Update { patch };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: SettingsRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }
}
