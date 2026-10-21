//! Viterbi decoding of pYIN candidates into one f0 track with voicing.
//!
//! This is lens work: every field of [`DecodeParams`] is an opinion, and the
//! decoder reads only the stored candidates, so re-decoding a 10-minute take
//! after a slider move never touches audio. It lives in this crate because
//! `cli analyze` needs a default decode for evaluation.
//!
//! Pitch and voicing are decoded separately. The pitch path runs through every
//! frame over pitch bins: candidates are the observations, and the frame's
//! unvoiced probability is spread evenly over all bins as a floor, so the path
//! holds its pitch through consonants and breaths. Pitch moves by at most
//! `max_step_semitones` per frame along a triangular kernel; a jump of about an
//! octave is allowed at a cost of `octave_jump_penalty`, so a sustained octave
//! disagreement between candidates and path can be corrected but a single-frame
//! octave blip cannot win. Voicing is a two-state Viterbi over the frame's
//! in-range voiced probability with a switching cost, so a frame is voiced when
//! that probability is above `voicing_threshold`, smoothed in time.
//!
//! (pYIN couples the two in one HMM with unvoiced likelihood `(1 - p) / bins`
//! per state; that makes almost any frame with a candidate voiced, which gave
//! a 28% voicing false-alarm rate on Vocadito.)

use crate::pitch::{N_CANDIDATES, PitchFrame};

#[derive(Clone, Debug, PartialEq)]
pub struct DecodeParams {
    /// Range prior: lowest and highest decodable f0.
    pub fmin_hz: f32,
    pub fmax_hz: f32,
    /// State resolution.
    pub bins_per_semitone: u32,
    /// Largest pitch move in one 5 ms frame without penalty (pYIN: 35.92 octaves/s).
    pub max_step_semitones: f32,
    /// Probability of a near-octave jump relative to a local move. 0 forbids them.
    pub octave_jump_penalty: f32,
    /// Probability of switching between voiced and unvoiced per frame.
    pub switch_prob: f32,
    /// Voiced probability at which a frame is equally likely voiced or not.
    pub voicing_threshold: f32,
}

impl Default for DecodeParams {
    fn default() -> Self {
        Self {
            fmin_hz: 55.0,
            fmax_hz: 1100.0,
            bins_per_semitone: 5,
            max_step_semitones: 2.155,
            octave_jump_penalty: 1e-3,
            switch_prob: 0.01,
            voicing_threshold: 0.5,
        }
    }
}

/// Decoded track. `f0_hz` is defined on every frame (held through unvoiced
/// frames; 0 throughout if no frame has an in-range candidate); `voiced`
/// says whether to use it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    pub f0_hz: Vec<f32>,
    pub voiced: Vec<bool>,
}

const LOG_FLOOR: f64 = -1e3;

fn ln(p: f64) -> f64 {
    if p > 0.0 {
        p.ln().max(LOG_FLOOR)
    } else {
        LOG_FLOOR
    }
}

/// Pitch-bin grid for the range prior.
struct Grid {
    fmin: f64,
    bps: f64,
    n_bins: usize,
}

impl Grid {
    fn new(params: &DecodeParams) -> Self {
        let bps = f64::from(params.bins_per_semitone.max(1));
        let fmin = f64::from(params.fmin_hz);
        let span = 12.0 * (f64::from(params.fmax_hz) / fmin).log2();
        Self {
            fmin,
            bps,
            n_bins: (span * bps).floor().max(0.0) as usize + 1,
        }
    }

    fn bin_of(&self, hz: f32) -> Option<usize> {
        if hz <= 0.0 {
            return None;
        }
        let b = (12.0 * (f64::from(hz) / self.fmin).log2() * self.bps).round();
        (b >= 0.0 && (b as usize) < self.n_bins).then_some(b as usize)
    }

    fn hz(&self, b: usize) -> f32 {
        (self.fmin * 2f64.powf(b as f64 / (12.0 * self.bps))) as f32
    }

    /// Total probability of the frame's in-range candidates, at most 1.
    fn voiced_p(&self, pf: &PitchFrame) -> f64 {
        (0..N_CANDIDATES)
            .filter(|&c| self.bin_of(pf.cand_hz[c]).is_some())
            .map(|c| f64::from(pf.cand_p[c]))
            .sum::<f64>()
            .min(1.0)
    }

