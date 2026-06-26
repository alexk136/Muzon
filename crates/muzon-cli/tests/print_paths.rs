// SPDX-License-Identifier: MIT OR Apache-2.0
//! End-to-end test for `muzon --print-paths`.
//!
//! Sets `MUZON_HOME` to a tempdir, runs the binary, and asserts the
//! six `key=value` lines it emits land under the expected layout.

use std::process::Command;

#[test]
fn print_paths_emits_six_lines() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = env!("CARGO_BIN_EXE_muzon");
    let output = Command::new(bin)
        .arg("--print-paths")
        .env("MUZON_HOME", tmp.path())
        .output()
        .expect("run muzon --print-paths");
    assert!(output.status.success(), "muzon --print-paths failed: {:?}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 6, "expected 6 key=value lines, got {lines:?}");
    assert!(lines[0].starts_with("config_dir="));
    assert!(lines[1].starts_with("data_dir="));
    assert!(lines[2].starts_with("cache_dir="));
    assert!(lines[3].starts_with("log_dir="));
    assert!(lines[4].starts_with("config_file="));
    assert!(lines[5].starts_with("log_file="));
}

#[test]
fn print_paths_lands_under_muzon_home() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = env!("CARGO_BIN_EXE_muzon");
    let output = Command::new(bin)
        .arg("--print-paths")
        .env("MUZON_HOME", tmp.path())
        .output()
        .expect("run muzon --print-paths");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tmp_str = tmp.path().to_str().expect("utf-8 path");
    let tmp_canonical = std::fs::canonicalize(tmp.path()).expect("canonicalize");
    let tmp_canonical_str = tmp_canonical.to_str().expect("utf-8 path");
    for line in stdout.lines() {
        let value = line.split_once('=').map(|(_, v)| v).unwrap_or("");
        let resolved = std::fs::canonicalize(value)
            .map(|p| p.to_str().unwrap_or("").to_string())
            .unwrap_or_else(|_| value.to_string());
        assert!(
            resolved.starts_with(tmp_canonical_str) || value.starts_with(tmp_str),
            "line {line:?} does not land under MUZON_HOME {tmp_str:?}; resolved={resolved:?}"
        );
    }
}
