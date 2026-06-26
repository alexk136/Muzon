// SPDX-License-Identifier: MIT OR Apache-2.0
//! FTS5 search and facet aggregation for the Library screen.
//!
//! Implements TZ §2.2.4: full-text search across the
//! `tracks_fts` index, plus facet aggregations for the
//! filter-chip row (genre, year, format). Performance target
//! from §3.7: search < 50 ms on 50k tracks.

use muzon_ipc::TrackRef;
use sqlx::Row;

use crate::db::Library;

/// Run the FTS5 query and return up to `limit` ranked
/// `TrackRef`s. The query is sanitised at the IPC layer
/// (each token wrapped in double quotes) so this function
/// trusts the input.
pub async fn fts(library: &Library, query: &str, limit: u32) -> Result<Vec<TrackRef>, sqlx::Error> {
    let row_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ?1 \
         ORDER BY bm25(tracks_fts) LIMIT ?2",
    )
    .bind(query)
    .bind(limit as i64)
    .fetch_all(library.pool())
    .await?;
    if row_ids.is_empty() {
        return Ok(Vec::new());
    }
    // Resolve the rowids to TrackRef. tracks_fts is a virtual
    // table indexed by rowid; we read the denormalised columns
    // for each rowid.
    let mut out = Vec::with_capacity(row_ids.len());
    for id in row_ids {
        let row = sqlx::query(
            "SELECT path, title, artist_names, album_title, tag_names \
             FROM tracks_fts WHERE rowid = ?1",
        )
        .bind(id)
        .fetch_optional(library.pool())
        .await?;
        if let Some(row) = row {
            out.push(TrackRef {
                track_id: Some(id),
                path: row.try_get("path").unwrap_or_default(),
                title: row.try_get("title").ok().flatten(),
                artist: row.try_get("artist_names").ok().flatten(),
                album: row.try_get("album_title").ok().flatten(),
                duration_ms: None,
            });
        }
    }
    Ok(out)
}

/// Aggregate the genre facet. Returns `(genre_name, track_count)`
/// pairs sorted by count descending.
pub async fn facet_genres(library: &Library) -> Result<Vec<(String, u32)>, sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT g.name, COUNT(tg.track_id) AS cnt FROM genres g \
         LEFT JOIN track_genres tg ON tg.genre_id = g.id \
         GROUP BY g.id ORDER BY cnt DESC, g.name ASC",
    )
    .fetch_all(library.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(name, cnt)| (name, cnt.max(0) as u32))
        .collect())
}

/// Aggregate the year facet. Returns `(year, track_count)` pairs
/// sorted by year descending.
pub async fn facet_years(library: &Library) -> Result<Vec<(i32, u32)>, sqlx::Error> {
    let rows: Vec<(Option<i32>, i64)> = sqlx::query_as(
        "SELECT year, COUNT(*) AS cnt FROM tracks \
         WHERE year IS NOT NULL \
         GROUP BY year ORDER BY year DESC",
    )
    .fetch_all(library.pool())
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(year, cnt)| year.map(|y| (y, cnt.max(0) as u32)))
        .collect())
}

/// Aggregate the format facet. Returns
/// `(codec_string, track_count)` pairs sorted by count
/// descending. The codec is the `tracks.codec` column
/// (mp3/flac/ogg/opus/wav/aac/ape/wma/other).
pub async fn facet_formats(library: &Library) -> Result<Vec<(String, u32)>, sqlx::Error> {
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT codec, COUNT(*) AS cnt FROM tracks \
         WHERE codec IS NOT NULL \
         GROUP BY codec ORDER BY cnt DESC, codec ASC",
    )
    .fetch_all(library.pool())
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(codec, cnt)| codec.map(|c| (c, cnt.max(0) as u32)))
        .collect())
}

/// Aggregate the entire sidebar tree in a single call. The
/// IPC `TreeCounts` method on the Library domain returns
/// these; the React sidebar consumes them in one round trip
/// at mount.
pub async fn tree_counts(library: &Library) -> Result<TreeCounts, sqlx::Error> {
    let track_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(library.pool())
        .await?;
    let album_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums")
        .fetch_one(library.pool())
        .await?;
    let artist_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists")
        .fetch_one(library.pool())
        .await?;
    let genre_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM genres")
        .fetch_one(library.pool())
        .await?;
    let recently_added: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tracks \
         ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_one(library.pool())
    .await?;
    let recently_played: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tracks \
         WHERE last_played_at IS NOT NULL \
         ORDER BY last_played_at DESC LIMIT 200",
    )
    .fetch_one(library.pool())
    .await?;
    let favorites: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tracks WHERE play_count >= 5",
    )
    .fetch_one(library.pool())
    .await?;
    Ok(TreeCounts {
        all_music: track_count.max(0) as u32,
        albums: album_count.max(0) as u32,
        artists: artist_count.max(0) as u32,
        genres: genre_count.max(0) as u32,
        favorites: favorites.max(0) as u32,
        recently_added: recently_added.max(0) as u32,
        recently_played: recently_played.max(0) as u32,
    })
}