    /// Pitch observation log-likelihoods: candidate probability on its bin
    /// plus the unvoiced probability spread evenly over all bins.
    fn observe(&self, pf: &PitchFrame, obs: &mut [f64]) {
        let floor = (1.0 - self.voiced_p(pf)) / self.n_bins as f64;
        let mut p = [0.0f64; N_CANDIDATES];
        let mut bins = [usize::MAX; N_CANDIDATES];
        for c in 0..N_CANDIDATES {
            if let Some(b) = self.bin_of(pf.cand_hz[c]) {
                bins[c] = b;
                p[c] = f64::from(pf.cand_p[c]);
            }
        }
        let base = ln(floor);
        obs.fill(base);
        for c in 0..N_CANDIDATES {
            if bins[c] != usize::MAX {
                let b = bins[c];
                let mass: f64 = (0..N_CANDIDATES)
                    .filter(|&d| bins[d] == b)
                    .map(|d| p[d])
                    .sum();
                obs[b] = ln(mass + floor);
            }
        }
    }

    /// The exact candidate frequency nearest bin `b` when one is within a
    /// bin of it, else the bin's centre.
    fn report(&self, pf: &PitchFrame, b: usize) -> f32 {
        let mut hz = self.hz(b);
        let mut best = usize::MAX;
        for c in 0..N_CANDIDATES {
            if let Some(cb) = self.bin_of(pf.cand_hz[c]) {
                let dist = cb.abs_diff(b);
                if dist <= 1 && dist < best {
                    best = dist;
                    hz = pf.cand_hz[c];
                }
            }
        }
        hz
    }
}

/// Pitch transition kernel as (bin offset, log probability). Every row uses
/// the same normaliser, so near the range edges, where part of the kernel
/// falls outside, rows sum to less than one. Normalising each row instead
/// makes the edges attractors: a path with nothing to follow drifts to them
/// and the held pitch of a long unvoiced stretch falls by octaves.
fn kernel(params: &DecodeParams, grid: &Grid) -> Vec<(i64, f64)> {
    let bps = grid.bps;
    let width = (f64::from(params.max_step_semitones) * bps).round() as i64;
    let octave = (12.0 * bps).round() as i64;
    let oct_width = (bps / 2.0).round().max(1.0) as i64;
    let mut kernel: Vec<(i64, f64)> = (-width..=width)
        .map(|d| (d, (width + 1 - d.abs()) as f64))
        .collect();
    if params.octave_jump_penalty > 0.0 {
        let peak = (width + 1) as f64 * f64::from(params.octave_jump_penalty);
        for sign in [-1, 1] {
            for d in -oct_width..=oct_width {
                let w = peak * (oct_width + 1 - d.abs()) as f64 / (oct_width + 1) as f64;
                kernel.push((sign * octave + d, w));
            }
        }
    }
    let norm = kernel.iter().map(|(_, w)| w).sum::<f64>().ln();
    kernel
        .into_iter()
        .map(|(d, w)| (d, w.ln() - norm))
        .collect()
}

/// Most probable pitch-bin path.
fn decode_pitch(frames: &[PitchFrame], params: &DecodeParams, grid: &Grid) -> Vec<usize> {
    let n = grid.n_bins;
    let t_len = frames.len();
    let kernel = kernel(params, grid);
    let mut obs = vec![0.0f64; n];
    let mut delta = vec![0.0f64; n];
    let mut next = vec![0.0f64; n];
    let mut back = vec![0u16; t_len * n];
    grid.observe(&frames[0], &mut obs);
    delta.copy_from_slice(&obs);
    for t in 1..t_len {
        grid.observe(&frames[t], &mut obs);
        let row = &mut back[t * n..(t + 1) * n];
        for j in 0..n {
            let mut best = f64::NEG_INFINITY;
            let mut arg = 0u16;
            for &(d, lw) in &kernel {
                let i = j as i64 - d;
                if !(0..n as i64).contains(&i) {
                    continue;
                }
                let i = i as usize;
                let v = delta[i] + lw;
                if v > best {
                    best = v;
                    arg = i as u16;
                }
            }
            next[j] = best + obs[j];
            row[j] = arg;
        }
        std::mem::swap(&mut delta, &mut next);
        // Keep magnitudes bounded over long takes.
        let top = delta.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        for v in &mut delta {
            *v -= top;
        }
    }
    let mut state = delta
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i);
    let mut path = vec![0usize; t_len];
    for t in (0..t_len).rev() {
        path[t] = state;
        if t > 0 {
            state = usize::from(back[t * n + state]);
        }
    }
    path
}

