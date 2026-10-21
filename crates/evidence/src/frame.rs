//! Frame extraction and windowed spectra on the 200 Hz control grid.

use std::f64::consts::PI;
use std::sync::Arc;

use realfft::num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};

/// Control rate. Frame `k` is centred at `k / 200` seconds.
pub const CONTROL_HZ: u64 = 200;

/// Centre sample of control frame `k` at `sample_rate`: `floor(k * sr / 200)`,
/// exact in integers so 44.1 kHz (220.5 samples per frame) never drifts.
pub fn frame_centre(k: u64, sample_rate: u32) -> u64 {
    k * u64::from(sample_rate) / CONTROL_HZ
}

/// Number of control frames whose centre lies inside a take of `n_samples`.
pub fn frame_count(n_samples: u64, sample_rate: u32) -> usize {
    (n_samples * CONTROL_HZ).div_ceil(u64::from(sample_rate)) as usize
}

/// An even window length of about `seconds` at `sample_rate`.
pub fn even_len(seconds: f64, sample_rate: u32) -> usize {
    let n = (seconds * f64::from(sample_rate) / 2.0).round() as usize;
    2 * n.max(2)
}

/// Periodic Hann window.
pub fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()) as f32)
        .collect()
}

/// Copies `out.len()` samples of `x` starting at `start`, zero outside `x`.
pub fn read_padded(x: &[f32], start: i64, out: &mut [f32]) {
    out.fill(0.0);
    let len = out.len() as i64;
    let lo = start.max(0);
    let hi = (start + len).min(x.len() as i64);
    if lo < hi {
        let src = &x[lo as usize..hi as usize];
        let off = (lo - start) as usize;
        out[off..off + src.len()].copy_from_slice(src);
    }
}

/// Hann-windowed magnitude and power spectrum of a frame centred on a sample.
pub struct Spectrum {
    pub len: usize,
    window: Vec<f32>,
    window_power: f64,
    fft: Arc<dyn RealToComplex<f32>>,
    input: Vec<f32>,
    scratch: Vec<Complex32>,
    pub bins: Vec<Complex32>,
    pub mag: Vec<f32>,
    pub power: Vec<f32>,
    /// Mean square of the windowed frame, normalised by window power.
    pub mean_square: f64,
}

impl std::fmt::Debug for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spectrum")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl Spectrum {
    pub fn new(planner: &mut RealFftPlanner<f32>, len: usize) -> Self {
        let fft = planner.plan_fft_forward(len);
        let window = hann(len);
        let window_power = window.iter().map(|&w| f64::from(w) * f64::from(w)).sum();
        Self {
            len,
            input: fft.make_input_vec(),
            scratch: fft.make_scratch_vec(),
            bins: fft.make_output_vec(),
            mag: vec![0.0; len / 2 + 1],
            power: vec![0.0; len / 2 + 1],
            window,
            window_power,
            fft,
            mean_square: 0.0,
        }
    }

    /// Sum of squared window coefficients.
    pub fn window_power(&self) -> f64 {
        self.window_power
    }

    /// Frequency in Hz of bin `k`.
    pub fn bin_hz(&self, k: usize, sample_rate: u32) -> f32 {
        (k as f64 * f64::from(sample_rate) / self.len as f64) as f32
    }

    /// Analyses the frame of `x` centred on sample `centre`.
    pub fn compute(&mut self, x: &[f32], centre: u64) {
        let start = centre as i64 - (self.len / 2) as i64;
        read_padded(x, start, &mut self.input);
        let mut sum_sq = 0.0f64;
        for (s, &w) in self.input.iter_mut().zip(&self.window) {
            *s *= w;
            sum_sq += f64::from(*s) * f64::from(*s);
        }
        self.mean_square = sum_sq / self.window_power;
        self.fft
            .process_with_scratch(&mut self.input, &mut self.bins, &mut self.scratch)
            .expect("buffer sizes come from the plan");
        for ((c, m), p) in self.bins.iter().zip(&mut self.mag).zip(&mut self.power) {
            *p = c.norm_sqr();
            *m = p.sqrt();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_centres_are_exact_at_44k1() {
        assert_eq!(frame_centre(0, 44_100), 0);
        assert_eq!(frame_centre(1, 44_100), 220);
        assert_eq!(frame_centre(2, 44_100), 441);
        assert_eq!(frame_centre(200 * 3600, 44_100), 44_100 * 3600);
        assert_eq!(frame_count(441, 44_100), 2);
        assert_eq!(frame_count(442, 44_100), 3);
        assert_eq!(frame_count(0, 48_000), 0);
        assert_eq!(frame_count(1, 48_000), 1);
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact copies
    fn padded_read_handles_both_edges() {
        let x = [1.0, 2.0, 3.0];
        let mut out = [9.0; 5];
        read_padded(&x, -2, &mut out);
        assert_eq!(out, [0.0, 0.0, 1.0, 2.0, 3.0]);
        read_padded(&x, 2, &mut out);
        assert_eq!(out, [3.0, 0.0, 0.0, 0.0, 0.0]);
        read_padded(&x, 7, &mut out);
        assert_eq!(out, [0.0; 5]);
    }
}
