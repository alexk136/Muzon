// SPDX-License-Identifier: MIT OR Apache-2.0
//! AcoustID client.
//!
//! Submits an audio fingerprint to the AcoustID service and
//! returns the matched MusicBrainz recording IDs. v0.4.0
//! minimum: the client is wired with the HTTP shape
//! (`https://api.acoustid.org/v2/lookup`) but the actual
//! request requires an AcoustID API key. The key is read
//! from `network.acoustid` in the config (gated behind
//! `network.explicit_opt_in` per decision 0001-N4); v0.4.0
//! ships the key field as an empty string; the user can
//! fill it in via Settings (0019).
//!
//! v0.4.0 minimum: a typed `AcoustIdMatch` struct, an
//! `AcoustIdClient::lookup` method that takes a `Fingerprint`
//! and returns a `Vec<AcoustIdMatch>`, and a stub HTTP
//! implementation. v0.4.0 hardening adds the real `reqwest`
//! integration and the `duration` parameter (AcoustID requires
//! the track duration in seconds).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::fingerprint::{Fingerprint, FingerprintAlgorithm};

/// A single match from the AcoustID service.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AcoustIdMatch {
    /// MusicBrainz recording ID.
    pub recording_id: String,
    /// Confidence score in `[0.0, 1.0]`.
    pub score: f32,
    /// Optional title (the service returns it when the
    /// recording has a known title).
    pub title: Option<String>,
    /// Optional artist.
    pub artist: Option<String>,
}

/// Client for the AcoustID service. Constructed with an
/// optional API key (empty string for v0.4.0 minimum; the
/// real key is filled in via Settings).
pub struct AcoustIdClient {
    api_key: String,
}

impl AcoustIdClient {
    /// Build a new client. The `api_key` is the AcoustID
    /// application API key; the user gets one by registering
    /// at acoustid.org. v0.4.0 ships an empty key (the user
    /// fills it in via Settings).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
    }

    /// Look up the fingerprint in the AcoustID database. The
    /// `duration` is the track's playback duration in
    /// seconds (AcoustID requires it). v0.4.0 minimum
    /// returns an empty result; v0.4.0 hardening adds the
    /// real HTTP call.
    pub async fn lookup(
        &self,
        _fingerprint: &Fingerprint,
        _duration: Duration,
    ) -> Result<Vec<AcoustIdMatch>, AcoustIdError> {
        if self.api_key.is_empty() {
            // Without a key, the service rejects the request.
            // v0.4.0 returns an empty result rather than
            // failing, so the UI can show a "configure your
            // AcoustID key" hint.
            return Ok(Vec::new());
        }
        // v0.4.0 hardening: POST to https://api.acoustid.org/v2/lookup
        // with form-encoded body (client=<key>&duration=<s>&fingerprint=<base64>&algorithm=chromaprint)
        // and parse the JSON response.
        Ok(Vec::new())
    }

    /// The algorithm string for the AcoustID request.
    pub fn algorithm(&self) -> FingerprintAlgorithm {
        FingerprintAlgorithm::default()
    }
}

/// Errors produced by the AcoustID client.
#[derive(Debug, thiserror::Error)]
pub enum AcoustIdError {
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_key_returns_no_matches() {
        let client = AcoustIdClient::new("");
        let fp = Fingerprint {
            base64: "AAAA".into(),
        };
        let result = client.lookup(&fp, Duration::from_secs(240)).await.expect("ok");
        assert!(result.is_empty());
    }
}
