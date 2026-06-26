// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon core crate.
//!
//! Leaf crate in the workspace graph: every other Muzon crate depends on
//! this one. Owns the cross-cutting domain types, error types, XDG path
//! resolution, TOML config loading, and the `tracing` setup. Real
//! implementations land in issues 0006+.

pub mod config;
pub mod logging;
pub mod paths;

pub use config::{
    AccentColor, AudioConfig, ConfigError, LibraryConfig, LoggingConfig, MuzonConfig,
    NetworkConfig, ReplayGainMode, Theme, ThemeConfig, ThemeMode, UiConfig,
};
pub use logging::{init_logging, LoggingError, LoggingGuard};
pub use paths::{MuzonPaths, PathsError};
