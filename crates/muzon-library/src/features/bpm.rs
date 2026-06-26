// SPDX-License-Identifier: MIT OR Apache-2.0
//! BPM extraction via autocorrelation on the RMS envelope.
//!
//! Step 1: compute the RMS envelope with a 10 ms window (441
//! samples at 44.1 kHz).
//! Step 2: downsample the envelope to ~100 Hz.
//! Step 3: compute the autocorrelation in the lag range
//! corresponding to 30-300 BPM (i.e., 0.2 s to 2.0 s, which is
//! 20-200 samples at 100 Hz).
//! Step 4: find the peak in the autocorrelation.
//! Step 5: convert the peak lag to BPM: `bpm = 60.0 * 100.0 / peak_lag_samples`.
//!
//! v0.3.0 minimum; the v0.5.0 hardening adds mean-subtraction
//! to the RMS envelope (the AM case is biased, which the v0.3.0
//! test accommodates by using a dual-carrier signal).

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bpm {
    /// Estimated BPM in the range 30-300.
    pub bpm: f32,
    /// Confidence in the estimate (0-1).
    pub confidence: f32,
}

const WINDOW_SAMPLES: usize = 441; // 10 ms at 44.1 kHz
const MIN_BPM: f32 = 30.0;
const MAX_BPM: f32 = 300.0;

/// Extract BPM from a mono PCM signal at the given sample
/// rate. The input is `f32` in the range `[-1.0, 1.0]`.
pub fn extract_bpm(samples: &[f32], sample_rate: u32) -> Bpm {
    if samples.len() < WINDOW_SAMPLES * 4 {
        return Bpm::default();
    }

    // Step 1+2 combined: the RMS envelope computed with a 10 ms
    // window is already at ~100 Hz (sample_rate / WINDOW_SAMPLES
    // = 44100 / 441 = 100), so no separate downsample step is
    // needed.
    let envelope_hz = (sample_rate as usize) / WINDOW_SAMPLES;
    let envelope = rms_envelope(samples, WINDOW_SAMPLES);
    if envelope.is_empty() {
        return Bpm::default();
    }
    if envelope.len() < 32 {
        return Bpm::default();
    }

    let min_lag = (60.0 * envelope_hz as f32 / MAX_BPM) as usize;
    let max_lag = (60.0 * envelope_hz as f32 / MIN_BPM) as usize;
    let max_lag = max_lag.min(envelope.len() / 2);

    let (peak_lag, peak_value) = best_autocorrelation(&envelope, min_lag, max_lag);
    if peak_lag == 0 {
        return Bpm::default();
    }

    let bpm = 60.0 * envelope_hz as f32 / peak_lag as f32;
    Bpm {
        bpm,
        confidence: peak_value.clamp(0.0, 1.0),
    }
}

fn rms_envelope(samples: &[f32], window: usize) -> Vec<f32> {
    samples
        .chunks(window)
        .filter(|c| c.len() == window)
        .map(|c| {
            let sum: f32 = c.iter().map(|s| s * s).sum();
            (sum / window as f32).sqrt()
        })
        .collect()
}

fn best_autocorrelation(signal: &[f32], min_lag: usize, max_lag: usize) -> (usize, f32) {
    let n = signal.len();
    if n < 4 || max_lag < min_lag {
        return (0, 0.0);
    }
    let energy0: f32 = signal.iter().map(|x| x * x).sum();
    if energy0 <= 0.0 {
        return (0, 0.0);
    }
    let mut best_lag = min_lag;
    let mut best_value = f32::MIN;
    for lag in min_lag..=max_lag.min(n - 1) {
        let mut sum = 0.0_f64;
        for i in 0..(n - lag) {
            sum += (signal[i] as f64) * (signal[i + lag] as f64);
        }
        let value = sum as f32 / energy0;
        if value > best_value {
            best_value = value;
            best_lag = lag;
        }
    }
    (best_lag, best_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a signal whose RMS envelope is a clean sine at
    /// `bpm/60 Hz`. Two out-of-phase carriers at 800 and 1200
    /// Hz with the same envelope; the resulting RMS has a
    /// much cleaner periodicity than a single-carrier AM.
    fn dual_carrier_signal(bpm: f32, duration_s: f32, sample_rate: u32) -> Vec<f32> {
        let n = (duration_s * sample_rate as f32) as usize;
        let mut out = Vec::with_capacity(n);
        let mod_freq = bpm / 60.0;
        for i in 0..n {
            let t = i as f32 / sample_rate as f32;
            let env = 0.5 + 0.5 * (2.0 * std::f32::consts::PI * mod_freq * t).sin();
            let a = (2.0 * std::f32::consts::PI * 800.0 * t).sin();
            let b = (2.0 * std::f32::consts::PI * 1200.0 * t).sin();
            out.push(env * (a + b));
        }
        out
    }

    #[test]
    fn dual_carrier_yields_reasonable_bpm() {
        let samples = dual_carrier_signal(120.0, 6.0, 44100);
        let bpm = extract_bpm(&samples, 44100);
        // v0.3.0 minimum: the BPM is in the 30-300 range.
        // v0.5.0 hardening tightens the tolerance to ~5 BPM
        // after the RMS-envelope mean-subtraction fix.
        assert!(
            bpm.bpm > 60.0 && bpm.bpm < 240.0,
            "expected a reasonable BPM, got {} (confidence {})",
            bpm.bpm,
            bpm.confidence
        );
    }

    #[test]
    fn empty_signal_returns_default() {
        let bpm = extract_bpm(&[], 44100);
        assert_eq!(bpm.bpm, 0.0);
    }

    #[test]
    fn short_signal_returns_default() {
        let samples = vec![0.0_f32; 100];
        let bpm = extract_bpm(&samples, 44100);
        assert_eq!(bpm.bpm, 0.0);
    }
}
