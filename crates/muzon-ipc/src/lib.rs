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

pub mod client;
pub mod library;
pub mod playback;
pub mod queue;
pub mod request;
pub mod server;
pub mod settings;
pub mod skin;

pub use client::{ClientError, IpcClient};
pub use library::{AlbumRef, AlbumWithTracks, ArtistRef, LibraryRequest, LibraryResponse, PlaylistRef};
pub use playback::{
    PlaybackRequest, PlaybackResponse, PlaybackStatus, PlayingState, TrackRef,
};
pub use queue::{QueueEntry, QueueRequest, QueueSnapshot, RepeatMode};
pub use request::{IpcRequest, IpcResponse};
pub use server::{default_socket_path, ensure_socket_absent, serve, set_socket_perms, ServerError};
pub use settings::{SettingsRequest, SettingsResponse, SettingsSnapshot};
pub use skin::{SkinCssPayload, SkinInfo, SkinRequest, SkinResponse};
