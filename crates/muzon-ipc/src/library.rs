// SPDX-License-Identifier: MIT OR Apache-2.0
//! Library IPC contract.
//!
//! Wire types for the library domain. v0.2.0 ships the request
//! surface; the handler delegates to the scanner and DB layer
//! from issues 0007-0008. v0.2.0 placeholder implementations
//! return empty results; the real DB-backed handlers land in
//! 0011's Tauri shell command registration.
//!
//! See TZ.md §2.2.

use serde::{Deserialize, Serialize};

use super::playback::TrackRef;

/// Library IPC request envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LibraryRequest {
    /// Full-text search over the FTS5 index. v0.2.0 returns
    /// empty; the real handler lands with the DB wired up.
    Search { query: String, limit: u32 },
    /// List the album ids known to the library, ordered by
    /// title. v0.2.0 returns empty.
    ListAlbums,
    /// List the artist ids known to the library, ordered by
    /// name. v0.2.0 returns empty.
    ListArtists,
    /// Get the album with the given id, including the track
    /// list. v0.2.0 returns `None`.
    GetAlbum { id: i64 },
    /// List the playlists in the library. v0.2.0 returns empty.
    ListPlaylists,
    /// Count of tracks in the library. v0.2.0 returns 0.
    TrackCount,
}

/// Album summary.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AlbumRef {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub year: Option<i32>,
    pub cover_path: Option<String>,
}

/// Artist summary.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: i64,
    pub name: String,
    pub track_count: u32,
}

/// Playlist summary.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlaylistRef {
    pub id: i64,
    pub name: String,
    pub is_smart: bool,
    pub track_count: u32,
}

/// Library IPC response envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LibraryResponse {
    Search { results: Vec<TrackRef> },
    ListAlbums { results: Vec<AlbumRef> },
    ListArtists { results: Vec<ArtistRef> },
    GetAlbum { result: Option<AlbumWithTracks> },
    ListPlaylists { results: Vec<PlaylistRef> },
    TrackCount { count: u64 },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AlbumWithTracks {
    pub album: AlbumRef,
    pub tracks: Vec<TrackRef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_round_trip() {
        let req = LibraryRequest::Search { query: "Bohemian".into(), limit: 10 };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: LibraryRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }

    #[test]
    fn list_albums_response_round_trip() {
        let resp = LibraryResponse::ListAlbums {
            results: vec![AlbumRef {
                id: 1,
                title: "A Night at the Opera".into(),
                artist: Some("Queen".into()),
                year: Some(1975),
                cover_path: None,
            }],
        };
        let json = serde_json::to_string(&resp).expect("serialise");
        let parsed: LibraryResponse = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(resp, parsed);
    }

    #[test]
    fn track_count_response_round_trip() {
        let resp = LibraryResponse::TrackCount { count: 3421 };
        let json = serde_json::to_string(&resp).expect("serialise");
        let parsed: LibraryResponse = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(resp, parsed);
    }
}
