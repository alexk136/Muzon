// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon MusicLab crate.
//!
//! Owns the audio-analysis pipeline: fingerprinting (Chromaprint, v0.4.0),
//! BPM and Key detection, LUFS loudness, the tiered audio-embedding
//! strategy (CLAP primary, Panns secondary per decision 0004), UMAP/HDBSCAN
//! cluster maps, and the LLM provider abstraction (v0.6.0). Real
//! implementations land in v0.4.0 / v0.5.0 / v0.6.0; this file is a stub
//! for the 0005 workspace skeleton.
