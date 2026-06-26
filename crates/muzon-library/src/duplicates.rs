// SPDX-License-Identifier: MIT OR Apache-2.0
//! Duplicate detection engine.
//!
//! v0.4.0 minimum: a typed `DuplicateGroup` struct, a
//! `Strategy` enum, and a `find_duplicates` function that
//! takes a list of `TrackForDedup` and produces a list of
//! `DuplicateGroup`. The strategies are:
//!
//! 1. **Fingerprint exact** — same Chromaprint (0022).
//! 2. **Metadata + duration** — same (title, artist, album)
//!    and duration within ±2 seconds.
//! 3. **Content hash** — same file hash (v0.4.0 minimum:
//!    same `(size, mtime)` as a placeholder; the real
//!    content hash is a v0.4.0 hardening addition).
//!
//! v0.4.0 minimum: the engine runs all three and produces a
//! unified group model. The Quality Dashboard (0026) reads
//! these groups to surface the "X duplicate groups found"
//! stats.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A track record for the duplicate-detection engine. The
/// fields are the ones that the three strategies need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackForDedup {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub size_bytes: i64,
    pub mtime: i64,
    /// The Chromaprint fingerprint (0022). Empty string
    /// means "no fingerprint available".
    pub fingerprint: String,
}

/// The strategy that matched a duplicate group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Same Chromaprint fingerprint.
    FingerprintExact,
    /// Same metadata (title + artist + album) and duration
    /// within ±2 s.
    MetadataDuration,
    /// Same file hash. v0.4.0 minimum: same `(size, mtime)`
    /// as a placeholder; v0.4.0 hardening replaces with a
    /// real content hash.
    #[default]
    ContentHash,
}

impl Strategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Strategy::FingerprintExact => "fingerprint_exact",
            Strategy::MetadataDuration => "metadata_duration",
            Strategy::ContentHash => "content_hash",
        }
    }
}

/// A group of duplicate tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// The matching strategy.
    pub strategy: Strategy,
    /// The track IDs in the group (2 or more).
    pub track_ids: Vec<i64>,
}

impl DuplicateGroup {
    /// The number of tracks in the group.
    pub fn len(&self) -> usize {
        self.track_ids.len()
    }

    /// True if the group has at least 2 tracks.
    pub fn is_duplicate(&self) -> bool {
        self.track_ids.len() >= 2
    }
}

/// Find duplicates in a list of tracks. Runs all three
/// strategies and returns a unified list of duplicate groups.
pub fn find_duplicates(tracks: &[TrackForDedup]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();

    // Strategy 1: fingerprint exact.
    let mut by_fp: HashMap<&str, Vec<i64>> = HashMap::new();
    for t in tracks {
        if t.fingerprint.is_empty() {
            continue;
        }
        by_fp.entry(t.fingerprint.as_str()).or_default().push(t.id);
    }
    for ids in by_fp.values() {
        if ids.len() >= 2 {
            groups.push(DuplicateGroup {
                strategy: Strategy::FingerprintExact,
                track_ids: ids.clone(),
            });
        }
    }

    // Strategy 2: metadata + duration ±2 s.
    let mut by_meta: HashMap<(String, String, String), Vec<i64>> = HashMap::new();
    for t in tracks {
        if t.title.is_empty() || t.artist.is_empty() {
            continue;
        }
        let key = (t.title.clone(), t.artist.clone(), t.album.clone());
        by_meta.entry(key).or_default().push(t.id);
    }
    for (_key, ids) in by_meta {
        if ids.len() >= 2 && same_duration(tracks, &ids) {
            groups.push(DuplicateGroup {
                strategy: Strategy::MetadataDuration,
                track_ids: ids,
            });
        }
    }

    // Strategy 3: content hash (= same size + mtime in v0.4.0
    // minimum).
    let mut by_hash: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
    for t in tracks {
        if t.size_bytes <= 0 {
            continue;
        }
        by_hash.entry((t.size_bytes, t.mtime)).or_default().push(t.id);
    }
    for ids in by_hash.values() {
        if ids.len() >= 2 {
            groups.push(DuplicateGroup {
                strategy: Strategy::ContentHash,
                track_ids: ids.clone(),
            });
        }
    }

    groups
}

fn same_duration(tracks: &[TrackForDedup], ids: &[i64]) -> bool {
    const TOLERANCE_MS: u64 = 2000;
    let durations: Vec<u64> = tracks
        .iter()
        .filter(|t| ids.contains(&t.id))
        .map(|t| t.duration_ms)
        .collect();
    if durations.len() < 2 {
        return false;
    }
    let min = *durations.iter().min().unwrap();
    let max = *durations.iter().max().unwrap();
    max - min <= TOLERANCE_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(
        id: i64,
        title: &str,
        artist: &str,
        album: &str,
        duration: u64,
        fp: &str,
    ) -> TrackForDedup {
        TrackForDedup {
            id,
            path: format!("/tmp/{title}-{id}.flac"),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            duration_ms: duration,
            size_bytes: 1024 * (id as i64 + 1),
            mtime: 1_700_000_000 + id,
            fingerprint: fp.to_string(),
        }
    }

    #[test]
    fn no_duplicates_when_all_unique() {
        let tracks = vec![
            make_track(1, "A", "X", "Al", 200_000, "fp1"),
            make_track(2, "B", "Y", "Al", 180_000, "fp2"),
        ];
        let groups = find_duplicates(&tracks);
        assert!(groups.is_empty());
    }

    #[test]
    fn fingerprint_exact_detects_duplicates() {
        let tracks = vec![
            make_track(1, "A", "X", "Al", 200_000, "fp_same"),
            make_track(2, "B", "Y", "Al", 180_000, "fp_same"),
        ];
        let groups = find_duplicates(&tracks);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].strategy, Strategy::FingerprintExact);
        assert_eq!(groups[0].track_ids, vec![1, 2]);
    }

    #[test]
    fn content_hash_detects_duplicates() {
        let mut t1 = make_track(1, "A", "X", "Al", 200_000, "");
        let mut t2 = make_track(2, "B", "Y", "Al", 180_000, "");
        t1.fingerprint = String::new();
        t2.fingerprint = String::new();
        t1.size_bytes = 5000;
        t2.size_bytes = 5000;
        t1.mtime = 12345;
        t2.mtime = 12345;
        let groups = find_duplicates(&[t1, t2]);
        assert!(groups.iter().any(|g| g.strategy == Strategy::ContentHash));
    }

    #[test]
    fn metadata_duration_detects_duplicates() {
        let tracks = vec![
            make_track(1, "Same", "Same", "Same", 200_000, ""),
            make_track(2, "Same", "Same", "Same", 200_000, ""),
        ];
        let groups = find_duplicates(&tracks);
        assert!(groups.iter().any(|g| g.strategy == Strategy::MetadataDuration));
    }
}
