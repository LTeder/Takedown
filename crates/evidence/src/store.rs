//! The analysis as core `Evidence`: what is cached, keyed and handed to the lens.
//!
//! Columns written by analyser v1: the three pitch candidates and their
//! probabilities, voiced probability, A-weighted loudness, centroid in Hz and
//! in harmonic units, tilt, CPP and the onset detection function. Vowel and
//! class-hint columns are not produced yet. Undefined values (spectral shape of
//! silence, harmonic units without a candidate) are NaN, which core allows in
//! those columns only.

use takedown_core::evidence::{
    Column, Evidence, EvidenceError, EvidenceHeader, OnsetCandidate, Provenance,
};
use takedown_core::ids::TakeId;
use takedown_core::key::KeyError;
use takedown_core::params::{ParamValue, Params};
use takedown_core::time::{SampleRate, TimelinePos};

use crate::pitch::{ANALYSIS_FMAX_HZ, ANALYSIS_FMIN_HZ, N_CANDIDATES, PitchFrame, WINDOW_SECONDS};
use crate::timbre::{LOUDNESS_SECONDS, SPECTRAL_SECONDS};
use crate::{ANALYZER_ID, ANALYZER_VERSION, Analysis};

/// Why an analysis could not become `Evidence`.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreError {
    /// Core accepts 1 kHz to 1.536 MHz.
    SampleRate(u32),
    Key(KeyError),
    Evidence(EvidenceError),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SampleRate(hz) => write!(f, "sample rate {hz} Hz is outside what core accepts"),
            Self::Key(e) => write!(f, "evidence key: {e}"),
            Self::Evidence(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// This analyser's provenance: its id, version and the fixed constants that
/// shape every column. No user setting ever appears here.
pub fn provenance() -> Provenance {
    let float = |v: f64| ParamValue::Float(v as f32);
    let params: Params = [
        ("pitch_fmin_hz", float(ANALYSIS_FMIN_HZ)),
        ("pitch_fmax_hz", float(ANALYSIS_FMAX_HZ)),
        ("yin_window_s", float(WINDOW_SECONDS)),
        ("loudness_window_s", float(LOUDNESS_SECONDS)),
        ("spectral_window_s", float(SPECTRAL_SECONDS)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    Provenance {
        analyzer: ANALYZER_ID.to_string(),
        version: ANALYZER_VERSION,
        models: std::collections::BTreeMap::new(),
        params,
    }
}

/// Converts an analysis of the take with id `take` into validated evidence.
pub fn to_evidence(a: &Analysis, take: TakeId) -> Result<Evidence, StoreError> {
    let rate = SampleRate::new(a.sample_rate).ok_or(StoreError::SampleRate(a.sample_rate))?;
    let provenance = provenance();
    let key = provenance.key(take).map_err(StoreError::Key)?;
    let header = EvidenceHeader {
        key,
        take,
        take_rate: rate,
        take_frames: a.n_samples,
        provenance,
    };
    let col = |f: &dyn Fn(usize) -> f32| (0..a.frames()).map(f).collect::<Vec<f32>>();
    let cand_hz = |c: usize| col(&|k| a.pitch[k].cand_hz[c]);
    let cand_p = |c: usize| col(&|k| a.pitch[k].cand_p[c]);
    let columns = vec![
        (Column::F0Hz0, cand_hz(0)),
        (Column::F0Hz1, cand_hz(1)),
        (Column::F0Hz2, cand_hz(2)),
        (Column::F0Sal0, cand_p(0)),
        (Column::F0Sal1, cand_p(1)),
        (Column::F0Sal2, cand_p(2)),
        (Column::VoicingP, col(&|k| a.pitch[k].voiced_p)),
        (Column::LoudnessDbA, col(&|k| a.timbre[k].loudness_db)),
        (Column::CentroidHz, col(&|k| a.timbre[k].centroid_hz)),
        (Column::BrightnessHarm, col(&|k| a.timbre[k].centroid_harm)),
        (Column::TiltDbOct, col(&|k| a.timbre[k].tilt_db_oct)),
        (Column::CppDb, col(&|k| a.timbre[k].cpp_db)),
        (Column::Odf, a.odf.clone()),
    ];
    let onsets = a
        .onsets
        .iter()
        .map(|c| OnsetCandidate {
            at: TimelinePos(c.sample),
            strength: c.strength,
            rise_db: c.rise_db,
        })
        .collect();
    Evidence::new(header, columns, onsets).map_err(StoreError::Evidence)
}

/// The stored pitch observations, for re-decoding without the audio. Frames
/// are zero (no candidate) where the pitch columns are absent.
pub fn pitch_frames(e: &Evidence) -> Vec<PitchFrame> {
    let n = e.n_frames();
    let hz = [Column::F0Hz0, Column::F0Hz1, Column::F0Hz2].map(|c| e.column(c));
    let sal = [Column::F0Sal0, Column::F0Sal1, Column::F0Sal2].map(|c| e.column(c));
    let voiced = e.column(Column::VoicingP);
    (0..n)
        .map(|k| {
            let mut pf = PitchFrame::default();
            for c in 0..N_CANDIDATES {
                pf.cand_hz[c] = hz[c].map_or(0.0, |v| v[k]);
                pf.cand_p[c] = sal[c].map_or(0.0, |v| v[k]);
            }
            pf.voiced_p = voiced.map_or(0.0, |v| v[k]);
            pf
        })
        .collect()
}
