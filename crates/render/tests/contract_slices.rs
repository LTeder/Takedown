//! The `SliceSource` purity rule (core-types §2.15): every output sample
//! depends only on (take, absolute output index). `SliceBank` reads each
//! union of touching slice spans once, so an impure source would let an
//! inserted hit change an existing hit's slice. These tests show that
//! `slice_purity_violation` and the end-to-end symptom both catch such a
//! source, and that the toy source passes.

use std::sync::Arc;

use proptest::prelude::*;
use takedown_core::control::ControlTrack;
use takedown_core::ids::{Hash32, TakeId};
use takedown_core::seed::{Seed, SeedTree};
use takedown_core::time::{SampleRate, TimelinePos, TimelineSpan};
use takedown_render::contract::testkit::{
    FIXED_RATE_PAIRS, ToyBackend, ToyConfig, ToySlices, fixed_track, hit_event, rate, rate_pair,
    render_cases, slice_purity_violation,
};
use takedown_render::contract::{BindSpec, BoundTrack, RenderBackend, SliceSource};

/// A windowed resampler done wrong: it fades in and out over the first and
/// last 16 samples of whatever span it is handed, as a kernel that reads
/// audio beyond the span as zero would.
struct EdgeFaded(ToySlices);

impl SliceSource for EdgeFaded {
    fn read(&mut self, take: TakeId, span: TimelineSpan, out_rate: SampleRate) -> Option<Vec<f32>> {
        let mut v = self.0.read(take, span, out_rate)?;
        let n = v.len();
        let k = 16.min(n / 2);
        for i in 0..k {
            let g = (i + 1) as f32 / (k + 1) as f32;
            v[i] *= g;
            v[n - 1 - i] *= g;
        }
        Some(v)
    }
}

fn take() -> TakeId {
    TakeId::from_hash(Hash32([0x5a; 32]))
}

fn span(a: u64, b: u64) -> TimelineSpan {
    TimelineSpan::new(TimelinePos(a), TimelinePos(b)).expect("ordered")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// The toy source is a function of the absolute output sample.
    #[test]
    fn toy_slices_are_position_pure(
        (timeline, out) in rate_pair(),
        start in 0u64..2_000_000,
        len in 0u64..200_000,
    ) {
        let mut source = ToySlices::new(timeline);
        let found = slice_purity_violation(&mut source, take(), timeline, out, span(start, start + len));
        prop_assert_eq!(found, None);
    }
}

/// The audit flags an edge-fading source at every fixed rate pair.
#[test]
fn edge_fading_fails_the_audit() {
    for (timeline, out) in FIXED_RATE_PAIRS {
        let (timeline, out) = (rate(timeline), rate(out));
        for len in [1_000u64, 9_600, 100_000] {
            let mut source = EdgeFaded(ToySlices::new(timeline));
            let found = slice_purity_violation(
                &mut source,
                take(),
                timeline,
                out,
                span(4_800, 4_800 + len),
            );
            assert!(found.is_some(), "{timeline:?} -> {out:?}, {len} samples");
        }
    }
}

/// The samples of the first trigger's slice, for a hit-only voice reading
/// 30 ms slices through `source`.
fn first_slice(
    track: &Arc<ControlTrack>,
    out: SampleRate,
    source: &mut dyn SliceSource,
) -> Vec<f32> {
    let mut cfg = ToyConfig::hits(&["kick"]);
    cfg.slice_max_ms = 30;
    let prepared = ToyBackend::new(cfg.clone()).prepare(out, 256);
    let (info, route) = (cfg.info(), cfg.route());
    let tree = SeedTree::new(Seed(0x5_11ce));
    let spec = BindSpec {
        voice: "kit",
        route: &route,
        out_rate: out,
        info: &info,
        prepared,
        seeds: &tree,
    };
    let bound = BoundTrack::bind(Arc::clone(track), &spec, source);
    let id = bound.triggers()[0]
        .kind
        .slice()
        .expect("the hit reads a slice");
    bound.slice(id).to_vec()
}

/// The failure mode end to end: a second hit whose slice touches the first
/// one's makes the bank read their union as one span. With the pure toy
/// source the first slice is unchanged; with the edge-fading one it changes,
/// and comparing slices (as `inserting_a_hit_keeps_other_seeds` does) shows it.
#[test]
fn an_impure_source_changes_a_neighbours_slice() {
    let (timeline, out) = (rate(48_000), rate(44_100));
    // 30 ms at 48 kHz is 1440 samples: the second slice starts where the
    // first ends.
    let track = |hits| Arc::new(fixed_track(timeline, 48_000, &["kick"], vec![], hits, true));
    let alone = track(vec![hit_event(4_800, 4_800, 4_800, 0, 100)]);
    let paired = track(vec![
        hit_event(4_800, 4_800, 4_800, 0, 100),
        hit_event(6_240, 6_240, 4_800, 0, 100),
    ]);

    let pure = |t: &Arc<ControlTrack>| first_slice(t, out, &mut ToySlices::new(timeline));
    assert_eq!(pure(&alone), pure(&paired), "a pure source keeps the slice");

    let impure =
        |t: &Arc<ControlTrack>| first_slice(t, out, &mut EdgeFaded(ToySlices::new(timeline)));
    let (a, b) = (impure(&alone), impure(&paired));
    assert_eq!(a.len(), b.len());
    assert_ne!(
        a, b,
        "an impure source changed the existing slice, and it shows"
    );
}
