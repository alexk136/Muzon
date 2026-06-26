// SPDX-License-Identifier: MIT OR Apache-2.0
//! Chromaprint FFI wrapper.
//!
//! Wraps the `chromaprint` 0.2 crate (which is a safe binding
//! to the C `libchromaprint` library) to compute a compact
//! audio fingerprint from raw mono PCM samples. The
//! fingerprint is a base64-encoded string that the AcoustID
//! client (0022 step 2) submits to the AcoustID HTTP API
//! for lookup.

use chromaprint::Chromaprint;

/// Compact audio fingerprint. Base64-encoded so it can be
/// sent over HTTP to the AcoustID service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// The base64-encoded fingerprint returned by chromaprint.
    /// The AcoustID service expects this exact format.
    pub base64: String,
}

impl Fingerprint {
    /// True if the fingerprint is empty.
    pub fn is_empty(&self) -> bool {
        self.base64.is_empty()
    }

    /// Length of the base64 string.
    pub fn len(&self) -> usize {
        self.base64.len()
    }
}

/// Compute the audio fingerprint for a mono PCM signal at the
/// given sample rate. The input is `f32` in `[-1.0, 1.0]`.
/// Returns an empty fingerprint on error.
pub fn compute_fingerprint(
    samples: &[f32],
    sample_rate: u32,
) -> Fingerprint {
    if samples.is_empty() {
        return Fingerprint { base64: String::new() };
    }

    // Chromaprint expects i16 samples. Convert f32 -> i16.
    let i16_samples: Vec<i16> = samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();

    let mut fp = Chromaprint::new();
    if !fp.start(sample_rate as i32, 1) {
        return Fingerprint { base64: String::new() };
    }
    if !fp.feed(&i16_samples) {
        return Fingerprint { base64: String::new() };
    }
    if !fp.finish() {
        return Fingerprint { base64: String::new() };
    }
    let base64 = fp.fingerprint().unwrap_or_default();
    Fingerprint { base64 }
}

/// Algorithm tag for the fingerprint (AcoustID expects
/// `chromaprint` in the API request). v0.4.0 minimum: only
/// the default algorithm (TEST2) is supported; the enum is
/// a placeholder for future algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FingerprintAlgorithm {
    #[default]
    Test2,
}

impl FingerprintAlgorithm {
    /// The string identifier for the AcoustID API request.
    pub fn as_str(&self) -> &'static str {
        match self {
            FingerprintAlgorithm::Test2 => "chromaprint",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_signal_returns_empty_fingerprint() {
        let f = compute_fingerprint(&[], 44100);
        assert!(f.is_empty());
    }

    #[test]
    fn silence_returns_non_empty_fingerprint() {
        // A silent signal still produces a fingerprint
        // (the algorithm hashes the silence into a fixed
        // pattern). The fingerprint is non-empty.
        let samples = vec![0.0_f32; 44100];
        let f = compute_fingerprint(&samples, 44100);
        assert!(!f.is_empty());
    }

    #[test]
    fn algorithm_default_is_test2() {
        assert_eq!(FingerprintAlgorithm::default().as_str(), "chromaprint");
    }
}
