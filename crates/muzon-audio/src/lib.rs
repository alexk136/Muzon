// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon audio crate.
//!
//! Owns the GStreamer `playbin3` pipeline wrapper, gapless playback
//! via the `about-to-finish` signal, ReplayGain, crossfade, bit-perfect
//! output, and the visualization PCM feed. The first end-to-end
//! implementation lands in issue 0009; the queue model lands in
//! issue 0014.

pub mod engine;
pub mod queue;

pub use engine::{init, AudioEngine, AudioError};
pub use queue::Queue;
