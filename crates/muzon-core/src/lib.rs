// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon core crate.
//!
//! Leaf crate in the workspace graph: every other Muzon crate depends on
//! this one. Owns the cross-cutting domain types, error types, XDG path
//! resolution, TOML config loading, and the `tracing` setup. Real
//! implementations land in issues 0006+; this file is a stub for the
//! 0005 workspace skeleton.
