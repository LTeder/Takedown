//! Parameter-free DSP analysis of a take: pitch candidates, voicing, loudness,
//! brightness, breathiness and onset candidates on the 200 Hz control grid.
//!
//! Everything here is observation. The only opinionated code is
//! [`decode::decode`], which turns stored pitch candidates into one f0 track
//! and is called by the lens with its own settings (and by `cli analyze` with
//! defaults for evaluation), and [`onset::pick`], the default onset decision.
//! Recipes follow `docs/research/08-review-ml-plan.md` section 2.

pub mod decode;
pub mod frame;
pub mod onset;
pub mod pitch;
pub mod store;
pub mod timbre;

use realfft::RealFftPlanner;

use crate::frame::{frame_centre, frame_count};
use crate::onset::{Flux, OnsetCandidate};
use crate::pitch::{PitchFrame, Yin};
use crate::timbre::{Timbre, TimbreFrame};

/// Identifies this analyser in evidence provenance. Bump the version whenever
/// any output for the same input changes.
pub const ANALYZER_ID: &str = "takedown-evidence-dsp";
pub const ANALYZER_VERSION: u32 = 1;

/// Per-frame observations for one mono take, frame `k` centred on sample
/// `floor(k * sample_rate / 200)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    pub sample_rate: u32,
    pub n_samples: u64,
    pub pitch: Vec<PitchFrame>,
    pub timbre: Vec<TimbreFrame>,
    /// Onset detection function per frame.
    pub odf: Vec<f32>,
    /// Every onset candidate, in time order, with its strength.
    pub onsets: Vec<OnsetCandidate>,
}

impl Analysis {
    pub fn frames(&self) -> usize {
        self.pitch.len()
    }
}

/// Analyses a mono signal at its capture rate. Non-finite samples count as 0.
/// Audio before the first sample is silence, so a take that starts mid-sound
/// has an onset candidate at its first frame.
pub fn analyze(x: &[f32], sample_rate: u32) -> Analysis {
    assert!(sample_rate > 0, "sample rate must be positive");
    // One NaN would otherwise poison every average it touches.
    if x.iter().any(|v| !v.is_finite()) {
        let clean: Vec<f32> = x
            .iter()
            .map(|&v| if v.is_finite() { v } else { 0.0 })
            .collect();
        return analyze(&clean, sample_rate);
    }
    let n_samples = x.len() as u64;
    let n = frame_count(n_samples, sample_rate);
    let mut planner = RealFftPlanner::new();
    let mut yin = Yin::new(&mut planner, sample_rate);
    let mut timbre = Timbre::new(&mut planner, sample_rate);
    let mut flux = Flux::new(timbre.spectral_len(), sample_rate);
    let mut out = Analysis {
        sample_rate,
        n_samples,
        pitch: Vec::with_capacity(n),
        timbre: Vec::with_capacity(n),
        odf: Vec::with_capacity(n),
        onsets: Vec::new(),
    };
    for k in 0..n {
        let c = frame_centre(k as u64, sample_rate);
        let pf = yin.analyze(x, c);
        out.timbre.push(timbre.analyze(x, c, pf.cand_hz[0]));
        out.odf.push(flux.push(timbre.spectral_mag()));
        out.pitch.push(pf);
    }
    out.onsets = onset::candidates(x, sample_rate, &out.odf, timbre.spectral_len());
    out
}
