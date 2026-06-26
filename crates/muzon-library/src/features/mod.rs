// SPDX-License-Identifier: MIT OR Apache-2.0
//! Audio feature extractors (BPM, Key, Loudness, Energy).
//!
//! v0.5.0 minimum: each extractor consumes a mono PCM
//! signal and returns a typed result. The Library's
//! extraction pipeline reads the decoded PCM via `symphonia`
//! (added in 0022) and runs the four extractors in sequence.
//!
//! See TZ §3.5.4 for the §2.4 MusicLab contract.

pub mod bpm;
pub mod energy;
pub mod key;
pub mod loudness;

pub use bpm::{extract_bpm, Bpm};
pub use energy::{compute_energy, EnergyFeatures};
pub use key::{extract_key, KeyEstimate};
pub use loudness::{extract_loudness_lufs, Loudness};
