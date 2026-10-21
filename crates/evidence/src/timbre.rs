//! Loudness, brightness and breathiness per frame, at capture rate.
//!
//! - Loudness: A-weighted mean square in dB re full scale, short window so
//!   attacks stay sharp.
//! - Brightness: spectral centroid in Hz (up to 16 kHz), the same in harmonic units of the
//!   frame's most probable pitch candidate, and spectral tilt in dB per octave
//!   from a regression over third-octave band levels (100 Hz to 8 kHz).
//! - Breathiness: smoothed cepstral peak prominence (Hillenbrand et al. 1994,
//!   CPPS-style quefrency smoothing) over the 60 to 880 Hz quefrency range.
//!   Lower CPP means breathier.
//!
//! Undefined values (silent frames, no pitch candidate) are NaN.

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner};

use crate::frame::{Spectrum, even_len};

/// Loudness window, about 23 ms (1024 samples at 44.1 kHz).
pub(crate) const LOUDNESS_SECONDS: f64 = 0.0232;
/// Spectral window for brightness and CPP, about 46 ms: three periods at 65 Hz.
pub const SPECTRAL_SECONDS: f64 = 0.0464;
/// Below this mean square (-90 dBFS) spectral shape is undefined.
const SILENCE_MS: f64 = 1e-9;
const LOUDNESS_FLOOR_DB: f32 = -120.0;
const CPP_FMIN_HZ: f64 = 60.0;
const CPP_FMAX_HZ: f64 = 880.0;
const TILT_LO_HZ: f64 = 100.0;
const TILT_HI_HZ: f64 = 8000.0;
/// Centroid ignores bins above this so it does not scale with capture rate.
const CENTROID_HI_HZ: f64 = 16_000.0;
/// CPP smooths the power cepstrum over this quefrency span (CPPS-style).
const CPP_SMOOTH_SECONDS: f64 = 0.001;

