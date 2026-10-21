//! Onset candidates.
//!
//! Detection function: SuperFlux (Böck and Widmer 2013), the positive
//! difference between a log-magnitude, 24-bands-per-octave filtered spectrum
//! and a frequency-max-filtered copy of the spectrum two frames earlier, on the
//! 200 Hz grid. Every local maximum is kept as a candidate with its height above
//! a 150 ms trailing average; choosing a threshold is lens and drums work.
//! Each candidate is then refined to a sample position by finding the energy
//! peak of the hit and backtracking to where the envelope rose out of the
//! floor before it (see [`backtrack`]).

use std::cmp::Ordering;

use crate::frame::{CONTROL_HZ, frame_centre};

const BANDS_PER_OCTAVE: f64 = 24.0;
const FB_FMIN_HZ: f64 = 30.0;
const FB_FMAX_HZ: f64 = 17_000.0;
/// Candidate must be the maximum from 10 ms before to 20 ms after. (SuperFlux
/// uses 50 ms after; on two-part beatbox sounds that lets the louder second
/// burst suppress the first and the onset lands 20 to 40 ms late.)
const PRE_MAX_S: f64 = 0.010;
const POST_MAX_S: f64 = 0.020;
/// Trailing average the strength is measured against.
const PRE_AVG_S: f64 = 0.150;
/// Energy envelope smoothing, backtracking threshold (fraction of the rise
/// from floor to peak, in dB) and how far back the floor is searched.
const ENV_SECONDS: f64 = 0.002;
const BACKTRACK_RATIO: f64 = 0.3;
const BACKTRACK_MAX_S: f64 = 0.060;
/// Below this rise the envelope cannot place the onset.
const MIN_REFINE_RISE_DB: f32 = 6.0;

/// Default decision used by `cli analyze`; the drums crate owns the real one.
/// Threshold from the SuperFlux paper; refractory window at the top of the
/// 30 to 60 ms range in `docs/research/08` section 5; a candidate within
/// the continuation window after a kept onset must rise at least
/// `DEFAULT_MIN_RISE_DB` out of its floor, which merges "b + oom" style
/// two-part sounds. Window and rise were chosen on AVP (see docs/STATUS.md).
pub const DEFAULT_THRESHOLD: f32 = 1.1;
pub const DEFAULT_REFRACTORY_S: f64 = 0.060;
pub const DEFAULT_CONTINUATION_S: f64 = 0.100;
pub const DEFAULT_MIN_RISE_DB: f32 = 25.0;

/// One onset candidate: refined position at capture rate, the grid frame it
/// was detected on, its detection-function height above the local average,
/// and how far (dB) the energy envelope rose from its floor to the hit's peak.
/// A small rise means the candidate continues an earlier sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnsetCandidate {
    pub sample: u64,
    pub frame: usize,
    pub strength: f32,
    pub rise_db: f32,
}

/// SuperFlux detection function, fed one magnitude spectrum per grid frame.
#[derive(Debug)]
pub struct Flux {
    filters: Vec<(usize, Vec<f32>)>,
    lag: usize,
    history: Vec<Vec<f32>>,
    current: Vec<f32>,
    count: usize,
}

impl Flux {
    /// `fft_len` is the length of the spectra that will be pushed.
    pub fn new(fft_len: usize, sample_rate: u32) -> Self {
        let sr = f64::from(sample_rate);
        let hz_per_bin = sr / fft_len as f64;
        let top = FB_FMAX_HZ.min(sr / 2.0);
        let mut bins: Vec<usize> = Vec::new();
        let mut i = 0.0;
        loop {
            let f = FB_FMIN_HZ * 2f64.powf(i / BANDS_PER_OCTAVE);
            if f > top {
                break;
            }
            let b = (f / hz_per_bin).round() as usize;
            if bins.last() != Some(&b) {
                bins.push(b);
            }
            i += 1.0;
        }
        let mut filters = Vec::new();
        for w in bins.windows(3) {
            let (start, centre, stop) = (w[0], w[1], w[2]);
            let mut weights: Vec<f32> = (start..=stop)
                .map(|j| match j.cmp(&centre) {
                    Ordering::Less => (j - start) as f32 / (centre - start) as f32,
                    Ordering::Equal => 1.0,
                    Ordering::Greater => (stop - j) as f32 / (stop - centre) as f32,
                })
                .collect();
            let sum: f32 = weights.iter().sum();
            for v in &mut weights {
                *v /= sum;
            }
            filters.push((start, weights));
        }
        // Compare against the frame where the Hann window has fallen to half.
        let hop = sr / CONTROL_HZ as f64;
        let lag = ((fft_len as f64 / 4.0) / hop).round().max(1.0) as usize;
        let n = filters.len();
        Self {
            filters,
            lag,
            history: vec![vec![0.0; n]; lag],
            current: vec![0.0; n],
            count: 0,
        }
    }

