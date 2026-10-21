//! core-types §3.6: `process_path_never_allocates`. This binary installs the
//! testkit's counting allocator, which counts per thread.

use std::hint::black_box;
use std::sync::Arc;

use proptest::collection::vec;
use proptest::prelude::*;
use takedown_core::control::Link;
use takedown_core::seed::{Seed, SeedTree};
use takedown_render::contract::testkit::{
    CountingAlloc, ToyConfig, ToyVoice, alloc_calls, fixed_track, gate_span, hit_event, rate,
    render_case, render_cases,
};
use takedown_render::contract::{Driver, RenderBackend};

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// Starts the voice at every segment and processes it in the given block
/// lengths through that segment and into the next; returns the allocations
/// made by `start` and `process`, which must be none.
fn allocations(v: &ToyVoice, seq: &[usize]) -> u64 {
    let t = &v.bound;
    let len = v.render_len();
    let mut b = v.backend();
    let mut d = Driver::new(&b.info(), v.prepared, v.max_block);
    let mut out = vec![0.0f32; len as usize];
    let mut made = 0;
    for (k, s) in t.segments().iter().enumerate() {
        let stop = t
            .segments()
            .get(k + 1)
            .map_or(len, |n| n.start.0 + v.max_block as u64)
            .min(len) as usize;
        let mut o = s.start.0 as usize;
        let before = alloc_calls();
        d.start(&mut b, t, k);
        for &n in seq.iter().cycle() {
            if o >= stop {
                break;
            }
            let n = n.clamp(1, stop - o);
            d.process(&mut b, t, &mut out[o..o + n]);
            o += n;
        }
        made += alloc_calls() - before;
    }
    black_box(&out);
    made
}

fn counter_counts() {
    let before = alloc_calls();
    drop(black_box(vec![1u8; 64]));
    assert!(
        alloc_calls() >= before + 2,
        "the counting allocator is not counting"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// No alloc, dealloc or realloc during `start` and `process`, for blocks
    /// of 1..=max_block, from every segment start.
    #[test]
    fn process_path_never_allocates(
        (case, seq) in render_case().prop_flat_map(|c| {
            let m = c.max_block;
            (Just(c), vec(1..=m, 1..=16))
        })
    ) {
        counter_counts();
        for v in case.bind() {
            prop_assert_eq!(allocations(&v, &seq), 0, "allocations on the process path");
        }
    }
}

/// One voice that exercises everything at once: phrase and hits, slices,
/// cells, latency pre-roll, several segments.
#[test]
fn process_path_never_allocates_with_every_hook() {
    counter_counts();
    let r = rate(44_100);
    let ms = |m: u64| m * 441 / 10;
    let track = Arc::new(fixed_track(
        r,
        ms(3_000),
        &["kick", "snare"],
        vec![
            gate_span(ms(100), ms(400), Link::Detached),
            gate_span(ms(400), ms(500), Link::Retrigger),
            gate_span(ms(1_500), ms(1_700), Link::Detached),
        ],
        vec![
            hit_event(ms(50), ms(48), ms(80), 0, 100),
            hit_event(ms(900), ms(900), ms(300), 1, 100),
            hit_event(ms(905), ms(900), ms(300), 0, 80),
            hit_event(ms(2_500), ms(2_501), ms(200), 1, 127),
        ],
        true,
    ));
    let mut cfg = ToyConfig::hits(&["kick", "snare"]);
    cfg.phrase = true;
    cfg.latency = 64;
    cfg.cell = 32;
    cfg.tail = 30;
    cfg.release = 30;
    cfg.slice_max_ms = 30;
    let v = ToyVoice::bind(
        &cfg,
        &track,
        rate(48_000),
        &SeedTree::new(Seed(3)),
        "all",
        256,
    );
    assert!(v.bound.segments().len() >= 4);
    assert!(v.bound.triggers().iter().any(|t| t.kind.slice().is_some()));
    for seq in [&[1usize][..], &[7, 220, 221, 256][..], &[256][..]] {
        assert_eq!(allocations(&v, seq), 0, "blocks {seq:?}");
    }
}
