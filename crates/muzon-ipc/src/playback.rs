// SPDX-License-Identifier: MIT OR Apache-2.0
//! Playback IPC contract.
//!
//! Wire types for the playback domain. The Tauri shell (0011)
//! registers these as `#[tauri::command]` handlers; the React
//! frontend calls them via `invoke()`. The same types are used
//! by the headless cross-process variant in 0012 (Unix socket
//! IPC); the in-process variant in 0011 is the v0.2.0 default.
//!
//! See TZ.md §2.1, §3.1.2.

use serde::{Deserialize, Serialize};

/// State of the playback engine. Mirrors the GStreamer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayingState {
    #[default]
    Stopped,
    Playing,
    Paused,
    EndOfStream,
    Error,
}

/// Lightweight track reference for the playback IPC surface. The
/// full `muzon_library::Track` is not serialised here because the
/// playback layer only needs the path and the display title.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TrackRef {
    /// `tracks.id` in the library database, if known.
    pub track_id: Option<i64>,
    /// File path (the natural key).
    pub path: String,
    /// Display title, if known.
    pub title: Option<String>,
    /// Display artist, if known.
    pub artist: Option<String>,
    /// Display album, if known.
    pub album: Option<String>,
    /// Duration in milliseconds, if known.
    pub duration_ms: Option<u64>,
}

/// Playback status snapshot. Returned by `Status`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PlaybackStatus {
    pub state: PlayingState,
    pub track: Option<TrackRef>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    /// Last error message, if any. `state == Error` when set.
    pub error: Option<String>,
}

/// Playback IPC request envelope. The Tauri command is a thin
/// dispatcher that matches on `request` and calls into the
/// playback engine; the headless variant in 0012 uses the same
/// enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PlaybackRequest {
    /// Start playback of a single file. Resolves the path via the
    /// library, then sets up the GStreamer pipeline.
    Play { path: String },
    Pause,
    Resume,
    Stop,
    /// Seek to `position_ms`.
    Seek { position_ms: u64 },
    /// Set the output volume in `[0.0, 1.0]`.
    SetVolume { volume: f32 },
    /// Snapshot the current playback state. Returns
    /// `PlaybackStatus`.
    Status,
}

/// Playback IPC response envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PlaybackResponse {
    Play { status: PlaybackStatus },
    Pause { status: PlaybackStatus },
    Resume { status: PlaybackStatus },
    Stop { status: PlaybackStatus },
    Seek { status: PlaybackStatus },
    SetVolume { status: PlaybackStatus },
    Status { status: PlaybackStatus },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_request_round_trip() {
        let req = PlaybackRequest::Seek { position_ms: 1234 };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: PlaybackRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }

    #[test]
    fn playback_response_status_round_trip() {
        let resp = PlaybackResponse::Status {
            status: PlaybackStatus {
                state: PlayingState::Playing,
                track: Some(TrackRef {
                    track_id: Some(42),
                    path: "/tmp/a.flac".into(),
                    title: Some("A".into()),
                    artist: Some("X".into()),
                    album: Some("Y".into()),
                    duration_ms: Some(240_000),
                }),
                position_ms: 12_345,
                duration_ms: 240_000,
                volume: 0.7,
                error: None,
            },
        };
        let json = serde_json::to_string(&resp).expect("serialise");
        let parsed: PlaybackResponse = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(resp, parsed);
    }

    #[test]
    fn playing_state_serde_lowercase() {
        for (s, expected) in [
            ("\"stopped\"", PlayingState::Stopped),
            ("\"playing\"", PlayingState::Playing),
            ("\"paused\"", PlayingState::Paused),
            ("\"endofstream\"", PlayingState::EndOfStream),
            ("\"error\"", PlayingState::Error),
        ] {
            let parsed: PlayingState = serde_json::from_str(s).expect("deserialise");
            assert_eq!(parsed, expected, "round-trip failed for {s}");
        }
    }
}
