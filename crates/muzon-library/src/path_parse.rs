// SPDX-License-Identifier: MIT OR Apache-2.0
//! Auto-detect `Artist / Album / Track` from a file's relative
//! path under a watched root. The patterns recognised in v0.1.0
//! match the §2.2.3 contract.
//!
//! Patterns:
//! - `Artist/Album/01 - Track.flac` (2 components above the file)
//! - `Artist/Year - Album/01 - Track.flac` (2 components)
//! - `Artist/Year - Album/CD1/01 - Track.flac` (3 components, CD
//!   subdir at the end)
//! - `Artist/Album/CD 1/01 - Track.flac` (3 components, CD subdir
//!   at the end, no year prefix)
//!
//! Any path with more than 3 components, or with a non-CD name in
//! the third slot, returns `None` and the caller falls back to file
//! tags.

use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedPath {
    pub artist: String,
    pub album: String,
    pub year: Option<u32>,
    pub disc_no: Option<u32>,
    pub track_no: u32,
    pub title: String,
}

/// Try to parse a relative path under `root` for the §2.2.3
/// patterns. Returns `None` if the path does not match.
pub fn parse_path(root: &Path, file: &Path) -> Option<ParsedPath> {
    let rel = file.strip_prefix(root).ok()?;
    let components: Vec<&str> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    if components.is_empty() {
        return None;
    }
    let (title_raw, dir_components) = components.split_last()?;
    let title = strip_extension(title_raw);
    if title.is_empty() {
        return None;
    }
    // We need 2 or 3 directory components above the file.
    if dir_components.len() < 2 || dir_components.len() > 3 {
        return None;
    }

    let artist = dir_components[0].to_string();
    let mut album_component = dir_components[1].to_string();
    let mut disc_no = None;

    if dir_components.len() == 3 {
        // The third component must be a CD subdir (CD1, CD 1, etc.).
        // Anything else means the path does not match the v0.1.0
        // patterns.
        let third = dir_components[2];
        match parse_disc(third) {
            Some(d) => disc_no = Some(d),
            None => return None,
        }
    }

    // Optional "Year - Album" prefix on the album component.
    let (album, year) = if let Some(idx) = album_component.find(" - ") {
        let candidate_year = album_component[..idx].trim();
        if let Ok(y) = candidate_year.parse::<u32>() {
            if (1900..=2100).contains(&y) {
                album_component = album_component[idx + 3..].to_string();
                (album_component, Some(y))
            } else {
                (album_component, None)
            }
        } else {
            (album_component, None)
        }
    } else {
        (album_component, None)
    };

    // Optional track_no prefix on the title.
    let (track_no, title) = parse_track_prefix(&title);

    Some(ParsedPath {
        artist,
        album,
        year,
        disc_no,
        track_no,
        title,
    })
}

fn strip_extension(s: &str) -> String {
    match s.rfind('.') {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
}

fn parse_disc(s: &str) -> Option<u32> {
    let lower = s.to_ascii_lowercase();
    let trimmed = lower.strip_prefix("cd")?.trim().trim_start_matches('0');
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u32>().ok()
}

fn parse_track_prefix(title: &str) -> (u32, String) {
    // "01 - Track" or "01. Track" or "01 Track" — the separator
    // between the track number and the title can be a " - ", ". ",
    // or a single space. We try the literal separators first; if
    // none match, we fall back to whitespace splitting.
    for sep in [" - ", ". ", " "] {
        if let Some((head, tail)) = title.split_once(sep) {
            if let Ok(track_no) = head.trim().parse::<u32>() {
                if (1..=999).contains(&track_no) {
                    return (track_no, tail.trim().to_string());
                }
            }
        }
    }
    (1, title.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(rel: &str) -> Option<ParsedPath> {
        let f = Path::new("/music").join(rel);
        parse_path(Path::new("/music"), &f)
    }

    #[test]
    fn artist_album_track() {
        let p = parse("Artist/Album/01 - Track.flac").expect("parsed");
        assert_eq!(p.artist, "Artist");
        assert_eq!(p.album, "Album");
        assert_eq!(p.track_no, 1);
        assert_eq!(p.title, "Track");
        assert_eq!(p.year, None);
        assert_eq!(p.disc_no, None);
    }

    #[test]
    fn year_and_disc() {
        let p = parse("Artist/2000 - Album/CD1/01 - Track.flac").expect("parsed");
        assert_eq!(p.artist, "Artist");
        assert_eq!(p.album, "Album");
        assert_eq!(p.year, Some(2000));
        assert_eq!(p.disc_no, Some(1));
        assert_eq!(p.track_no, 1);
        assert_eq!(p.title, "Track");
    }

    #[test]
    fn cd_space() {
        let p = parse("Artist/Album/CD 2/03 - Track.flac").expect("parsed");
        assert_eq!(p.disc_no, Some(2));
        assert_eq!(p.track_no, 3);
        assert_eq!(p.title, "Track");
    }

    #[test]
    fn nested_extra_dir_returns_none() {
        // Path with 4 components: Artist/Sub/Album/01 - Track.flac
        // does not match the recognised 2-or-3-component pattern;
        // the scanner falls back to tags.
        let p = parse("Artist/Sub/Album/01 - Track.flac");
        assert!(p.is_none(), "expected None for 4-component path, got {p:?}");
    }

    #[test]
    fn non_matching_returns_none() {
        let p = parse("random.mp3");
        assert!(p.is_none(), "expected None for top-level file, got {p:?}");
    }

    #[test]
    fn year_prefix_only() {
        let p = parse("Artist/1999 - Album/05 Track.flac").expect("parsed");
        assert_eq!(p.year, Some(1999));
        assert_eq!(p.album, "Album");
        assert_eq!(p.track_no, 5);
        assert_eq!(p.title, "Track");
    }

    #[test]
    fn disc_without_year() {
        let p = parse("Artist/Album/CD 1/01 - Track.flac").expect("parsed");
        assert_eq!(p.year, None);
        assert_eq!(p.disc_no, Some(1));
    }
}
