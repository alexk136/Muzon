// SPDX-License-Identifier: MIT OR Apache-2.0
//! `muzon-app` binary entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    muzon_app_lib::run().expect("muzon-app: tauri runtime failed");
}
