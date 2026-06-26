// SPDX-License-Identifier: MIT OR Apache-2.0
//! Top-level IPC request envelope.
//!
//! The Tauri shell (0011) uses the per-domain enums from
//! `playback`, `queue`, `library`, and `skin` directly. The
//! cross-process Unix socket variant (0012) wraps every
//! per-domain request in a single `IpcRequest` enum with a
//! tagged variant per domain. This keeps the wire contract
//! stable while letting each domain grow independently.

use serde::{Deserialize, Serialize};

use super::library::LibraryRequest;
use super::playback::PlaybackRequest;
use super::queue::QueueRequest;
use super::skin::SkinRequest;

/// Cross-process IPC request. Default externally-tagged
/// representation; both `serde_json` and `postcard` support it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IpcRequest {
    Playback(PlaybackRequest),
    Queue(QueueRequest),
    Library(LibraryRequest),
    Skin(SkinRequest),
    /// Liveness probe. The server responds with `Pong(nonce)`.
    /// The `nonce` exists so the variant is non-empty (postcard
    /// does not support zero-byte variants).
    Ping { nonce: u64 },
}

/// Cross-process IPC response. Default externally-tagged
/// representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IpcResponse {
    Playback(super::playback::PlaybackResponse),
    Queue(super::queue::QueueSnapshot),
    Library(super::library::LibraryResponse),
    Skin(super::skin::SkinResponse),
    Pong { nonce: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_request_round_trip() {
        let req = IpcRequest::Ping { nonce: 42 };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: IpcRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }

    #[test]
    fn pong_response_round_trip() {
        let resp = IpcResponse::Pong { nonce: 42 };
        let json = serde_json::to_string(&resp).expect("deserialise");
        let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(resp, parsed);
    }

    #[test]
    fn playback_request_in_envelope_round_trip() {
        let req = IpcRequest::Playback(PlaybackRequest::SetVolume { volume: 0.5 });
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: IpcRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }
}
