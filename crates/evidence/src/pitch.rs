//! pYIN pitch candidates (Mauch and Dixon 2014, following the official pYIN
//! software's threshold and trough priors rather than the paper's).
//!
//! Per frame this stores observations only: the three most probable YIN troughs
//! as (frequency, probability) and the total voiced probability. Choosing a
//! path through them is the decoder's job ([`crate::decode`]), because range
//! priors and jump penalties are lens settings.

use std::sync::Arc;

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::frame::read_padded;

/// Lowest frequency a candidate may have. Below every human voice.
pub const ANALYSIS_FMIN_HZ: f64 = 40.0;
/// Highest frequency a candidate may have.
pub const ANALYSIS_FMAX_HZ: f64 = 2000.0;
/// YIN integration window.
pub(crate) const WINDOW_SECONDS: f64 = 0.025;
/// Candidates kept per frame.
pub const N_CANDIDATES: usize = 3;

const N_THRESHOLDS: usize = 100;
/// Beta(2, 18) prior over thresholds: mean 0.1, as in pYIN.
const BETA_A: i32 = 2;
const BETA_B: i32 = 18;
/// Boltzmann prior over trough order: earlier (higher-frequency) troughs win.
const BOLTZMANN: f64 = 2.0;
/// Probability mass given to the global minimum when no trough is below threshold.
const NO_TROUGH_PROB: f64 = 0.01;

/// Pitch observations for one frame. Unused candidate slots are 0 Hz with 0 probability.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PitchFrame {
    pub cand_hz: [f32; N_CANDIDATES],
    pub cand_p: [f32; N_CANDIDATES],
    /// Sum of all trough probabilities, at most 1.
    pub voiced_p: f32,
}

/// Reusable pYIN analyser for one sample rate.
pub struct Yin {
    sample_rate: u32,
    w: usize,
    tau_min: usize,
    tau_max: usize,
    fwd: Arc<dyn RealToComplex<f32>>,
    inv: Arc<dyn ComplexToReal<f32>>,
    frame: Vec<f32>,
    a: Vec<f32>,
    b: Vec<f32>,
    fa: Vec<Complex32>,
    fb: Vec<Complex32>,
    corr: Vec<f32>,
    scratch_f: Vec<Complex32>,
    scratch_i: Vec<Complex32>,
    prefix: Vec<f64>,
    cmnd: Vec<f64>,
    troughs: Vec<(usize, f64)>,
    trough_p: Vec<f64>,
    beta_p: [f64; N_THRESHOLDS],
}

impl std::fmt::Debug for Yin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Yin")
            .field("sample_rate", &self.sample_rate)
            .field("w", &self.w)
            .field("tau_min", &self.tau_min)
            .field("tau_max", &self.tau_max)
            .finish_non_exhaustive()
    }
}

/// Regularised incomplete beta function for integer parameters.
fn beta_cdf(x: f64, a: i32, b: i32) -> f64 {
    // I_x(a, b) = sum_{j=a}^{n} C(n, j) x^j (1-x)^(n-j), n = a + b - 1.
    let n = a + b - 1;
    let mut sum = 0.0;
    let mut binom = 1.0f64;
    for j in 0..=n {
        if j > 0 {
            binom *= f64::from(n - j + 1) / f64::from(j);
        }
        if j >= a {
            sum += binom * x.powi(j) * (1.0 - x).powi(n - j);
        }
    }
    sum
}

fn boltzmann_pmf(k: usize, n: usize) -> f64 {
    let l = BOLTZMANN;
    (1.0 - (-l).exp()) * (-l * k as f64).exp() / (1.0 - (-l * n as f64).exp())
}

