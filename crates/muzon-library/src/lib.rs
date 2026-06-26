// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon library crate.
//!
//! Owns the SQLite schema, migrations, FTS5 search, library scanner
//! (walkdir + lofty + notify), tag reading, the playback log, and
//! the audio-embedding pipeline. The schema and typed accessors
//! land in issue 0007; the scanner / watcher / tags land in 0008;
//! the search + facets + playback log land in 0013.

pub mod autotag;
pub mod db;
pub mod features;
pub mod musicbrainz;
pub mod path_parse;
pub mod playback_log;
pub mod scanner;
pub mod schema;
pub mod search;
pub mod tags;

pub use autotag::{AutoTagMatch, AutoTagger};
pub use musicbrainz::{Artist, MusicBrainzClient, MusicBrainzError, Recording, Release};

#[cfg(test)]
pub mod test_lock {
    //! Global test mutex for env-var- and database-file-touching
    //! tests. The crate has many tests that set `MUZON_HOME` to
    //! a fresh `tempdir` and then drop the dir at the end of the
    //! test. With cargo's default parallel execution, two tests
    //! in different modules can race on the env var or on the
    //! database file. This mutex serialises them.
    use std::sync::Mutex;
    pub static ENV_LOCK: Mutex<()> = Mutex::new(());
}

pub use db::Library;
pub use features::{
    extract_bpm, extract_key, extract_loudness_lufs, EnergyFeatures, KeyEstimate, Loudness,
};
pub use path_parse::{parse_path, ParsedPath};
pub use playback_log::{today_stats, DailyStat};
pub use scanner::{is_audio_file, ScanEvent, Scanner, DEBOUNCE_WINDOW};
pub use schema::{Codec, DbPool, Track};
pub use search::{
    all_facets, facet_formats, facet_genres, facet_years, fts, overview, tree_counts,
    AllFacets, LibraryOverview, TreeCounts,
};

