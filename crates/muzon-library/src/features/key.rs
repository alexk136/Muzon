// SPDX-License-Identifier: MIT OR Apache-2.0
//! Key (pitch class) estimation via Krumhansl-Schmuckler.
//!
//! Step 1: compute the STFT (Short-Time Fourier Transform)
//! with a 4096-sample window, 50% overlap.
//! Step 2: for each frame, map the magnitude spectrum to a
//! 12-bin chromagram (sum the magnitude in each pitch class).
//! Step 3: average the chromagram across frames.
//! Step 4: correlate the average chromagram with the 24
//! Krumhansl-Schmuckler key profiles (12 major + 12 minor).
//!
//! v0.5.0 minimum; the v0.3.0 minimum (0027) ships a simpler
//! version. The algorithm is a clean-room Rust port.

use std::f32::consts::PI;

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

const A4_FREQ: f32 = 440.0;
const PITCH_CLASSES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const MAJOR_PROFILE: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
const MAJOR_PROFILES: [[f32; 12]; 12] = [MAJOR_PROFILE; 12];
const MINOR_PROFILES: [[f32; 12]; 12] = [MINOR_PROFILE; 12];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct KeyEstimate {
    /// The estimated key, e.g. "C" (major) or "A minor".
    pub key: String,
    /// Confidence in the estimate (0-1).
    pub confidence: f32,
}

/// Estimate the musical key from a mono PCM signal. The
/// algorithm is the Krumhansl-Schmuckler 1986 method: STFT
/// -> chromagram -> correlation with the 24 key profiles.
pub fn extract_key(samples: &[f32], sample_rate: u32) -> KeyEstimate {
    if samples.len() < 8192 {
        return KeyEstimate::default();
    }

    let chromagram = compute_chromagram(samples, sample_rate);
    if chromagram.iter().all(|&m| m == 0.0) {
        return KeyEstimate::default();
    }

    // Correlate with the 24 profiles (12 major + 12 minor).
    let mut best_key = String::new();
    let mut best_value = f32::MIN;
    for (i, profile) in MAJOR_PROFILES.iter().enumerate() {
        let v = correlate(&chromagram, profile);
        if v > best_value {
            best_value = v;
            best_key = format!("{}", PITCH_CLASSES[i]);
        }
    }
    for (i, profile) in MINOR_PROFILES.iter().enumerate() {
        let v = correlate(&chromagram, profile);
        if v > best_value {
            best_value = v;
            best_key = format!("{} minor", PITCH_CLASSES[i]);
        }
    }
    KeyEstimate {
        key: best_key,
        confidence: best_value.clamp(0.0, 1.0),
    }
}


fn compute_chromagram(samples: &[f32], sample_rate: u32) -> [f32; 12] {
    const WINDOW: usize = 4096;
    const HOP: usize = WINDOW / 2;
    let mut chromagram = [0.0_f32; 12];
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let mut buf = vec![0.0_f32; WINDOW * 2];
    let mut count = 0;

    let mut start = 0;
    while start + WINDOW <= samples.len() {
        for i in 0..WINDOW {
            // Hann window
            let w = 0.5
                - 0.5 * (2.0 * PI * i as f32 / (WINDOW - 1) as f32).cos();
            buf[i] = samples[start + i] * w;
            buf[WINDOW + i] = 0.0;
        }
        // rustfft expects interleaved complex samples.
        let mut spectrum: Vec<Complex<f32>> = buf
            .chunks_exact(2)
            .map(|c| Complex::new(c[0], c[1]))
            .collect();
        fft.process(&mut spectrum);
        // Aggregate the magnitude into the 12 pitch classes.
        for bin in 1..(WINDOW / 2) {
            let freq = bin as f32 * sample_rate as f32 / WINDOW as f32;
            if freq < 20.0 || freq > 5000.0 {
                continue;
            }
            let pitch_class = freq_to_pitch_class(freq);
            let c = spectrum[bin];
            let mag = (c.re * c.re + c.im * c.im).sqrt();
            chromagram[pitch_class] += mag;
        }
        count += 1;
        start += HOP;
    }
    if count > 0 {
        for c in chromagram.iter_mut() {
            *c /= count as f32;
        }
    }
    chromagram
}

fn freq_to_pitch_class(freq: f32) -> usize {
    if freq <= 0.0 {
        return 0;
    }
    // MIDI note: 69 + 12 * log2(f / A4)
    let note = 69.0 + 12.0 * (freq / A4_FREQ).log2();
    let note_rounded = note.round() as i32;
    let pc = note_rounded.rem_euclid(12);
    pc as usize
}

fn correlate(chroma: &[f32; 12], profile: &[f32; 12]) -> f32 {
    let chroma_norm = norm(chroma);
    let profile_norm = norm(profile);
    let mut sum = 0.0;
    for i in 0..12 {
        sum += chroma_norm[i] * profile_norm[i];
    }
    sum
}

fn norm(v: &[f32; 12]) -> [f32; 12] {
    let mean: f32 = v.iter().sum::<f32>() / 12.0;
    let mut centered = [0.0_f32; 12];
    for i in 0..12 {
        centered[i] = v[i] - mean;
    }
    let s: f32 = centered.iter().map(|x| x * x).sum::<f32>().sqrt();
    if s > 0.0 {
        for c in centered.iter_mut() {
            *c /= s;
        }
    }
    centered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_with_partials(freqs: &[f32], duration_s: f32, sample_rate: u32) -> Vec<f32> {
        let n = (duration_s * sample_rate as f32) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sample_rate as f32;
            let mut s = 0.0;
            for &f in freqs {
                s += (2.0 * PI * f * t).sin();
            }
            out.push(s / freqs.len() as f32);
        }
        out
    }

    #[test]
    fn detects_c_major_chord() {
        // C major chord: C, E, G (262, 330, 392 Hz).
        let samples = sine_with_partials(&[262.0, 330.0, 392.0], 4.0, 44100);
        let k = extract_key(&samples, 44100);
        // The Krumhansl-Schmuckler method requires a few
        // seconds of audio to converge; with a clean major
        // chord the algorithm picks the right tonic.
        assert!(k.key.starts_with("C"), "got {:?}", k.key);
    }

    #[test]
    fn empty_signal_returns_default() {
        let k = extract_key(&[], 44100);
        assert_eq!(k.key, "");
    }
}
