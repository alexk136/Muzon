// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mood map: UMAP 2D projection + HDBSCAN clustering.
//!
//! v0.5.0 minimum: a typed `MoodMap` struct + a
//! `project_and_cluster` function that takes a list of
//! feature vectors and produces a list of `(track_id, x, y,
//! cluster_id)` tuples. v0.5.0 minimum returns a
//! deterministic placeholder (a function of the input);
//! v0.5.0 hardening wires the real `fast-umap` and `hdbscan`
//! crates.
//!
//! The mood map is the central surface for the Cinematic
//! MusicLab variant (M4) — the 2D cluster visualization
//! with filter, cluster, and AI sections.

use serde::{Deserialize, Serialize};

/// A single point in the mood map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoodPoint {
    /// The track id.
    pub track_id: i64,
    /// The 2D x coordinate (UMAP output).
    pub x: f32,
    /// The 2D y coordinate.
    pub y: f32,
    /// The cluster id from HDBSCAN. -1 means "noise" (not in
    /// any cluster).
    pub cluster_id: i32,
}

/// The mood map result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoodMap {
    pub points: Vec<MoodPoint>,
    /// The number of distinct clusters (excluding noise).
    pub cluster_count: i32,
}

/// The UMAP parameters. v0.5.0 minimum defaults are sensible
/// for collections of a few hundred to a few thousand
/// tracks; v0.5.0 hardening tunes these for collections in
/// the millions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UmapParams {
    pub n_neighbors: usize,
    pub n_components: usize,
    pub min_dist: f32,
    pub n_epochs: usize,
}

impl Default for UmapParams {
    fn default() -> Self {
        Self {
            n_neighbors: 15,
            n_components: 2,
            min_dist: 0.1,
            n_epochs: 200,
        }
    }
}

/// The HDBSCAN parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HdbscanParams {
    pub min_cluster_size: usize,
    pub min_samples: usize,
}

impl Default for HdbscanParams {
    fn default() -> Self {
        Self {
            min_cluster_size: 5,
            min_samples: 3,
        }
    }
}

/// Project the feature vectors to 2D and cluster them. v0.5.0
/// minimum returns a deterministic placeholder (a function
/// of the input); v0.5.0 hardening wires the real `fast-umap`
/// and `hdbscan` crates.
pub fn project_and_cluster(
    feature_vectors: &[(i64, Vec<f32>)],
    umap_params: UmapParams,
    hdbscan_params: HdbscanParams,
) -> Result<MoodMap, MoodError> {
    if feature_vectors.is_empty() {
        return Ok(MoodMap::default());
    }

    // Deterministic 2D projection: each point gets a
    // coordinate derived from its position in the list. v0.5.0
    // minimum; v0.5.0 hardening replaces with the real UMAP
    // call.
    let n = feature_vectors.len() as f32;
    let points: Vec<MoodPoint> = feature_vectors
        .iter()
        .enumerate()
        .map(|(i, (id, _))| {
            let t = (i as f32) / n.max(1.0);
            let x = (t * std::f32::consts::TAU).cos();
            let y = (t * std::f32::consts::TAU).sin();
            MoodPoint {
                track_id: *id,
                x,
                y,
                cluster_id: -1,
            }
        })
        .collect();

    // Deterministic clustering: points whose index is in
    // [0, hdbscan_params.min_cluster_size) are cluster 0,
    // [min_cluster_size, 2*min) are cluster 1, etc. v0.5.0
    // minimum; v0.5.0 hardening replaces with the real HDBSCAN
    // call.
    let min_cluster_size = hdbscan_params.min_cluster_size.max(1);
    let mut points = points;
    let mut cluster_count: i32 = 0;
    for (i, point) in points.iter_mut().enumerate() {
        point.cluster_id = (i / min_cluster_size) as i32;
        if (i / min_cluster_size) as i32 >= cluster_count {
            cluster_count = (i / min_cluster_size) as i32 + 1;
        }
    }
    Ok(MoodMap {
        points,
        cluster_count,
    })
}

/// Errors produced by the mood map.
#[derive(Debug, thiserror::Error)]
pub enum MoodError {
    #[error("umap error: {0}")]
    Umap(String),

    #[error("hdbscan error: {0}")]
    Hdbscan(String),

    #[error("shape mismatch: expected {expected} points, got {actual}")]
    ShapeMismatch { expected: usize, actual: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty_map() {
        let map = project_and_cluster(&[], UmapParams::default(), HdbscanParams::default())
            .expect("ok");
        assert!(map.points.is_empty());
        assert_eq!(map.cluster_count, 0);
    }

    #[test]
    fn single_point_returns_one_with_cluster_id_zero() {
        let pts = vec![(1_i64, vec![0.0_f32, 0.0, 0.0])];
        let map = project_and_cluster(&pts, UmapParams::default(), HdbscanParams::default())
            .expect("ok");
        assert_eq!(map.points.len(), 1);
        assert_eq!(map.points[0].cluster_id, 0);
    }

    #[test]
    fn two_clusters_with_min_cluster_size_three() {
        let pts = vec![
            (1_i64, vec![0.0_f32, 0.0]),
            (2_i64, vec![1.0_f32, 0.0]),
            (3_i64, vec![0.0_f32, 1.0]),
            (4_i64, vec![10.0_f32, 10.0]),
            (5_i64, vec![11.0_f32, 10.0]),
            (6_i64, vec![10.0_f32, 11.0]),
        ];
        let params = HdbscanParams {
            min_cluster_size: 3,
            min_samples: 1,
        };
        let map = project_and_cluster(&pts, UmapParams::default(), params).expect("ok");
        assert_eq!(map.cluster_count, 2);
    }

    #[test]
    fn deterministic_output() {
        let pts = vec![(1_i64, vec![0.0_f32; 3]), (2_i64, vec![1.0_f32; 3])];
        let m1 = project_and_cluster(&pts, UmapParams::default(), HdbscanParams::default())
            .expect("ok");
        let m2 = project_and_cluster(&pts, UmapParams::default(), HdbscanParams::default())
            .expect("ok");
        assert_eq!(m1.points, m2.points);
    }
}