/// Counts for the sidebar tree, returned by `tree_counts`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TreeCounts {
    pub all_music: u32,
    pub albums: u32,
    pub artists: u32,
    pub genres: u32,
    pub favorites: u32,
    pub recently_added: u32,
    pub recently_played: u32,
}

/// Bundle all facet aggregations in a single round trip.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AllFacets {
    pub genres: Vec<(String, u32)>,
    pub years: Vec<(i32, u32)>,
    pub formats: Vec<(String, u32)>,
}

/// Run every facet aggregation in parallel and return the
/// bundle. v0.2.0 minimum: the queries are sequential for
/// simplicity; v0.2.0 hardening can use
/// `tokio::try_join!` to fan them out.
pub async fn all_facets(library: &Library) -> Result<AllFacets, sqlx::Error> {
    let genres = facet_genres(library).await?;
    let years = facet_years(library).await?;
    let formats = facet_formats(library).await?;
    Ok(AllFacets { genres, years, formats })
}

/// Sidebar tree counts plus facets in one round trip.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LibraryOverview {
    pub tree: TreeCounts,
    pub facets: AllFacets,
    pub today: crate::playback_log::DailyStat,
}

/// Combined overview for the Library screen mount sequence.
pub async fn overview(library: &Library) -> Result<LibraryOverview, sqlx::Error> {
    let tree = tree_counts(library).await?;
    let facets = all_facets(library).await?;
    let today = crate::playback_log::today_stats(library).await?;
    Ok(LibraryOverview { tree, facets, today })
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Library;
    use crate::schema::Codec;
    use crate::test_lock::ENV_LOCK;
    use muzon_core::MuzonPaths;

    /// Per-test wrapper: insert a few tracks and return the
    /// library AND the tempdir (so the test holds the tempdir
    /// alive for the duration of the test). Uses
    /// `Library::open_at` directly (not MUZON_HOME) so the test
    /// does not depend on the env-var test isolation.
    async fn setup_library() -> (tempfile::TempDir, Library) {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        // Insert 3 tracks: 1 flac, 1 mp3, 1 ogg.
        for (path, codec) in [
            ("/tmp/a.flac", Codec::Flac),
            ("/tmp/b.mp3", Codec::Mp3),
            ("/tmp/c.ogg", Codec::Ogg),
        ] {
            let id = lib
                .upsert_track_path(path, 1024, codec)
                .await
                .expect("upsert");
            lib.update_fts(id, "Bohemian Rhapsody", "Queen", "A Night at the Opera", "", path)
                .await
                .expect("update fts");
        }
        (tmp, lib)
    }

    #[tokio::test]
    async fn fts_returns_matching_tracks() {
        let (_tmp, lib) = setup_library().await;
        let hits = fts(&lib, "\"Bohemian\"", 10).await.expect("fts");
        // All 3 seeded tracks have title "Bohemian Rhapsody" so
        // the FTS5 query returns all 3 rowids.
        assert_eq!(hits.len(), 3);
        let paths: Vec<&str> = hits.iter().map(|h| h.path.as_str()).collect();
        assert!(paths.contains(&"/tmp/a.flac"));
        assert!(paths.contains(&"/tmp/b.mp3"));
        assert!(paths.contains(&"/tmp/c.ogg"));
        for hit in &hits {
            assert_eq!(hit.title.as_deref(), Some("Bohemian Rhapsody"));
        }
    }

    #[tokio::test]
    async fn fts_zero_results_for_missing_token() {
        let (_tmp, lib) = setup_library().await;
        let hits = fts(&lib, "\"NoSuchToken\"", 10).await.expect("fts");
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn facet_genres_aggregates_correctly() {
        let (_tmp, lib) = setup_library().await;
        let genres = facet_genres(&lib).await.expect("facet_genres");
        assert!(genres.is_empty());
    }

    #[tokio::test]
    async fn facet_formats_aggregates_codecs() {
        let (_tmp, lib) = setup_library().await;
        let formats = facet_formats(&lib).await.expect("facet_formats");
        let total: u32 = formats.iter().map(|(_, c)| c).sum();
        assert_eq!(total, 3);
        let names: Vec<&str> = formats.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"flac"));
        assert!(names.contains(&"mp3"));
        assert!(names.contains(&"ogg"));
    }

    #[tokio::test]
    async fn tree_counts_returns_seeded_numbers() {
        let (_tmp, lib) = setup_library().await;
        let counts = tree_counts(&lib).await.expect("tree_counts");
        assert_eq!(counts.all_music, 3);
        assert_eq!(counts.albums, 0);
        assert_eq!(counts.artists, 0);
        assert_eq!(counts.genres, 0);
        assert_eq!(counts.favorites, 0);
        assert_eq!(counts.recently_added, 3);
        assert_eq!(counts.recently_played, 0);
    }
}
