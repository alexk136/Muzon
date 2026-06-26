// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon library crate.
//!
//! Owns the SQLite schema, migrations, FTS5 search, library scanner
//! (walkdir + lofty + notify), tag reading, and the audio-embedding
//! pipeline. The schema and typed accessors land in issue 0007; the
//! scanner / watcher / tags land in issue 0008.

pub mod db;
pub mod path_parse;
pub mod scanner;
pub mod schema;
pub mod tags;

pub use db::Library;
pub use path_parse::{parse_path, ParsedPath};
pub use scanner::{is_audio_file, ScanEvent, Scanner, DEBOUNCE_WINDOW};
pub use schema::{Codec, DbPool, Track};

