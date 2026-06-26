// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon IPC crate.
//!
//! Owns the typed wire contract between the Tauri shell (0011)
//! and the headless `muzon-core` process. The same types are used
//! by the in-process variant in 0011 and the cross-process Unix
//! socket variant in 0012. Four domain modules cover the v0.2.0
//! surface; MusicLab, LLM, and scrobble modules land in their
//! respective milestones.
//!
//! See TZ.md §3.1.2 (process model), §3.6 (UI).

pub mod library;
pub mod playback;
pub mod queue;
pub mod skin;

pub use library::{AlbumRef, AlbumWithTracks, ArtistRef, LibraryRequest, LibraryResponse, PlaylistRef};
pub use playback::{
    PlaybackRequest, PlaybackResponse, PlaybackStatus, PlayingState, TrackRef,
};
pub use queue::{QueueEntry, QueueRequest, QueueSnapshot, RepeatMode};
pub use skin::{SkinCssPayload, SkinInfo, SkinRequest, SkinResponse};