    /// Detection-function value for the next frame, given its magnitudes.
    pub fn push(&mut self, mag: &[f32]) -> f32 {
        for (out, (start, w)) in self.current.iter_mut().zip(&self.filters) {
            let band: f32 = mag[*start..*start + w.len()]
                .iter()
                .zip(w)
                .map(|(m, w)| m * w)
                .sum();
            *out = (1.0 + band).log10();
        }
        let slot = self.count % self.lag;
        let reference = &self.history[slot];
        let n = self.current.len();
        let mut flux = 0.0;
        for b in 0..n {
            let lo = b.saturating_sub(1);
            let hi = (b + 1).min(n - 1);
            let r = reference[lo..=hi]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            flux += (self.current[b] - r).max(0.0);
        }
        self.history[slot].copy_from_slice(&self.current);
        self.count += 1;
        flux
    }
}

/// Local maxima of the detection function with their strength, unrefined.
fn peaks(odf: &[f32]) -> Vec<(usize, f32)> {
    let fps = CONTROL_HZ as f64;
    let pre_max = (PRE_MAX_S * fps).round() as usize;
    let post_max = (POST_MAX_S * fps).round() as usize;
    let pre_avg = (PRE_AVG_S * fps).round() as usize;
    let mut out = Vec::new();
    for (n, &v) in odf.iter().enumerate() {
        if v <= 0.0 {
            continue;
        }
        let lo = n.saturating_sub(pre_max);
        let hi = (n + post_max).min(odf.len() - 1);
        if odf[lo..=hi].iter().any(|&u| u > v) {
            continue;
        }
        // Plateau: only its first frame counts (nothing nearby exceeds v).
        if n > 0 && odf[n - 1] >= v {
            continue;
        }
        // Zero padding before the start, as madmom's `mode='constant'`.
        let start = n.saturating_sub(pre_avg);
        let avg = odf[start..=n].iter().sum::<f32>() / (pre_avg + 1) as f32;
        let strength = v - avg;
        if strength > 0.0 {
            out.push((n, strength));
        }
    }
    out
}

/// Where a hit starts, located on one energy envelope (see [`backtrack`]).
/// Returns `None` when the envelope rose less than [`MIN_REFINE_RISE_DB`].
fn locate(
    signal: impl Fn(i64) -> f64,
    box_len: i64,
    lo: i64,
    r0: i64,
    r1: i64,
    reach: i64,
) -> (Option<i64>, f32) {
    let mut prefix = Vec::with_capacity((r1 - lo + 1) as usize);
    prefix.push(0.0f64);
    for i in lo..r1 {
        let v = signal(i);
        prefix.push(prefix.last().copied().unwrap_or(0.0) + v * v);
    }
    // Envelope in dB: mean square over a box centred on i.
    let env = |i: i64| {
        let a = (i - box_len / 2).clamp(lo, r1);
        let b = (a + box_len).clamp(lo, r1);
        let ms = (prefix[(b - lo) as usize] - prefix[(a - lo) as usize]) / (b - a).max(1) as f64;
        10.0 * (ms + 1e-12).log10()
    };
    let (mut peak_i, mut peak) = (r0, f64::NEG_INFINITY);
    for i in r0..r1 {
        let e = env(i);
        if e > peak {
            peak = e;
            peak_i = i;
        }
    }
    let start = (peak_i - reach).max(lo);
    let floor = (start..=peak_i).map(env).fold(f64::INFINITY, f64::min);
    let rise = (peak - floor) as f32;
    if rise < MIN_REFINE_RISE_DB {
        return (None, rise);
    }
    let thr = floor + BACKTRACK_RATIO * (peak - floor);
    let mut i = peak_i;
    while i > start && env(i - 1) > thr {
        i -= 1;
    }
    // The centred box crosses the threshold as soon as its leading half
    // touches the attack, so an instant attack reads up to half a box (1 ms)
    // early. Shifting by half a box made AVP timing slightly worse, so no.
    (Some(i), rise)
}

