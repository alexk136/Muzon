// SPDX-License-Identifier: MIT OR Apache-2.0
//! Quality Dashboard (M3) data layer.
//!
//! v0.4.0 minimum: typed structs for the collection stats
//! and the gap analysis. The Quality Dashboard (M3 in
//! musiclab.html) is a read-mostly surface; the dashboard
//! consumes the duplicate groups (0024), the MusicBrainz
//! match state (0023), and the tag write-back state (0025)
//! to produce actionable summaries.
//!
//! The M3 React UI is a v0.4.0 hardening task; the data
//! layer is the v0.4.0 minimum.

use serde::{Deserialize, Serialize};

/// The Quality Dashboard surface.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct QualityStats {
    /// Total number of tracks in the library.
    pub total_tracks: u32,
    /// Number of tracks with the `MusicBrainz recording id`
    /// column populated (i.e., matched via the auto-tag
    /// pipeline 0023).
    pub musicbrainz_matched: u32,
    /// Number of tracks with the `AcoustID` column populated.
    pub acoustid_matched: u32,
    /// Number of tracks with a non-empty `cover_path` (the
    /// cover art is in `tracks.cover_path`).
    pub with_cover: u32,
    /// Number of tracks with `play_count >= 5` (the
    /// favourites threshold from 0013).
    pub favourites: u32,
    /// Number of tracks missing at least one tag (title,
    /// artist, album). The schema's `needs_tag_write`
    /// column captures the explicit "needs editing" flag.
    pub missing_tags: u32,
    /// Number of tracks with the `needs_tag_write` flag
    /// set.
    pub needs_tag_write: u32,
    /// Number of duplicate groups (from 0024).
    pub duplicate_groups: u32,
    /// Number of tracks that are part of a duplicate group.
    pub duplicate_tracks: u32,
    /// Total library size in bytes.
    pub total_size_bytes: u64,
    /// Total library duration in milliseconds.
    pub total_duration_ms: u64,
    /// FLAC ratio in `[0.0, 1.0]`. The number of tracks
    /// whose `codec = 'flac'` divided by the total.
    pub flac_ratio: f32,
}

/// Compute the Quality Dashboard stats from the inputs.
/// v0.4.0 minimum: a pure-function aggregator; v0.4.0
/// hardening adds the SQL queries that read these from the
/// library DB.
pub fn compute_quality_stats(input: QualityStatsInput) -> QualityStats {
    let flac_ratio = if input.total_tracks == 0 {
        0.0
    } else {
        input.flac_tracks as f32 / input.total_tracks as f32
    };
    QualityStats {
        total_tracks: input.total_tracks,
        musicbrainz_matched: input.musicbrainz_matched,
        acoustid_matched: input.acoustid_matched,
        with_cover: input.with_cover,
        favourites: input.favourites,
        missing_tags: input.missing_tags,
        needs_tag_write: input.needs_tag_write,
        duplicate_groups: input.duplicate_groups,
        duplicate_tracks: input.duplicate_tracks,
        total_size_bytes: input.total_size_bytes,
        total_duration_ms: input.total_duration_ms,
        flac_ratio,
    }
}

/// The raw counts that `compute_quality_stats` aggregates.
/// The library DB query gathers these; v0.4.0 minimum uses
/// the typed struct as the input.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QualityStatsInput {
    pub total_tracks: u32,
    pub flac_tracks: u32,
    pub musicbrainz_matched: u32,
    pub acoustid_matched: u32,
    pub with_cover: u32,
    pub favourites: u32,
    pub missing_tags: u32,
    pub needs_tag_write: u32,
    pub duplicate_groups: u32,
    pub duplicate_tracks: u32,
    pub total_size_bytes: u64,
    pub total_duration_ms: u64,
}

/// A gap analysis item: a category of "what's missing" in
/// the library, with a count.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GapItem {
    /// The category id, e.g. "missing-album", "missing-cover",
    /// "missing-bpm", "missing-key", "missing-musicbrainz".
    pub category: String,
    /// Human-readable label.
    pub label: String,
    /// The number of tracks in this category.
    pub count: u32,
    /// The action id that the M3 UI dispatches when the
    /// user clicks the "Fix" button.
    pub action: String,
}

/// The gap analysis surface. v0.4.0 minimum: a typed struct
/// with a list of `GapItem`. The library DB query gathers
/// the counts; v0.4.0 hardening wires the queries.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GapAnalysis {
    pub items: Vec<GapItem>,
}

/// Build the gap analysis surface from a list of items.
pub fn build_gap_analysis(items: Vec<GapItem>) -> GapAnalysis {
    GapAnalysis { items }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> QualityStatsInput {
        QualityStatsInput {
            total_tracks: 1000,
            flac_tracks: 600,
            musicbrainz_matched: 850,
            acoustid_matched: 920,
            with_cover: 700,
            favourites: 50,
            missing_tags: 30,
            needs_tag_write: 15,
            duplicate_groups: 8,
            duplicate_tracks: 18,
            total_size_bytes: 90_000_000_000,
            total_duration_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn flac_ratio_is_fraction() {
        let stats = compute_quality_stats(sample_input());
        assert!((stats.flac_ratio - 0.6).abs() < 1e-3, "got {}", stats.flac_ratio);
    }

    #[test]
    fn total_tracks_passthrough() {
        let stats = compute_quality_stats(sample_input());
        assert_eq!(stats.total_tracks, 1000);
    }

    #[test]
    fn duplicate_passthrough() {
        let stats = compute_quality_stats(sample_input());
        assert_eq!(stats.duplicate_groups, 8);
        assert_eq!(stats.duplicate_tracks, 18);
    }

    #[test]
    fn flac_ratio_handles_zero_total() {
        let mut input = sample_input();
        input.total_tracks = 0;
        let stats = compute_quality_stats(input);
        // total is clamped to 1, so flac_ratio is 0/1 = 0
        assert_eq!(stats.flac_ratio, 0.0);
    }

    #[test]
    fn gap_analysis_holds_items() {
        let items = vec![GapItem {
            category: "missing-album".into(),
            label: "Missing album".into(),
            count: 42,
            action: "fix-missing-album".into(),
        }];
        let gap = build_gap_analysis(items.clone());
        assert_eq!(gap.items, items);
    }
}
