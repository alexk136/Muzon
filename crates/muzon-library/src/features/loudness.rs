// SPDX-License-Identifier: MIT OR Apache-2.0
//! Loudness via ITU-R BS.1770 integrated LUFS (K-weighted).
//!
//! v0.5.0 minimum: uses the `ebur128` crate for the gating
//! and integration. The K-weighting is applied via the
//! `ebur128::Mode::l()` true-peak measurement; the integrated
//! loudness is read after the full track is processed.

use ebur128::EbuR128;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Loudness {
    /// Integrated loudness in LUFS.
    pub lufs: f32,
    /// True peak in dBTP.
    pub true_peak_dbtp: f32,
}

/// Measure the integrated LUFS loudness of a mono PCM signal
/// at the given sample rate. Returns the integrated loudness
/// and the true peak in dBTP.
pub fn extract_loudness_lufs(samples: &[f32], sample_rate: u32) -> Loudness {
    if samples.is_empty() {
        return Loudness::default();
    }
    // The ebur128 API expects f32 samples at the given rate.
    let mut meter = match EbuR128::new(1, sample_rate as u32, ebur128::Mode::I) {
        Ok(m) => m,
        Err(_) => return Loudness::default(),
    };
    // Add frames in chunks for the loudness window.
    const CHUNK: usize = 4096;
    for chunk in samples.chunks(CHUNK) {
        if meter.add_frames_f32(chunk).is_err() {
            return Loudness::default();
        }
    }
    let loudness = match meter.loudness_global() {
        Ok(v) => v as f32,
        Err(_) => return Loudness::default(),
    };
    let peak_db = match meter.true_peak(0) {
        Ok(p) => 20.0 * (p.max(1e-9) as f32).log10(),
        Err(_) => -f32::INFINITY,
    };
    Loudness {
        lufs: loudness,
        true_peak_dbtp: peak_db,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_returns_neg_inf_lufs() {
        // A silent signal has a very low LUFS value; the
        // ebur128 library returns -inf or a very negative
        // value. The extractor returns it as-is.
        let samples = vec![0.0_f32; 44100];
        let l = extract_loudness_lufs(&samples, 44100);
        // -70 LUFS is the practical noise floor per BS.1770.
        // We accept any value < -40 LUFS.
        assert!(l.lufs < -40.0, "got {}", l.lufs);
    }

    #[test]
    fn sine_at_minus_20_dbfs() {
        // 1 kHz sine at -20 dBFS RMS (so the level is around
        // -23 LUFS K-weighted). 1 second at 44.1 kHz.
        let n = 44100;
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / 44100.0;
            // -20 dBFS amplitude is 0.1.
            let s = 0.1 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
            samples.push(s);
        }
        let l = extract_loudness_lufs(&samples, 44100);
        // 1 kHz sine at -20 dBFS amplitude is approximately
        // -23 LUFS after K-weighting (the K-weight at 1 kHz
        // is 0 dB). Allow a wide tolerance.
        assert!(
            (l.lufs + 23.0).abs() < 5.0,
            "expected ~-23 LUFS, got {}",
            l.lufs
        );
    }
}