impl Yin {
    pub fn new(planner: &mut RealFftPlanner<f32>, sample_rate: u32) -> Self {
        let sr = f64::from(sample_rate);
        let w = (WINDOW_SECONDS * sr).round() as usize;
        let tau_max = (sr / ANALYSIS_FMIN_HZ).ceil() as usize;
        let tau_min = ((sr / ANALYSIS_FMAX_HZ).floor() as usize).max(2);
        let n = w + tau_max + 1;
        let m = (n + w).next_power_of_two();
        let fwd = planner.plan_fft_forward(m);
        let inv = planner.plan_fft_inverse(m);
        let mut beta_p = [0.0; N_THRESHOLDS];
        for (i, p) in beta_p.iter_mut().enumerate() {
            let lo = i as f64 / N_THRESHOLDS as f64;
            let hi = (i + 1) as f64 / N_THRESHOLDS as f64;
            *p = beta_cdf(hi, BETA_A, BETA_B) - beta_cdf(lo, BETA_A, BETA_B);
        }
        Self {
            sample_rate,
            w,
            tau_min,
            tau_max,
            frame: vec![0.0; n],
            a: fwd.make_input_vec(),
            b: fwd.make_input_vec(),
            fa: fwd.make_output_vec(),
            fb: fwd.make_output_vec(),
            corr: inv.make_output_vec(),
            scratch_f: fwd.make_scratch_vec(),
            scratch_i: inv.make_scratch_vec(),
            prefix: vec![0.0; n + 1],
            cmnd: vec![1.0; tau_max + 2],
            troughs: Vec::with_capacity(tau_max),
            trough_p: Vec::with_capacity(tau_max),
            beta_p,
            fwd,
            inv,
        }
    }

    /// Fills `self.cmnd` with the cumulative mean normalised difference of the
    /// frame centred on `centre`. Returns false for a silent frame.
    fn difference(&mut self, x: &[f32], centre: u64) -> bool {
        // The integration window is centred on the frame centre; lagged copies
        // reach up to tau_max past its end.
        let n = self.frame.len();
        let start = centre as i64 - (self.w / 2) as i64;
        read_padded(x, start, &mut self.frame);

        self.prefix[0] = 0.0;
        for (i, &s) in self.frame.iter().enumerate() {
            self.prefix[i + 1] = self.prefix[i] + f64::from(s) * f64::from(s);
        }
        let e_a = self.prefix[self.w];
        if e_a < 1e-10 * self.w as f64 {
            return false;
        }

        // r(tau) = sum_{j<w} a_j b_{j+tau} via one forward FFT each and one inverse.
        self.a.fill(0.0);
        self.a[..self.w].copy_from_slice(&self.frame[..self.w]);
        self.b.fill(0.0);
        self.b[..n].copy_from_slice(&self.frame);
        self.fwd
            .process_with_scratch(&mut self.a, &mut self.fa, &mut self.scratch_f)
            .expect("plan sizes");
        self.fwd
            .process_with_scratch(&mut self.b, &mut self.fb, &mut self.scratch_f)
            .expect("plan sizes");
        for (fa, fb) in self.fa.iter_mut().zip(&self.fb) {
            *fa = fa.conj() * fb;
        }
        let last = self.fa.len() - 1;
        self.fa[0].im = 0.0;
        self.fa[last].im = 0.0;
        self.inv
            .process_with_scratch(&mut self.fa, &mut self.corr, &mut self.scratch_i)
            .expect("plan sizes");
        let scale = 1.0 / self.corr.len() as f64;

        // d'(tau) = d(tau) * tau / sum_{j=1..tau} d(j)
        self.cmnd[0] = 1.0;
        let mut running = 0.0;
        for tau in 1..=self.tau_max {
            let e_b = self.prefix[tau + self.w] - self.prefix[tau];
            let r = f64::from(self.corr[tau]) * scale;
            let d = (e_a + e_b - 2.0 * r).max(0.0);
            running += d;
            self.cmnd[tau] = if running > 0.0 {
                d * tau as f64 / running
            } else {
                1.0
            };
        }
        true
    }

