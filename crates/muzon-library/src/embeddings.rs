// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tiered embedding pipeline (LAION-CLAP + Panns).
//!
//! v0.5.0 minimum: a typed `Embedding` struct, a `Model`
//! enum (CLAP or Panns), a `compute_embedding` function
//! that takes raw PCM + a model choice and returns the
//! typed embedding. v0.5.0 minimum returns a deterministic
//! placeholder embedding (a function of the input length and
//! the model id); v0.5.0 hardening wires the real ONNX
//! Runtime (`ort` crate) and loads the local ONNX model
//! files.
//!
//! The decision is in `docs/decisions/0004-audio-embedding-model.md`:
//! CLAP (LAION-CLAP HTSAT-base) as the primary model for
//! text-aligned + similarity queries, Panns_inference as the
//! secondary for instrument/mood tag queries.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Which model produced the embedding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    /// LAION-CLAP HTSAT-base (Apache-2.0). Text-aligned +
    /// similarity queries.
    #[default]
    Clap,
    /// Panns_inference (MIT). Instrument / mood tag queries.
    Panns,
}

impl Model {
    /// The dimensionality of the embedding vector.
    pub fn dimensions(&self) -> usize {
        match self {
            Model::Clap => 512,
            Model::Panns => 2048,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Model::Clap => "clap",
            Model::Panns => "panns",
        }
    }
}

/// A single embedding vector.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub model: Model,
    pub vector: Vec<f32>,
}

impl Embedding {
    /// The number of dimensions.
    pub fn len(&self) -> usize {
        self.vector.len()
    }

    /// True if the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.vector.is_empty()
    }

    /// Cosine similarity with another vector. Both vectors
    /// must have the same length.
    pub fn cosine(&self, other: &Embedding) -> f32 {
        if self.vector.len() != other.vector.len() {
            return 0.0;
        }
        let dot: f32 = self
            .vector
            .iter()
            .zip(&other.vector)
            .map(|(a, b)| a * b)
            .sum();
        let norm_a: f32 = self.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

/// Compute an embedding for a raw PCM signal. v0.5.0 minimum
/// returns a deterministic placeholder (a function of the
/// input length and the model id); v0.5.0 hardening wires
/// the real ONNX Runtime call.
pub fn compute_embedding(samples: &[f32], model: Model) -> Embedding {
    let dims = model.dimensions();
    let mut vector = Vec::with_capacity(dims);
    for i in 0..dims {
        // Deterministic placeholder: a function of the
        // model id (so different models produce different
        // embeddings), the index, and the input length.
        let seed = (model.as_str().len() as f32) * 0.13
            + (i as f32) * 0.07
            + (samples.len() as f32) * 0.00001;
        vector.push(seed.sin());
    }
    Embedding { model, vector }
}

/// Estimate the time to embed a single track. v0.5.0 minimum
/// returns a placeholder based on the model id; v0.5.0
/// hardening returns the actual ONNX inference time.
pub fn estimate_embed_time(model: Model) -> Duration {
    match model {
        Model::Clap => Duration::from_millis(450),
        Model::Panns => Duration::from_millis(200),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_match_model() {
        assert_eq!(Model::Clap.dimensions(), 512);
        assert_eq!(Model::Panns.dimensions(), 2048);
    }

    #[test]
    fn compute_embedding_returns_correct_dimensions() {
        let samples = vec![0.0_f32; 1000];
        let e_clap = compute_embedding(&samples, Model::Clap);
        let e_panns = compute_embedding(&samples, Model::Panns);
        assert_eq!(e_clap.len(), 512);
        assert_eq!(e_panns.len(), 2048);
    }

    #[test]
    fn compute_embedding_is_deterministic() {
        let samples = vec![0.0_f32; 1000];
        let e1 = compute_embedding(&samples, Model::Clap);
        let e2 = compute_embedding(&samples, Model::Clap);
        assert_eq!(e1.vector, e2.vector);
    }

    #[test]
    fn compute_embedding_differs_by_model() {
        let samples = vec![0.0_f32; 1000];
        let e_clap = compute_embedding(&samples, Model::Clap);
        let e_panns = compute_embedding(&samples, Model::Panns);
        assert_ne!(e_clap.vector, e_panns.vector);
    }

    #[test]
    fn cosine_self_is_one() {
        let e = compute_embedding(&[0.0_f32; 100], Model::Clap);
        assert!((e.cosine(&e) - 1.0).abs() < 1e-5, "got {}", e.cosine(&e));
    }

    #[test]
    fn cosine_different_dimensions_returns_zero() {
        let e_clap = compute_embedding(&[0.0_f32; 100], Model::Clap);
        let e_panns = compute_embedding(&[0.0_f32; 100], Model::Panns);
        assert_eq!(e_clap.cosine(&e_panns), 0.0);
    }

    #[test]
    fn estimate_embed_time_is_positive() {
        assert!(estimate_embed_time(Model::Clap).as_millis() > 0);
        assert!(estimate_embed_time(Model::Panns).as_millis() > 0);
    }
}
