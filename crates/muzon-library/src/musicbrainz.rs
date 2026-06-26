// SPDX-License-Identifier: MIT OR Apache-2.0
//! MusicBrainz client.
//!
//! v0.4.0 minimum: typed structs for the recording/release/
//! artist/release-group metadata. The client queries
//! `musicbrainz.org/ws/2/` (XML) for the recording by MBID
//! and returns the canonical tags. v0.4.0 hardening adds
//! the real `reqwest` integration; v0.4.0 minimum returns
//! a stub response from `lookup_recording`.
//!
//! Rate-limited to 1 req/sec per the MusicBrainz ToS; the
//! user is expected to provide a contact email via Settings
//! (the `User-Agent` header).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// A MusicBrainz recording.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Recording {
    /// The MusicBrainz ID (MBID), e.g.
    /// `8cb5be4e-3a36-4b71-89a2-3df65d8d94b3`.
    pub id: String,
    /// The canonical title.
    pub title: String,
    /// The artist MBID.
    pub artist_id: String,
    /// The artist name.
    pub artist_name: String,
    /// The release MBID (album).
    pub release_id: String,
    /// The release title (album).
    pub release_title: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// A MusicBrainz release (album).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub title: String,
    pub artist_id: String,
    pub artist_name: String,
    /// Release date as ISO-8601 (`YYYY-MM-DD` or `YYYY`).
    pub date: String,
}

/// A MusicBrainz artist.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    /// The artist's sort name (e.g. "Beatles, The").
    pub sort_name: String,
}

/// Client for the MusicBrainz web service. Constructed with
/// a contact email (the `User-Agent` header).
pub struct MusicBrainzClient {
    user_agent: String,
}

impl MusicBrainzClient {
    /// Build a new client. The `contact` is a contact email
    /// that the MusicBrainz service requires in the
    /// `User-Agent` header.
    pub fn new(contact: impl Into<String>) -> Self {
        Self {
            user_agent: format!("muzon/0.1 (contact: {})", contact.into()),
        }
    }

    /// Look up a recording by MBID. v0.4.0 minimum returns a
    /// stub; v0.4.0 hardening adds the real HTTP call.
    pub async fn lookup_recording(
        &self,
        mbid: &str,
    ) -> Result<Recording, MusicBrainzError> {
        // v0.4.0 hardening: GET https://musicbrainz.org/ws/2/recording/{mbid}?inc=artist-credits+artist+releases
        // with the `User-Agent` header. Rate-limited to 1 req/sec.
        let _ = &self.user_agent; // suppress unused warning in v0.4.0 minimum
        let _ = mbid; // suppress unused
        Ok(Recording::default())
    }

    /// Look up a release by MBID. v0.4.0 minimum stub.
    pub async fn lookup_release(
        &self,
        mbid: &str,
    ) -> Result<Release, MusicBrainzError> {
        let _ = mbid;
        Ok(Release::default())
    }

    /// Look up an artist by MBID. v0.4.0 minimum stub.
    pub async fn lookup_artist(
        &self,
        mbid: &str,
    ) -> Result<Artist, MusicBrainzError> {
        let _ = mbid;
        Ok(Artist::default())
    }

    /// The recommended rate-limit between requests. The
    /// MusicBrainz ToS require ≤ 1 req/sec.
    pub fn rate_limit(&self) -> Duration {
        Duration::from_secs(1)
    }
}

/// Errors produced by the MusicBrainz client.
#[derive(Debug, thiserror::Error)]
pub enum MusicBrainzError {
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lookup_recording_returns_default() {
        let client = MusicBrainzClient::new("test@example.com");
        let r = client
            .lookup_recording("8cb5be4e-3a36-4b71-89a2-3df65d8d94b3")
            .await
            .expect("ok");
        assert_eq!(r.id, "");
    }

    #[tokio::test]
    async fn lookup_release_returns_default() {
        let client = MusicBrainzClient::new("test@example.com");
        let r = client
            .lookup_release("8cb5be4e-3a36-4b71-89a2-3df65d8d94b3")
            .await
            .expect("ok");
        assert_eq!(r.id, "");
    }

    #[tokio::test]
    async fn lookup_artist_returns_default() {
        let client = MusicBrainzClient::new("test@example.com");
        let r = client
            .lookup_artist("8cb5be4e-3a36-4b71-89a2-3df65d8d94b3")
            .await
            .expect("ok");
        assert_eq!(r.id, "");
    }

    #[test]
    fn rate_limit_is_one_second() {
        let client = MusicBrainzClient::new("test@example.com");
        assert_eq!(client.rate_limit(), Duration::from_secs(1));
    }
}