    /// pYIN observations for the frame centred on sample `centre`.
    pub fn analyze(&mut self, x: &[f32], centre: u64) -> PitchFrame {
        if !self.difference(x, centre) {
            return PitchFrame::default();
        }
        let c = &self.cmnd;
        self.troughs.clear();
        for tau in self.tau_min..self.tau_max {
            if c[tau] < c[tau - 1] && c[tau] <= c[tau + 1] {
                self.troughs.push((tau, c[tau]));
            }
        }
        if self.troughs.is_empty() {
            return PitchFrame::default();
        }

        // Every threshold shares its prior mass among the troughs below it,
        // weighted by a Boltzmann prior on their order.
        self.trough_p.clear();
        self.trough_p.resize(self.troughs.len(), 0.0);
        let global_min = self
            .troughs
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
            .map_or(0, |(i, _)| i);
        for (i, &bp) in self.beta_p.iter().enumerate() {
            let thr = (i + 1) as f64 / N_THRESHOLDS as f64;
            let below = self.troughs.iter().filter(|t| t.1 < thr).count();
            if below == 0 {
                self.trough_p[global_min] += NO_TROUGH_PROB * bp;
                continue;
            }
            let mut pos = 0;
            for (j, t) in self.troughs.iter().enumerate() {
                if t.1 < thr {
                    self.trough_p[j] += bp * boltzmann_pmf(pos, below);
                    pos += 1;
                }
            }
        }

        let mut out = PitchFrame {
            voiced_p: self.trough_p.iter().sum::<f64>().min(1.0) as f32,
            ..PitchFrame::default()
        };
        for (j, &(tau, _)) in self.troughs.iter().enumerate() {
            let p = self.trough_p[j];
            if p <= 0.0 {
                continue;
            }
            // Insert into the top-N list, sorted by probability, highest first.
            let slot = out.cand_p.iter().position(|&q| p > f64::from(q));
            if let Some(slot) = slot {
                for s in (slot + 1..N_CANDIDATES).rev() {
                    out.cand_p[s] = out.cand_p[s - 1];
                    out.cand_hz[s] = out.cand_hz[s - 1];
                }
                out.cand_p[slot] = p as f32;
                out.cand_hz[slot] = self.refine_hz(tau);
            }
        }
        out
    }

    /// Frequency of trough `tau` after parabolic interpolation.
    fn refine_hz(&self, tau: usize) -> f32 {
        let c = &self.cmnd;
        let (l, m, r) = (c[tau - 1], c[tau], c[tau + 1]);
        let den = l - 2.0 * m + r;
        let shift = if den.abs() > 1e-12 {
            (0.5 * (l - r) / den).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        (f64::from(self.sample_rate) / (tau as f64 + shift)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_prior_sums_to_one_with_mean_point_one() {
        let total: f64 = (0..N_THRESHOLDS)
            .map(|i| {
                beta_cdf((i + 1) as f64 / 100.0, BETA_A, BETA_B)
                    - beta_cdf(i as f64 / 100.0, BETA_A, BETA_B)
            })
            .sum();
        assert!((total - 1.0).abs() < 1e-12);
        // Closed form for (2, 18): 1 - (1-x)^19 - 19x(1-x)^18.
        let x: f64 = 0.1;
        let closed = 1.0 - (1.0 - x).powi(19) - 19.0 * x * (1.0 - x).powi(18);
        assert!((beta_cdf(x, 2, 18) - closed).abs() < 1e-12);
    }

    #[test]
    fn boltzmann_pmf_normalises() {
        for n in 1..6 {
            let s: f64 = (0..n).map(|k| boltzmann_pmf(k, n)).sum();
            assert!((s - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn pure_tone_top_candidate_is_the_tone() {
        let sr = 44_100;
        let f = 196.0;
        let x: Vec<f32> = (0..sr)
            .map(|i| {
                (0.5 * (2.0 * std::f64::consts::PI * f * f64::from(i) / f64::from(sr)).sin()) as f32
            })
            .collect();
        let mut planner = RealFftPlanner::new();
        let mut yin = Yin::new(&mut planner, sr);
        let pf = yin.analyze(&x, 22_050);
        let cents = 1200.0 * (f64::from(pf.cand_hz[0]) / f).log2();
        assert!(cents.abs() < 2.0, "{pf:?}");
        assert!(pf.voiced_p > 0.9, "{pf:?}");
    }

    #[test]
    fn silence_has_no_candidates() {
        let x = vec![0.0f32; 48_000];
        let mut planner = RealFftPlanner::new();
        let mut yin = Yin::new(&mut planner, 48_000);
        assert_eq!(yin.analyze(&x, 24_000), PitchFrame::default());
    }
}