/// IEC 61672 A-weighting as a power gain, 1 at 1 kHz.
fn a_weight_power(f: f64) -> f64 {
    let f2 = f * f;
    let ra = 12194.0f64.powi(2) * f2 * f2
        / ((f2 + 20.6f64.powi(2))
            * ((f2 + 107.7f64.powi(2)) * (f2 + 737.9f64.powi(2))).sqrt()
            * (f2 + 12194.0f64.powi(2)));
    // +2.00 dB normalises to 0 dB at 1 kHz.
    (ra * 10f64.powf(2.0 / 20.0)).powi(2)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TimbreFrame {
    pub loudness_db: f32,
    pub centroid_hz: f32,
    pub centroid_harm: f32,
    pub tilt_db_oct: f32,
    pub cpp_db: f32,
}

pub struct Timbre {
    sample_rate: u32,
    short: Spectrum,
    a_weights: Vec<f64>,
    long: Spectrum,
    bands: Vec<(usize, usize, f64)>,
    inv: std::sync::Arc<dyn ComplexToReal<f32>>,
    cep_in: Vec<Complex32>,
    cep_out: Vec<f32>,
    cep_scratch: Vec<Complex32>,
    q_lo: usize,
    q_hi: usize,
    q_half: usize,
    cep_power: Vec<f64>,
}

impl std::fmt::Debug for Timbre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timbre")
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

impl Timbre {
    pub fn new(planner: &mut RealFftPlanner<f32>, sample_rate: u32) -> Self {
        let sr = f64::from(sample_rate);
        let short = Spectrum::new(planner, even_len(LOUDNESS_SECONDS, sample_rate));
        let a_weights = (0..short.mag.len())
            .map(|k| a_weight_power(k as f64 * sr / short.len as f64))
            .collect();
        let long = Spectrum::new(planner, even_len(SPECTRAL_SECONDS, sample_rate));
        let hz_per_bin = sr / long.len as f64;
        let nyq_bin = long.mag.len() - 1;
        let mut bands = Vec::new();
        for i in 0.. {
            let centre = TILT_LO_HZ * 2f64.powf(f64::from(i) / 3.0);
            // Third-octave centres are nominal: 8063 Hz is the "8 kHz" band.
            if centre > TILT_HI_HZ * 1.01 || centre * 2f64.powf(1.0 / 6.0) > sr / 2.0 {
                break;
            }
            let lo = (centre * 2f64.powf(-1.0 / 6.0) / hz_per_bin).ceil() as usize;
            let hi = ((centre * 2f64.powf(1.0 / 6.0) / hz_per_bin).floor() as usize).min(nyq_bin);
            if lo <= hi {
                bands.push((lo, hi, centre.log2()));
            }
        }
        let inv = planner.plan_fft_inverse(long.len);
        let q_lo = (sr / CPP_FMAX_HZ).floor() as usize;
        let q_half = ((CPP_SMOOTH_SECONDS * sr / 2.0).round() as usize).max(1);
        let q_hi = ((sr / CPP_FMIN_HZ).ceil() as usize).min(long.len / 2 - 1 - q_half);
        Self {
            sample_rate,
            a_weights,
            bands,
            cep_in: inv.make_input_vec(),
            cep_out: inv.make_output_vec(),
            cep_scratch: inv.make_scratch_vec(),
            inv,
            q_lo,
            q_hi,
            q_half,
            cep_power: vec![0.0; long.len / 2 + 1],
            short,
            long,
        }
    }

    /// Magnitudes of the long-window spectrum from the last [`Self::analyze`].
    pub fn spectral_mag(&self) -> &[f32] {
        &self.long.mag
    }

    /// Length of the long-window spectrum's FFT.
    pub fn spectral_len(&self) -> usize {
        self.long.len
    }

    /// Features for the frame centred on `centre`. `pitch_hz` is the most
    /// probable pitch candidate (0 when there is none).
    pub fn analyze(&mut self, x: &[f32], centre: u64, pitch_hz: f32) -> TimbreFrame {
        TimbreFrame {
            loudness_db: self.loudness(x, centre),
            ..self.spectral(x, centre, pitch_hz)
        }
    }

    fn loudness(&mut self, x: &[f32], centre: u64) -> f32 {
        let s = &mut self.short;
        s.compute(x, centre);
        let last = s.power.len() - 1;
        let mut sum = 0.0;
        for (k, (&p, &a)) in s.power.iter().zip(&self.a_weights).enumerate() {
            let one_sided = if k == 0 || k == last { 1.0 } else { 2.0 };
            sum += one_sided * a * f64::from(p);
        }
        // Parseval: sum |X_k|^2 = L * sum (w x)^2; normalise by window power.
        let window_power = s.window_power();
        let ms = sum / (s.len as f64 * window_power);
        if ms > 0.0 {
            ((10.0 * ms.log10()) as f32).max(LOUDNESS_FLOOR_DB)
        } else {
            LOUDNESS_FLOOR_DB
        }
    }

    fn spectral(&mut self, x: &[f32], centre: u64, pitch_hz: f32) -> TimbreFrame {
        let sr = self.sample_rate;
        let s = &mut self.long;
        s.compute(x, centre);
        if s.mean_square < SILENCE_MS {
            return TimbreFrame {
                centroid_hz: f32::NAN,
                centroid_harm: f32::NAN,
                tilt_db_oct: f32::NAN,
                cpp_db: f32::NAN,
                ..TimbreFrame::default()
            };
        }

        let (mut num, mut den) = (0.0f64, 0.0f64);
        for (k, &m) in s.mag.iter().enumerate().skip(1) {
            let f = f64::from(s.bin_hz(k, sr));
            if f > CENTROID_HI_HZ {
                break;
            }
            num += f * f64::from(m);
            den += f64::from(m);
        }
        let centroid = if den > 0.0 {
            (num / den) as f32
        } else {
            f32::NAN
        };
        let centroid_harm = if pitch_hz > 0.0 {
            centroid / pitch_hz
        } else {
            f32::NAN
        };

        // Least-squares slope of band level (dB, mean power per bin) on log2 frequency.
        let (mut sx, mut sy, mut sxx, mut sxy, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for &(lo, hi, lx) in &self.bands {
            let p: f64 =
                s.power[lo..=hi].iter().map(|&v| f64::from(v)).sum::<f64>() / (hi - lo + 1) as f64;
            let ly = 10.0 * (p + 1e-20).log10();
            sx += lx;
            sy += ly;
            sxx += lx * lx;
            sxy += lx * ly;
            n += 1.0;
        }
        let tilt = if n >= 2.0 {
            ((n * sxy - sx * sy) / (n * sxx - sx * sx)) as f32
        } else {
            f32::NAN
        };

        TimbreFrame {
            loudness_db: 0.0,
            centroid_hz: centroid,
            centroid_harm,
            tilt_db_oct: tilt,
            cpp_db: self.cpp(),
        }
    }

    /// Smoothed cepstral peak prominence of the current long-window spectrum,
    /// in dB. The power cepstrum is averaged over [`CPP_SMOOTH_SECONDS`] of
    /// quefrency before taking dB, which removes the notches at the signed
    /// cepstrum's zero crossings that otherwise make CPP jump by several dB
    /// between neighbouring pitches.
    fn cpp(&mut self) -> f32 {
        let s = &self.long;
        let floor = s.power.iter().copied().fold(0.0f32, f32::max) * 1e-10 + 1e-30;
        for (c, &p) in self.cep_in.iter_mut().zip(&s.power) {
            *c = Complex32::new(10.0 * (p + floor).log10(), 0.0);
        }
        self.inv
            .process_with_scratch(&mut self.cep_in, &mut self.cep_out, &mut self.cep_scratch)
            .expect("plan sizes");
        let scale = 1.0 / s.len as f64;
        for (pw, &c) in self.cep_power.iter_mut().zip(&self.cep_out) {
            let c = f64::from(c) * scale;
            *pw = c * c;
        }
        let h = self.q_half;
        let smoothed_db = |q: usize| {
            let window = &self.cep_power[q - h..=q + h];
            10.0 * (window.iter().sum::<f64>() / window.len() as f64 + 1e-20).log10()
        };
        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
        let (mut peak_q, mut peak) = (self.q_lo, f64::NEG_INFINITY);
        for q in self.q_lo..=self.q_hi {
            let y = smoothed_db(q);
            let x = q as f64;
            sx += x;
            sy += y;
            sxx += x * x;
            sxy += x * y;
            if y > peak {
                peak = y;
                peak_q = q;
            }
        }
        let n = (self.q_hi - self.q_lo + 1) as f64;
        let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
        let icept = (sy - slope * sx) / n;
        (peak - (icept + slope * peak_q as f64)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weighting_reference_points() {
        let db = |f: f64| 10.0 * a_weight_power(f).log10();
        assert!(db(1000.0).abs() < 0.01);
        assert!((db(100.0) + 19.1).abs() < 0.1, "{}", db(100.0));
        assert!((db(10_000.0) + 2.5).abs() < 0.1, "{}", db(10_000.0));
    }
}
