// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skin IPC contract.
//!
//! Wire types for the skin domain. v0.2.0 ships the 4 built-in
//! skins from 0010; the real loader for user-installed skins
//! (ZIP / dir) lands in 0016. The IPC surface is the same in
//! both cases; the `ListInstalled` and `GetActive` commands
//! return the same shape.
//!
//! See TZ.md §2.3, §3.3.

use serde::{Deserialize, Serialize};

/// Skin summary returned by the IPC surface. Carries the manifest
/// header (id, name, version, author) plus the active flag.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkinInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub active: bool,
    /// True for the 4 built-in skins (modern, winamp, dense_pro,
    /// cinematic). False for user-installed skins.
    pub builtin: bool,
}

/// Skin CSS payload returned by `GetActiveCss`. The frontend
/// injects this into a `<style id="muzon-skin">` element on
/// mount and on every skin switch.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkinCssPayload {
    pub skin: SkinInfo,
    pub css: String,
    pub main_template: String,
}

/// Skin IPC request envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SkinRequest {
    /// List all installed skins (built-ins + user-installed).
    /// v0.2.0 returns the 4 built-ins; 0016 adds user-installed.
    ListInstalled,
    /// Get the active skin's `SkinInfo`.
    Active,
    /// Set the active skin by id. The frontend calls this from
    /// the Settings skin picker (0019).
    SetActive { id: String },
    /// Get the active skin's CSS content and main template.
    /// The frontend injects both into the WebView.
    GetActiveCss,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SkinResponse {
    ListInstalled { results: Vec<SkinInfo> },
    Active { result: Option<SkinInfo> },
    SetActive { ok: bool },
    GetActiveCss { result: Option<SkinCssPayload> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_installed_response_round_trip() {
        let resp = SkinResponse::ListInstalled {
            results: vec![SkinInfo {
                id: "modern".into(),
                name: "Modern Skin".into(),
                version: "0.1.0".into(),
                author: "Muzon contributors".into(),
                active: true,
                builtin: true,
            }],
        };
        let json = serde_json::to_string(&resp).expect("serialise");
        let parsed: SkinResponse = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(resp, parsed);
    }

    #[test]
    fn get_active_css_response_round_trip() {
        let resp = SkinResponse::GetActiveCss {
            result: Some(SkinCssPayload {
                skin: SkinInfo {
                    id: "modern".into(),
                    name: "Modern Skin".into(),
                    version: "0.1.0".into(),
                    author: "Muzon contributors".into(),
                    active: true,
                    builtin: true,
                },
                css: ":root { --bg-0: #0a0a0c; }".into(),
                main_template: "<div class=\"app\"></div>".into(),
            }),
        };
        let json = serde_json::to_string(&resp).expect("serialise");
        let parsed: SkinResponse = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(resp, parsed);
    }

    #[test]
    fn set_active_request_round_trip() {
        let req = SkinRequest::SetActive { id: "winamp".into() };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: SkinRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }
}
