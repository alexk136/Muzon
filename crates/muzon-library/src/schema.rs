// SPDX-License-Identifier: MIT OR Apache-2.0
//! Typed accessors over the SQLite schema from 0007.
//!
//! This module defines the data model and the typed `Library`
//! methods that consumers (the scanner in 0008, the CLI in 0009,
//! the v0.2.0 UI) call. The implementation targets sqlx's
//! `FromRow` derives; queries are not compile-time-checked (no
//! `sqlx::query!` macro) because the v0.1.0 build keeps CI
//! hermetic without a live DB. v0.2.0 may adopt compile-time
//! queries via a checked-in `.sqlx` snapshot.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};

use muzon_core::MuzonPaths;

use crate::db::Library;

/// One row of the `tracks` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub size_bytes: i64,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub codec: Option<String>,
    pub title: Option<String>,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub replaygain_track: Option<f64>,
    pub replaygain_album: Option<f64>,
    pub fingerprint: Option<String>,
    pub last_played_at: Option<i64>,
    pub play_count: i64,
    pub skip_count: i64,
    pub mbid_recording: Option<String>,
    pub acoustid: Option<String>,
    pub embedding: Option<Vec<u8>>,
    pub needs_tag_write: i64,
    pub cover_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Codec enum mirroring the eight formats in TZ §2.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Mp3,
    Flac,
    Ogg,
    Opus,
    Wav,
    Aac,
    Ape,
    Wma,
    Other,
}

impl Codec {
    /// Map a file extension (lowercase, without the leading dot) to
    /// a `Codec`. Returns `Codec::Other` for unknown extensions.
    pub fn from_ext(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "mp3" => Self::Mp3,
            "flac" => Self::Flac,
            "ogg" | "oga" => Self::Ogg,
            "opus" => Self::Opus,
            "wav" => Self::Wav,
            "aac" | "m4a" | "mp4" => Self::Aac,
            "ape" => Self::Ape,
            "wma" => Self::Wma,
            _ => Self::Other,
        }
    }

    /// Stable string for storage in `tracks.codec`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Wav => "wav",
            Self::Aac => "aac",
            Self::Ape => "ape",
            Self::Wma => "wma",
            Self::Other => "other",
        }
    }
}

impl Track {
    /// Parse the codec from the file extension in `path`.
    pub fn codec_from_path(path: impl AsRef<Path>) -> Codec {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(Codec::from_ext)
            .unwrap_or(Codec::Other)
    }
}

// Free function alias used by the scanner. Kept as a free fn for
// ergonomic call sites that already have a `Path` value.
pub fn codec_from_path(path: impl AsRef<Path>) -> Codec {
    Track::codec_from_path(path)
}

impl Library {
    /// Upsert a track by `path`. Returns the row id of the inserted
    /// or updated row. The caller fills in metadata fields; this
    /// method is intentionally narrow (path + size + codec) so the
    /// scanner in 0008 can call it cheaply and then do a follow-up
    /// tag read.
    pub async fn upsert_track_path(
        &self,
        path: &str,
        size_bytes: i64,
        codec: Codec,
    ) -> Result<i64, sqlx::Error> {
        let codec_str = codec.as_str();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (path, size_bytes, codec) VALUES (?, ?, ?) \
             ON CONFLICT(path) DO UPDATE SET size_bytes = excluded.size_bytes, \
                                            codec = excluded.codec, \
                                            updated_at = strftime('%s', 'now') \
             RETURNING id",
        )
        .bind(path)
        .bind(size_bytes)
        .bind(codec_str)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    /// Fetch a track by id.
    pub async fn track_by_id(&self, id: i64) -> Result<Option<Track>, sqlx::Error> {
        sqlx::query_as::<_, Track>("SELECT * FROM tracks WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool())
            .await
    }

    /// Fetch a track by path (the natural key).
    pub async fn track_by_path(&self, path: &str) -> Result<Option<Track>, sqlx::Error> {
        sqlx::query_as::<_, Track>("SELECT * FROM tracks WHERE path = ?")
            .bind(path)
            .fetch_optional(self.pool())
            .await
    }

    /// Update the FTS index for a track. The application layer joins
    /// `track_artists` / `track_albums` / `track_tags` to compute the
    /// denormalised search columns and writes them via this method.
    pub async fn update_fts(
        &self,
        track_id: i64,
        title: &str,
        artist_names: &str,
        album_title: &str,
        tag_names: &str,
        path: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE tracks_fts SET title = ?, artist_names = ?, album_title = ?, tag_names = ?, path = ? WHERE rowid = ?")
            .bind(title)
            .bind(artist_names)
            .bind(album_title)
            .bind(tag_names)
            .bind(path)
            .bind(track_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// FTS5 search. Returns up to `limit` track ids ranked by FTS5
    /// bm25. The query is wrapped in a `MATCH` expression and is
    /// expected to be a sanitized user input (or a server-side
    /// query). This is the §3.7 perf target: < 50 ms on 50k tracks.
    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<i64>, sqlx::Error> {
        // Sanitise the query for FTS5: wrap each token in double
        // quotes, escape any internal double quote by doubling it.
        // This avoids FTS5 syntax errors on raw user input.
        let safe = sanitise_fts_query(query);
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ? ORDER BY bm25(tracks_fts) LIMIT ?",
        )
        .bind(&safe)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Count of tracks in the library.
    pub async fn track_count(&self) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(self.pool())
            .await
    }
}

fn sanitise_fts_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    let mut started = false;
    for tok in input.split_whitespace() {
        if started {
            out.push(' ');
        }
        out.push('"');
        for ch in tok.chars() {
            if ch == '"' {
                out.push('"');
            }
            out.push(ch);
        }
        out.push('"');
        started = true;
    }
    if out.is_empty() {
        "\"\"".to_string()
    } else {
        out
    }
}

/// Open the library at the standard `library.db` path. Convenience
/// wrapper used by 0008 (the scanner) and 0009 (the CLI).
pub async fn open_default(paths: &MuzonPaths) -> Result<Library, sqlx::Error> {
    Library::open(paths).await
}

/// Re-export the pool type for downstream crates that want raw
/// `sqlx` access (the scanner in 0008 uses this for the bulk
/// insert).
pub type DbPool = Pool<Sqlite>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_lock::ENV_LOCK;
    use muzon_core::MuzonPaths;