/// Sample index where the hit near detection frame `frame` starts, and the
/// rise in dB that located it. Finds the energy peak, then walks back to where
/// the envelope (dB) last sat below `floor + BACKTRACK_RATIO * (peak - floor)`,
/// the floor being the quietest point in the preceding [`BACKTRACK_MAX_S`].
/// Audio before the take is silence. If the envelope rose less than
/// [`MIN_REFINE_RISE_DB`] (a click on a sustained sound), the same is tried on
/// the first difference of the signal, where low sustained sounds are quiet;
/// failing that the frame centre is kept. Never earlier than `earliest`.
fn backtrack(
    x: &[f32],
    sample_rate: u32,
    frame: usize,
    half_window: usize,
    earliest: i64,
) -> (u64, f32) {
    let sr = f64::from(sample_rate);
    let len = x.len() as i64;
    let c = frame_centre(frame as u64, sample_rate) as i64;
    let clamp = |i: i64| i.clamp(earliest.max(0), (len - 1).max(0)) as u64;
    let reach = (BACKTRACK_MAX_S * sr).round() as i64;
    let box_len = ((ENV_SECONDS * sr).round() as i64).max(1);
    // The flux peaks while the attack is in the leading half of the window.
    let lo = (c - half_window as i64 / 2 - reach).max(earliest);
    let r0 = (c - half_window as i64 / 2).max(lo);
    let r1 = (c + half_window as i64).min(len);
    if r1 <= r0 {
        return (clamp(c), 0.0);
    }
    let at = |i: i64| {
        if (0..len).contains(&i) {
            f64::from(x[i as usize])
        } else {
            0.0
        }
    };
    let (found, rise) = locate(at, box_len, lo, r0, r1, reach);
    if let Some(i) = found {
        return (clamp(i), rise);
    }
    let (found, _) = locate(|i| at(i) - at(i - 1), box_len, lo, r0, r1, reach);
    (clamp(found.unwrap_or(c)), rise)
}

/// Candidates from a detection function, each refined against the audio.
pub fn candidates(x: &[f32], sample_rate: u32, odf: &[f32], fft_len: usize) -> Vec<OnsetCandidate> {
    let mut out: Vec<OnsetCandidate> = Vec::new();
    for (frame, strength) in peaks(odf) {
        let earliest = out.last().map_or(i64::MIN / 2, |c| c.sample as i64 + 1);
        let (sample, rise_db) = backtrack(x, sample_rate, frame, fft_len / 2, earliest);
        out.push(OnsetCandidate {
            sample,
            frame,
            strength,
            rise_db,
        });
    }
    out
}

/// Parameters of the default onset decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickParams {
    pub threshold: f32,
    pub refractory_s: f64,
    pub continuation_s: f64,
    pub min_rise_db: f32,
}

impl Default for PickParams {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            refractory_s: DEFAULT_REFRACTORY_S,
            continuation_s: DEFAULT_CONTINUATION_S,
            min_rise_db: DEFAULT_MIN_RISE_DB,
        }
    }
}

/// The default decision: candidates at or above the threshold, none within
/// the refractory window of the previous kept onset, and within the
/// continuation window only those that rise far enough out of their floor.
pub fn pick(cands: &[OnsetCandidate], sample_rate: u32, p: &PickParams) -> Vec<OnsetCandidate> {
    let sr = f64::from(sample_rate);
    let refractory = (p.refractory_s * sr).round() as u64;
    let continuation = (p.continuation_s * sr).round() as u64;
    let mut out: Vec<OnsetCandidate> = Vec::new();
    for c in cands.iter().filter(|c| c.strength >= p.threshold) {
        if let Some(prev) = out.last() {
            let gap = c.sample.saturating_sub(prev.sample);
            if gap < refractory || (gap < continuation && c.rise_db < p.min_rise_db) {
                continue;
            }
        }
        out.push(*c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_need_to_dominate_their_neighbourhood() {
        let mut odf = vec![0.0f32; 100];
        odf[17] = 4.5; // a bigger peak follows within 20 ms
        odf[20] = 5.0;
        odf[21] = 4.0; // a bigger peak precedes within 10 ms
        odf[60] = 3.0;
        let p = peaks(&odf);
        assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), vec![20, 60]);
    }

    #[test]
    fn pick_applies_threshold_refractory_and_continuation() {
        let c = |sample, strength, rise_db| OnsetCandidate {
            sample,
            frame: 0,
            strength,
            rise_db,
        };
        let cands = [
            c(0, 2.0, 40.0),
            c(1_000, 3.0, 40.0),  // 21 ms: refractory
            c(4_000, 2.0, 10.0),  // 83 ms, small rise: continuation of the first
            c(6_000, 0.5, 40.0),  // below threshold
            c(10_000, 2.0, 10.0), // 208 ms: a new sound whatever its rise
            c(13_500, 2.0, 30.0), // 73 ms after the last kept, big rise: kept
        ];
        let picked = pick(&cands, 48_000, &PickParams::default());
        assert_eq!(
            picked.iter().map(|c| c.sample).collect::<Vec<_>>(),
            vec![0, 10_000, 13_500]
        );
    }
}
