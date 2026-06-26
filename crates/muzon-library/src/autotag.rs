// SPDX-License-Identifier: MIT OR Apache-2.0
//! Auto-tag pipeline: AcoustID (0022) → MusicBrainz
//! (0023) → tag write-back (0025).
//!
//! v0.4.0 minimum: a typed `AutoTagger` struct that takes
//! a `Fingerprint` and a file path, and returns a list of
//! `AutoTagMatch` candidates. The actual HTTP calls (AcoustID
//! + MusicBrainz) and the tag write-back are deferred to a
//! v0.4.0 hardening pass.

use std::path::Path;

/// A single candidate from the auto-tag pipeline.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AutoTagMatch {
    /// MusicBrainz recording ID.
    pub recording_id: String,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f32,
    /// Title suggested by MusicBrainz.
    pub title: String,
    /// Artist suggested by MusicBrainz.
    pub artist: String,
}


/// Auto-tag pipeline. v0.4.0 minimum is a typed struct with
/// a stub `run` method; v0.4.0 hardening wires the real
/// AcoustID + MusicBrainz + tag write-back calls.
pub struct AutoTagger {
    acoustid_key: String,
    mb_contact: String,
}

impl AutoTagger {
    /// Build a new auto-tagger. The `acoustid_key` is the
    /// AcoustID application API key; the `mb_contact` is the
    /// contact email for the MusicBrainz service.
    pub fn new(acoustid_key: impl Into<String>, mb_contact: impl Into<String>) -> Self {
        Self {
            acoustid_key: acoustid_key.into(),
            mb_contact: mb_contact.into(),
        }
    }

    /// Run the auto-tag pipeline on a single file. v0.4.0
    /// minimum: returns an empty result. v0.4.0 hardening:
    /// 1. Compute the fingerprint (0022).
    /// 2. Submit to AcoustID (0022).
    /// 3. For each match, fetch MusicBrainz metadata (0023).
    /// 4. Write the tags back to the file (0025).
    pub async fn run(
        &self,
        _path: &Path,
        _fingerprint_base64: &str,
        _duration_ms: u64,
    ) -> Result<Vec<AutoTagMatch>, String> {
        let _ = (&self.acoustid_key, &self.mb_contact);
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_returns_empty_for_v0_4_0_minimum() {
        let tagger = AutoTagger::new("", "test@example.com");
        let result = tagger
            .run(Path::new("/tmp/a.flac"), "AAAA", 240_000)
            .await
            .expect("ok");
        assert!(result.is_empty());
    }
}