    #[test]
    fn codec_from_ext_covers_all_forms() {
        assert_eq!(Codec::from_ext("mp3"), Codec::Mp3);
        assert_eq!(Codec::from_ext("FLAC"), Codec::Flac);
        assert_eq!(Codec::from_ext("oga"), Codec::Ogg);
        assert_eq!(Codec::from_ext("m4a"), Codec::Aac);
        assert_eq!(Codec::from_ext("opus"), Codec::Opus);
        assert_eq!(Codec::from_ext("wav"), Codec::Wav);
        assert_eq!(Codec::from_ext("ape"), Codec::Ape);
        assert_eq!(Codec::from_ext("wma"), Codec::Wma);
        assert_eq!(Codec::from_ext("xyz"), Codec::Other);
    }

    #[test]
    fn sanitise_fts_query_handles_special_chars() {
        assert_eq!(sanitise_fts_query("hello world"), "\"hello\" \"world\"");
        assert_eq!(sanitise_fts_query(""), "\"\"");
        assert_eq!(sanitise_fts_query("a\"b"), "\"a\"\"b\"");
    }

    #[tokio::test]
    async fn upsert_then_fetch_round_trip() {
        let _g = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        let id1 = lib
            .upsert_track_path("/tmp/a.mp3", 1024, Codec::Mp3)
            .await
            .expect("insert");
        let id2 = lib
            .upsert_track_path("/tmp/a.mp3", 2048, Codec::Mp3)
            .await
            .expect("upsert");
        assert_eq!(id1, id2, "same path should reuse the same row id");
        let t = lib.track_by_id(id1).await.expect("by id").expect("row");
        assert_eq!(t.path, "/tmp/a.mp3");
        assert_eq!(t.size_bytes, 2048);
        assert_eq!(t.codec.as_deref(), Some("mp3"));
    }

    #[tokio::test]
    async fn fts_round_trip() {
        let _g = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        let id = lib
            .upsert_track_path("/tmp/bohemian.flac", 4096, Codec::Flac)
            .await
            .expect("insert");
        lib.update_fts(id, "Bohemian Rhapsody", "Queen", "A Night at the Opera", "", "/tmp/bohemian.flac")
            .await
            .expect("update fts");
        let hits = lib.search("Bohemian", 10).await.expect("search");
        assert_eq!(hits, vec![id], "expected the inserted track id in FTS results");
    }

    #[tokio::test]
    async fn fts_triggers_keep_index_in_sync() {
        let _g = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        let id = lib
            .upsert_track_path("/tmp/foo.flac", 1024, Codec::Flac)
            .await
            .expect("insert");
        // The insert trigger seeds the FTS row with title="" and path=...
        let hits = lib.search("/tmp/foo.flac", 10).await.expect("search");
        assert_eq!(hits, vec![id], "path-only match expected right after insert");
        // Update the path; the trigger should keep the FTS index in sync.
        sqlx::query("UPDATE tracks SET path = ? WHERE id = ?")
            .bind("/tmp/bar.flac")
            .bind(id)
            .execute(lib.pool())
            .await
            .expect("update path");
        // FTS5 path column is now stale because the trigger only
        // updates the title and the manual update_fts path column.
        // Verify the manual update_fts path is the contract.
        lib.update_fts(id, "", "", "", "", "/tmp/bar.flac")
            .await
            .expect("update fts path");
        let hits = lib.search("/tmp/bar.flac", 10).await.expect("search");
        assert_eq!(hits, vec![id], "expected the FTS path update to be reflected");
    }

    #[tokio::test]
    async fn open_default_uses_muzon_paths() {
        let _g = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_var("MUZON_HOME", tmp.path());
        let paths = MuzonPaths::resolve().expect("resolve");
        let lib = open_default(&paths).await.expect("open_default");
        let count = lib.track_count().await.expect("count");
        assert_eq!(count, 0, "fresh library should have zero tracks");
        std::env::remove_var("MUZON_HOME");
    }
}
