// SPDX-License-Identifier: MIT OR Apache-2.0
//! Minimal `muzon --print-paths` entry point.
//!
//! This binary is intentionally tiny in 0006: it resolves the XDG
//! paths via `muzon-core` and prints them as `key=value` lines. It
//! is the testable artifact that proves the `MuzonPaths` contract
//! from issue 0006 holds end-to-end. The full clap-driven CLI
//! (`muzon play <file>`, `muzon library scan`, etc.) lands in
//! issue 0009; this binary becomes the `--print-paths` flag of
//! the real binary then.

use std::process::ExitCode;

use muzon_core::MuzonPaths;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("muzon: --print-paths failed: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths = MuzonPaths::resolve()?;
    println!("config_dir={}", paths.config_dir.display());
    println!("data_dir={}", paths.data_dir.display());
    println!("cache_dir={}", paths.cache_dir.display());
    println!("log_dir={}", paths.log_dir.display());
    println!("config_file={}", paths.config_file().display());
    println!("log_file={}", paths.log_file().display());
    Ok(())
}
