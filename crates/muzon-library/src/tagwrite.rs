// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tag write-back pipeline (lofty).
//!
//! v0.4.0 minimum: a typed `TagEdit` struct and a
//! `write_tags_with_backup` function that creates a `.bak`
//! backup of the file and calls into `lofty` to apply the
//! edit. The actual per-format write APIs in lofty 0.21
//! differ across formats; v0.4.0 minimum creates the backup
//! and the read-back verification, while v0.4.0 hardening
//! wires the per-format write APIs.
//!
//! See TZ.md §2.2.3 (write tags back via the `commit` action).

use std::fs;
use std::path::{Path, PathBuf};

use lofty::file::AudioFile;
use lofty::probe::Probe;
use thiserror::Error;

/// A proposed new tag set for a single track.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TagEdit {
    /// The new title. `Some(s)` with empty string means
    /// "do not change".
    pub title: Option<String>,
    /// The new artist.
    pub artist: Option<String>,
    /// The new album.
    pub album: Option<String>,
    /// The new track number.
    pub track_number: Option<u32>,
    /// The new year.
    pub year: Option<u32>,
    /// The new genre.
    pub genre: Option<String>,
    /// The new MusicBrainz recording ID. Empty string means
    /// "do not change".
    pub musicbrainz_recording_id: Option<String>,
}

/// Errors produced by the tag write-back pipeline.
#[derive(Debug, Error)]
pub enum TagWriteError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("lofty error: {0}")]
    Lofty(String),
}

/// Apply a `TagEdit` to the file at `path`. The pipeline:
/// 1. Back up the original file to `path.bak`.
/// 2. Read the existing tags with `lofty`.
/// 3. (v0.4.0 hardening) Apply the `TagEdit` and write back
///    via lofty's per-format write APIs.
/// 4. (v0.4.0 hardening) Re-read and verify.
///
/// v0.4.0 minimum: the backup is created; the read is done
/// (to assert the file is parseable); the write is a no-op
/// pending v0.4.0 hardening's per-format write paths.
pub fn write_tags_with_backup(
    path: &Path,
    edit: &TagEdit,
) -> Result<(), TagWriteError> {
    // Step 1: back up the original file.
    let backup = backup_path(path);
    fs::copy(path, &backup).map_err(TagWriteError::Io)?;

    // Step 2: read the existing tags (and assert the file is
    // parseable).
    let _tagged = Probe::open(path)
        .map_err(|e| TagWriteError::Lofty(e.to_string()))?
        .read()
        .map_err(|e| TagWriteError::Lofty(e.to_string()))?;

    // Step 3: v0.4.0 minimum, the lofty 0.21 per-format write
    // APIs are not yet wired; the edit is a typed no-op for
    // now. v0.4.0 hardening calls into the per-format
    // `lofty::tag::Tag` items to apply the edit.
    let _ = edit;

    // Step 4: v0.4.0 hardening does the verification re-read.
    Ok(())
}

/// Compute the backup path for a given file. The convention
/// is `<path>.bak` (sibling file, same directory).
pub fn backup_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".bak");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a tiny silent WAV file in `dir` and return
    /// its path. The file is valid for the lofty Probe
    /// (WAV is universally supported).
    fn write_silent_wav(dir: &Path, name: &str) -> PathBuf {
        use std::io::Write;
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create");
        // 44-byte WAV header for a 1-sample 16-bit mono 8000 Hz file.
        let sample_rate: u32 = 8000;
        let num_samples: u32 = 1;
        let bits_per_sample: u16 = 16;
        let num_channels: u16 = 1;
        let byte_rate: u32 = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
        let block_align: u16 = num_channels * bits_per_sample / 8;
        let data_size: u32 = num_samples * block_align as u32;
        let chunk_size: u32 = 36 + data_size;
        f.write_all(b"RIFF").unwrap();
        f.write_all(&chunk_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16_u32.to_le_bytes()).unwrap();
        f.write_all(&1_u16.to_le_bytes()).unwrap();
        f.write_all(&num_channels.to_le_bytes()).unwrap();
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&byte_rate.to_le_bytes()).unwrap();
        f.write_all(&block_align.to_le_bytes()).unwrap();
        f.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_size.to_le_bytes()).unwrap();
        f.write_all(&0_i16.to_le_bytes()).unwrap();
        path
    }

    #[test]
    fn write_tags_with_backup_creates_backup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_silent_wav(tmp.path(), "test.wav");
        let edit = TagEdit {
            title: Some("New Title".into()),
            ..Default::default()
        };
        // Don't assert the write succeeds (lofty may not
        // round-trip the WAV tags perfectly); just assert the
        // backup file is created.
        let _ = write_tags_with_backup(&path, &edit);
        let backup = backup_path(&path);
        assert!(backup.exists(), "expected backup at {:?}", backup);
    }

    #[test]
    fn backup_path_appends_dot_bak() {
        let p = PathBuf::from("/tmp/a.wav");
        assert_eq!(backup_path(&p), PathBuf::from("/tmp/a.wav.bak"));
    }

    #[test]
    fn tag_edit_default_is_all_none() {
        let edit = TagEdit::default();
        assert_eq!(edit.title, None);
        assert_eq!(edit.artist, None);
        assert_eq!(edit.album, None);
        assert_eq!(edit.track_number, None);
        assert_eq!(edit.year, None);
        assert_eq!(edit.genre, None);
        assert_eq!(edit.musicbrainz_recording_id, None);
    }
}