/// Two-state voicing path: voiced likelihood `p`, unvoiced `(1 - p)`
/// rescaled so the two are equal at `voicing_threshold`.
fn decode_voicing(frames: &[PitchFrame], params: &DecodeParams, grid: &Grid) -> Vec<bool> {
    let t_len = frames.len();
    let thr = f64::from(params.voicing_threshold).clamp(1e-6, 1.0 - 1e-6);
    let bias = ln(thr / (1.0 - thr));
    let stay = ln(1.0 - f64::from(params.switch_prob));
    let switch = ln(f64::from(params.switch_prob));
    let obs = |pf: &PitchFrame| {
        let p = grid.voiced_p(pf);
        [ln(1.0 - p) + bias, ln(p)]
    };
    let mut delta = obs(&frames[0]);
    let mut back = vec![[0u8; 2]; t_len];
    for t in 1..t_len {
        let o = obs(&frames[t]);
        let mut next = [0.0; 2];
        for (j, nj) in next.iter_mut().enumerate() {
            let from_same = delta[j] + stay;
            let from_other = delta[1 - j] + switch;
            let (v, i) = if from_same >= from_other {
                (from_same, j)
            } else {
                (from_other, 1 - j)
            };
            *nj = v + o[j];
            back[t][j] = i as u8;
        }
        let top = next[0].max(next[1]);
        delta = [next[0] - top, next[1] - top];
    }
    let mut state = usize::from(delta[1] > delta[0]);
    let mut out = vec![false; t_len];
    for t in (0..t_len).rev() {
        out[t] = state == 1;
        if t > 0 {
            state = usize::from(back[t][state]);
        }
    }
    out
}

/// Decodes the most probable f0 path and voicing from the candidates.
pub fn decode(frames: &[PitchFrame], params: &DecodeParams) -> Decoded {
    if frames.is_empty() {
        return Decoded::default();
    }
    let grid = Grid::new(params);
    assert!(u16::try_from(grid.n_bins).is_ok(), "pitch grid too fine");
    if frames.iter().all(|pf| grid.voiced_p(pf) <= 0.0) {
        return Decoded {
            f0_hz: vec![0.0; frames.len()],
            voiced: vec![false; frames.len()],
        };
    }
    let path = decode_pitch(frames, params, &grid);
    Decoded {
        f0_hz: path
            .iter()
            .zip(frames)
            .map(|(&b, pf)| grid.report(pf, b))
            .collect(),
        voiced: decode_voicing(frames, params, &grid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(hz: f32, p: f32) -> PitchFrame {
        PitchFrame {
            cand_hz: [hz, 0.0, 0.0],
            cand_p: [p, 0.0, 0.0],
            voiced_p: p,
        }
    }

    #[test]
    fn steady_candidates_decode_to_themselves() {
        let frames = vec![frame(220.0, 0.9); 50];
        let d = decode(&frames, &DecodeParams::default());
        assert!(d.voiced.iter().all(|&v| v));
        assert!(d.f0_hz.iter().all(|&f| (f - 220.0).abs() < 1e-3));
    }

    #[test]
    fn single_frame_octave_blip_is_rejected() {
        let mut frames = vec![frame(220.0, 0.9); 60];
        frames[30] = PitchFrame {
            cand_hz: [440.0, 220.0, 0.0],
            cand_p: [0.6, 0.3, 0.0],
            voiced_p: 0.9,
        };
        let d = decode(&frames, &DecodeParams::default());
        assert!((d.f0_hz[30] - 220.0).abs() < 1e-3, "{}", d.f0_hz[30]);
    }

    #[test]
    fn sustained_octave_leap_is_followed() {
        let mut frames = vec![frame(220.0, 0.9); 40];
        frames.extend(vec![frame(440.0, 0.9); 40]);
        let d = decode(&frames, &DecodeParams::default());
        assert!((d.f0_hz[10] - 220.0).abs() < 1e-3);
        assert!((d.f0_hz[70] - 440.0).abs() < 1e-3);
    }

    #[test]
    fn pitch_is_held_through_long_unvoiced_gaps() {
        for gap in [10, 50, 400] {
            let mut frames = vec![frame(220.0, 0.9); 50];
            frames.extend(vec![PitchFrame::default(); gap]);
            frames.extend(vec![frame(220.0, 0.9); 50]);
            let d = decode(&frames, &DecodeParams::default());
            let held = &d.f0_hz[50..50 + gap];
            assert!(
                held.iter().all(|&f| (f - 220.0).abs() < 2.0),
                "gap {gap}: {held:?}"
            );
            assert!(d.voiced[50..50 + gap].iter().all(|&v| !v));
        }
    }

    #[test]
    fn no_candidates_means_unvoiced() {
        let frames = vec![PitchFrame::default(); 20];
        let d = decode(&frames, &DecodeParams::default());
        assert!(d.voiced.iter().all(|&v| !v));
        assert!(d.f0_hz.iter().all(|&f| f == 0.0));
    }

    #[test]
    fn empty_input_decodes_to_empty() {
        assert_eq!(decode(&[], &DecodeParams::default()), Decoded::default());
    }
}
