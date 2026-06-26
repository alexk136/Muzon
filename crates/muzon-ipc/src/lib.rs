// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon IPC crate.
//!
//! Owns the Unix-socket IPC layer (inside `XDG_RUNTIME_DIR`) used by
//! the headless `muzon-core` process to expose its control surface,
//! and the D-Bus surface (MPRIS2 + custom) used by other applications
//! on the system desktop. Real implementation lands in v0.2.0; this
//! file is a stub for the 0005 workspace skeleton.
