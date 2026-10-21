//! Analysis to core `Evidence` and back.

use std::f64::consts::PI;

use takedown_core::evidence::{Column, Evidence};
use takedown_core::ids::TakeId;
use takedown_core::take::TakeHasher;
use takedown_core::time::{SampleRate, frames_covering};
use takedown_evidence::analyze;
use takedown_evidence::decode::{DecodeParams, decode};
use takedown_evidence::frame::frame_count;
use takedown_evidence::store::{pitch_frames, provenance, to_evidence};

fn take_of(x: &[f32], sr: u32) -> TakeId {
    let mut h = TakeHasher::new(SampleRate::new(sr).unwrap(), 1);
    h.update(x).unwrap();
    h.finish().unwrap().0
}

fn hum(sr: u32) -> Vec<f32> {
    (0..sr)
        .map(|i| {
            let t = f64::from(i) / f64::from(sr);
            let gate = if (0.2..0.8).contains(&t) { 1.0 } else { 0.0 };
            (gate * 0.3 * (2.0 * PI * 196.0 * t).sin()) as f32
        })
        .collect()
}

#[test]
fn frame_grid_matches_core() {
    for sr in [
        8_000, 16_000, 22_050, 44_100, 48_000, 88_200, 96_000, 192_000,
    ] {
        let rate = SampleRate::new(sr).unwrap();
        for n in [0u64, 1, 219, 220, 221, 441, 44_100, 26_460_000] {
            assert_eq!(
                frame_count(n, sr) as u64,
                frames_covering(n, rate),
                "{sr} Hz, {n} samples"
            );
        }
    }
}

#[test]
fn analysis_becomes_valid_evidence_and_decodes_the_same() {
    let sr = 44_100;
    let x = hum(sr);
    let a = analyze(&x, sr);
    let take = take_of(&x, sr);
    let e = to_evidence(&a, take).expect("valid evidence");
    assert_eq!(e.n_frames(), a.frames());
    assert_eq!(e.header().take, take);
    assert_eq!(e.header().key, provenance().key(take).unwrap());
    assert_eq!(e.onsets().len(), a.onsets.len());
    // Silent frames carry NaN brightness, which core accepts in that column.
    assert!(e.column(Column::CentroidHz).unwrap()[0].is_nan());
    assert!(e.column(Column::Vowel0).is_none());

    // The lens re-decodes from stored evidence exactly as from the analysis.
    let params = DecodeParams::default();
    assert_eq!(
        decode(&pitch_frames(&e), &params),
        decode(&a.pitch, &params)
    );

    // And the evidence survives its container.
    let back = Evidence::from_container(&e.to_container(), e.header().key).unwrap();
    assert_eq!(back.content_hash(), e.content_hash());
}

#[test]
fn provenance_is_deterministic_and_rate_free() {
    let x = hum(48_000);
    let take = take_of(&x, 48_000);
    assert_eq!(
        provenance().key(take).unwrap(),
        provenance().key(take).unwrap()
    );
    assert!(!provenance().params.is_empty());
}

#[test]
fn rates_core_refuses_are_an_error_not_a_panic() {
    let x = vec![0.0f32; 500];
    let a = analyze(&x, 500);
    let take = take_of(&hum(8_000), 8_000);
    assert!(to_evidence(&a, take).is_err());
}
