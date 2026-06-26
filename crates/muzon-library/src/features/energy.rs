// SPDX-License-Identifier: MIT OR Apache-2.0
//! Energy / RMSE-based features.
//!
//! v0.5.0 minimum: the energy feature is a single number
//! (the root-mean-square energy of the PCM) that the mood
//! map (0030) and the smart playlists (0029) use as one
//! of the cluster axes.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnergyFeatures {
    /// Root-mean-square energy in `[0.0, 1.0]`.
    pub rmse: f32,
    /// Peak absolute amplitude in `[0.0, 1.0]`.
    pub peak: f32,
}

/// Compute the RMSE and peak amplitude of a PCM signal.
pub fn compute_energy(samples: &[f32]) -> EnergyFeatures {
    if samples.is_empty() {
        return EnergyFeatures::default();
    }
    let mut sum_sq = 0.0_f64;
    let mut peak = 0.0_f32;
    for &s in samples {
        sum_sq += (s as f64) * (s as f64);
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }
    let rmse = (sum_sq / samples.len() as f64).sqrt() as f32;
    EnergyFeatures { rmse, peak }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_has_zero_energy() {
        let e = compute_energy(&[0.0_f32; 100]);
        assert_eq!(e.rmse, 0.0);
        assert_eq!(e.peak, 0.0);
    }

    #[test]
    fn sine_has_known_rmse() {
        // 1 kHz sine at amplitude 0.5 has RMSE = 0.5 / sqrt(2) ≈ 0.354.
        let n = 44100;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin())
            .collect();
        let e = compute_energy(&samples);
        assert!((e.rmse - 0.354).abs() < 0.01, "got {}", e.rmse);
        assert!((e.peak - 0.5).abs() < 0.01);
    }
}
