// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lofty` wrapper that maps lofty's type system to the project-local
//! `TagData`. Reads ID3v1/v2, Vorbis comments, APE, and MP4. v0.1.0
//! ships read-only tag reading; tag write-back is v0.4.0.
//!
//! See TZ.md §2.2.3.

use std::fs::File;
use std::path::Path;

use lofty::file::AudioFile;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::Accessor;

/// Project-local tag data extracted from a single audio file. Used
/// by the scanner in [`crate::scanner`] to populate the `tracks`
/// row after the path is upserted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagData {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: Option<u64>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub has_embedded_art: bool,
}

/// Read tags from `path`. Returns `Ok(TagData::default())` for files
/// that exist but have no tags (lofty returns a tagged file with
/// an empty tag set; we surface that as `default()` so the scanner
/// can upsert the path and skip the optional tag read).
pub fn read_tags(path: &Path) -> Result<TagData, lofty::error::LoftyError> {
    let tagged = Probe::open(path)?.read()?;
    let props = tagged.properties();
    let duration_ms: u64 = props
        .duration()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let bitrate = props.audio_bitrate();
    let sample_rate = props.sample_rate();
    let channels = props.channels();

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let mut data = TagData {
        duration_ms: Some(duration_ms),
        bitrate,
        sample_rate,
        channels,
        ..Default::default()
    };

    if let Some(tag) = tag {
        data.title = tag.title().map(|s| s.to_string());
        data.artist = tag.artist().map(|s| s.to_string());
        data.album_artist = tag
            .get_string(&lofty::tag::ItemKey::AlbumArtist)
            .map(|s| s.to_string());
        data.album = tag.album().map(|s| s.to_string());
        data.genre = tag.genre().map(|s| s.to_string());
        if let Some(year) = tag.year() {
            data.year = Some(year);
        }
        if let Some(tno) = tag.track() {
            data.track_no = Some(tno);
        }
        if let Some(dno) = tag.disk() {
            data.disc_no = Some(dno);
        }
        data.has_embedded_art = !tag.pictures().is_empty();
    }

    Ok(data)
}

/// Open `path` and return its size in bytes. Used by the scanner
/// to populate `tracks.size_bytes` without reading the full
/// file. Falls back to `File::metadata` for files lofty cannot
/// probe (corrupted, zero-byte).
pub fn file_size(path: &Path) -> std::io::Result<u64> {
    File::open(path)?.metadata().map(|m| m.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tags_missing_file_errors() {
        let result = read_tags(Path::new("/nonexistent/path.mp3"));
        assert!(result.is_err(), "expected error for missing file, got {result:?}");
    }
}
