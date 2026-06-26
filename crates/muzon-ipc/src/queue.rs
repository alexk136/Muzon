// SPDX-License-Identifier: MIT OR Apache-2.0
//! Queue IPC contract.
//!
//! Wire types for the queue domain. v0.2.0 ships the request
//! surface; the in-process handler in 0011 and the headless
//! variant in 0012 share the same enum.
//!
//! See TZ.md §2.1.1 (queue, repeat modes, shuffle).

use serde::{Deserialize, Serialize};

use super::playback::TrackRef;

/// Repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

/// One entry in the queue snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QueueEntry {
    pub track: TrackRef,
    pub position: u32,
}

/// Queue snapshot. Returned by `Snapshot`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub entries: Vec<QueueEntry>,
    pub current_position: Option<u32>,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

/// Queue IPC request envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum QueueRequest {
    /// Append a track to the end of the queue.
    Enqueue { path: String },
    /// Insert a track at the head of the queue (next to play).
    EnqueueNext { path: String },
    /// Remove the entry at the given position.
    Remove { position: u32 },
    /// Move an entry from `from` to `to`.
    Move { from: u32, to: u32 },
    /// Clear the queue.
    Clear,
    /// Set the repeat mode.
    SetRepeatMode { mode: RepeatMode },
    /// Toggle shuffle on or off.
    SetShuffle { on: bool },
    /// Snapshot the queue state.
    Snapshot,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_mode_serde() {
        for (s, expected) in [
            ("\"off\"", RepeatMode::Off),
            ("\"one\"", RepeatMode::One),
            ("\"all\"", RepeatMode::All),
        ] {
            let parsed: RepeatMode = serde_json::from_str(s).expect("deserialise");
            assert_eq!(parsed, expected, "round-trip failed for {s}");
        }
    }

    #[test]
    fn enqueue_request_round_trip() {
        let req = QueueRequest::Enqueue { path: "/tmp/a.flac".into() };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: QueueRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }

    #[test]
    fn snapshot_round_trip() {
        let snap = QueueSnapshot {
            entries: vec![QueueEntry {
                track: TrackRef {
                    track_id: Some(1),
                    path: "/tmp/a.flac".into(),
                    title: Some("A".into()),
                    artist: None,
                    album: None,
                    duration_ms: Some(180_000),
                },
                position: 0,
            }],
            current_position: Some(0),
            repeat: RepeatMode::All,
            shuffle: true,
        };
        let json = serde_json::to_string(&snap).expect("serialise");
        let parsed: QueueSnapshot = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(snap, parsed);
    }
}
