// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon MusicLab crate.
//!
//! Owns the audio-fingerprint pipeline (Chromaprint FFI,
//! AcoustID HTTP client), the tiered audio-embedding
//! strategy (CLAP primary, Panns secondary per decision
//! 0004), UMAP / HDBSCAN mood-map clustering, the LLM
//! provider abstraction (Ollama, OpenAI, Anthropic,
//! OpenRouter per TZ §3.5.4), and the MusicLab feature
//! surface that the 4 MusicLab screen variants
//! (musiclab.html m1-m4) consume.
//!
//! v0.4.0 minimum: the fingerprint pipeline + the AcoustID
//! client stub + the LLM provider abstraction types.

pub mod acoustid;
pub mod fingerprint;
pub mod llm;

pub use acoustid::{AcoustIdClient, AcoustIdMatch};
pub use fingerprint::{compute_fingerprint, Fingerprint, FingerprintAlgorithm};
pub use llm::{LlmProvider, LlmRequest, LlmResponse};
