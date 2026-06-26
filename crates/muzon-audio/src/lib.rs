// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon audio crate.
//!
//! Owns the GStreamer `playbin3` pipeline wrapper, gapless playback
//! via the `about-to-finish` signal, ReplayGain, crossfade, bit-perfect
//! output, and the visualization PCM feed. The first end-to-end
//! implementation lands in issue 0009.

pub mod engine;

pub use engine::{init, AudioEngine, AudioError};
