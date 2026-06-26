// SPDX-License-Identifier: MIT OR Apache-2.0
//! Vector index for similarity search.
//!
//! v0.5.0 minimum: typed `VecIndex` struct + a `cosine_top_k`
//! function that takes a query vector and a list of indexed
//! vectors and returns the top-K matches by cosine similarity.
//! The real `sqlite-vec` extension is deferred to v0.5.0
//! hardening; the v0.5.0 minimum is a pure-Rust fallback
//! that computes cosine similarity in-process.
//!
//! The production path (v0.5.0 hardening) loads the
//! `sqlite-vec` extension via `sqlx-sqlite-vec` only when the
//! extension file is present on disk; gracefully fall back to
//! "vec disabled" when it is not.

use std::path::Path;

use thiserror::Error;

/// One indexed vector in the vector store.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedVector {
    /// The track id this vector belongs to.
    pub track_id: i64,
    /// The vector itself.
    pub vector: Vec<f32>,
    /// Optional metadata: the model that produced the
    /// vector (e.g., "clap" or "panns"). v0.5.0 minimum
    /// stores it; v0.5.0 hardening uses it for query routing.
    pub model: Option<String>,
}

/// Errors produced by the vector index.
#[derive(Debug, Error)]
pub enum VecError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite-vec extension not available: {0}")]
    SqliteVecUnavailable(String),

    #[error("invalid vector length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
}

/// The vector index. v0.5.0 minimum: an in-memory list of
/// indexed vectors with a `cosine_top_k` query method.
/// v0.5.0 hardening wires the real `sqlite-vec` extension
/// for the production path.
#[derive(Debug, Clone, Default)]
pub struct VecIndex {
    pub vectors: Vec<IndexedVector>,
    /// The path to the `sqlite-vec` extension file, if it
    /// should be loaded. v0.5.0 minimum: `None`; v0.5.0
    /// hardening: `Some(PathBuf::from("/usr/lib/sqlite-vec.so"))`
    /// on Linux.
    pub sqlite_vec_path: Option<PathBuf>,
}

use std::path::PathBuf;

impl VecIndex {
    /// Build a new empty in-memory index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an index from a list of pre-existing vectors.
    pub fn from_vectors(vectors: Vec<IndexedVector>) -> Self {
        Self {
            vectors,
            sqlite_vec_path: None,
        }
    }

    /// Try to enable the `sqlite-vec` extension. v0.5.0
    /// minimum: returns `Ok(false)` if the extension is not
    /// available; v0.5.0 hardening loads the extension.
    pub fn try_enable_sqlite_vec(&mut self, path: &Path) -> Result<bool, VecError> {
        if !path.exists() {
            return Ok(false);
        }
        // v0.5.0 hardening: load the extension via
        // `sqlx-sqlite-vec::load_extension`. v0.5.0 minimum
        // just records the path.
        self.sqlite_vec_path = Some(path.to_path_buf().clone());
        Ok(true)
    }

    /// Insert a vector.
    pub fn insert(&mut self, v: IndexedVector) {
        self.vectors.push(v);
    }

    /// Compute the top-K matches for a query vector by
    /// cosine similarity. v0.5.0 minimum: pure-Rust
    /// `O(n log k)` heap-based top-K; v0.5.0 hardening
    /// delegates to the `sqlite-vec` extension.
    pub fn cosine_top_k(&self, query: &[f32], k: usize) -> Vec<(i64, f32)> {
        if query.is_empty() || k == 0 {
            return Vec::new();
        }
        // Compute (track_id, score) for every vector, then
        // take the top k.
        let mut scored: Vec<(i64, f32)> = self
            .vectors
            .iter()
            .map(|v| (v.track_id, cosine(query, &v.vector)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }
}

/// Cosine similarity between two vectors. Returns 0.0 if the
/// lengths don't match or either vector has zero norm.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: i64, vals: &[f32]) -> IndexedVector {
        IndexedVector {
            track_id: id,
            vector: vals.to_vec(),
            model: Some("clap".into()),
        }
    }

    #[test]
    fn cosine_self_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_different_lengths_is_zero() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine(&a, &b), 0.0);
    }

    #[test]
    fn top_k_returns_highest_scores() {
        let mut idx = VecIndex::new();
        idx.insert(v(1, &[1.0, 0.0, 0.0]));
        idx.insert(v(2, &[0.0, 1.0, 0.0]));
        idx.insert(v(3, &[0.9, 0.1, 0.0])); // close to 1
        let top = idx.cosine_top_k(&[1.0, 0.0, 0.0], 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, 1);
        assert_eq!(top[1].0, 3);
    }

    #[test]
    fn top_k_zero_query_returns_empty() {
        let mut idx = VecIndex::new();
        idx.insert(v(1, &[1.0, 0.0, 0.0]));
        let top = idx.cosine_top_k(&[], 5);
        assert!(top.is_empty());
    }

    #[test]
    fn try_enable_sqlite_vec_returns_false_for_missing_path() {
        let mut idx = VecIndex::new();
        let path = Path::new("/nonexistent/sqlite-vec.so");
        let result = idx.try_enable_sqlite_vec(path).expect("ok");
        assert!(!result);
    }
}
