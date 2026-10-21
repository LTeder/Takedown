//! core-types §3.6: `segment_plan_is_sound`,
//! `latency_compensation_is_a_pure_shift`, `release_survives_to_declared_tail`
//! and `tail_overrun_flags_dishonest_tails`.

use std::sync::Arc;

use proptest::prelude::*;
use proptest::sample::select;
use takedown_core::control::Link;
use takedown_core::seed::{Seed, SeedTree};
use takedown_core::time::{MIN_GAP_FRAMES, RenderPos, SampleRate, control_index, frame_start};
use takedown_render::contract::testkit::{
    ToyConfig, ToyVoice, first_bit_difference, fixed_track, gate_span, hit_event, rate, rate_pair,
    render_case, render_cases, render_mono,
};
use takedown_render::contract::{BoundKind, BoundTrack, Prepared, activity_frames, segment_plan};

/// The output time an event's own sound ends, before tail and latency: a
/// hit's `at + dur`; a gate event's next phrase event (a NoteOff's own `at`).
fn event_end(t: &BoundTrack, i: usize) -> u64 {
    let trigs = t.triggers();
    let g = &trigs[i];
    match g.kind {
        BoundKind::Hit { dur, .. } => g.at.0 + dur,
        BoundKind::NoteOff => g.at.0,
        BoundKind::NoteOn { .. } | BoundKind::Retrigger { .. } => trigs[i + 1..]
            .iter()
            .find(|n| !matches!(n.kind, BoundKind::Hit { .. }))
            .map_or(u64::MAX, |n| n.at.0),
    }
}

/// The toy's release in output samples.
fn release_samples(frames: u32, out: SampleRate) -> u64 {
    u64::from(frames) * u64::from(out.hz()) / 200
}

fn check_plan(v: &ToyVoice) -> Result<(), TestCaseError> {
    let t = &v.bound;
    let out = t.rate();
    let segs = t.segments();
    prop_assert!(!segs.is_empty());
    prop_assert_eq!(segs[0].start, RenderPos(0), "the plan starts at 0");
    prop_assert_eq!(t.end(), segs[segs.len() - 1].quiet_from);
    for (i, s) in segs.iter().enumerate() {
        prop_assert_eq!(s.start.0, frame_start(s.frame, out));
        prop_assert!(s.quiet_from >= s.start);
        if let Some(n) = segs.get(i + 1) {
            prop_assert!(n.start > s.start, "starts strictly increase");
            let quiet_frames = n.frame - control_index(s.quiet_from.0, out).0;
            prop_assert!(
                quiet_frames >= MIN_GAP_FRAMES,
                "a quiet part under 50 ms: {:?}",
                segs
            );
            prop_assert!(n.start.0 - s.quiet_from.0 >= u64::from(out.hz()) / 20);
        }
    }
    let tail = release_samples(v.cfg.tail, out);
    let latency = u64::from(v.prepared.latency);
    let trigs = t.triggers();
    for (i, g) in trigs.iter().enumerate() {
        prop_assert!(i == 0 || trigs[i - 1].at <= g.at, "triggers are sorted");
        let k = t.segment_at(g.at);
        let seg = segs[k];
        prop_assert!(
            g.at < seg.quiet_from,
            "trigger {} at {} is in a quiet part: {:?}",
            i,
            g.at.0,
            segs
        );
        let end = event_end(t, i);
        prop_assert!(end < u64::MAX, "every NoteOn is closed");
        let sound_end = end + tail + latency;
        prop_assert!(
            sound_end <= seg.quiet_from.0,
            "trigger {} ({:?}) sounds to {} past quiet_from {}",
            i,
            g.kind,
            sound_end,
            seg.quiet_from.0
        );
        prop_assert_eq!(
            t.segment_at(RenderPos(sound_end.max(g.at.0 + 1) - 1)),
            k,
            "a reset inside an event"
        );
        if let Some(id) = g.kind.slice() {
            let n = t.slice(id).len() as u64;
            prop_assert!(n > 0, "empty slices are not handed out");
            prop_assert!(g.at.0 + n <= end + 2, "a slice outlives its event");
            prop_assert!(
                g.at.0 + n <= seg.quiet_from.0,
                "a slice ends outside its activity"
            );
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// Starts strictly increase from 0; quiet parts last at least 50 ms; no
    /// trigger falls in a quiet part; every trigger span plus tail and latency
    /// ends before its quiet part; every slice ends inside its event's
    /// activity.
    #[test]
    fn segment_plan_is_sound(case in render_case()) {
        for v in case.bind() {
            check_plan(&v)?;
            // The plan is the frame plan of the voice's activity, rate-free.
            let act = activity_frames(&case.track, v.cfg.phrase, v.bound.role_mask());
            let ext = u64::from(v.cfg.tail)
                + takedown_core::time::frames_covering(u64::from(v.cfg.latency), case.out_rate);
            let with_tail: Vec<(u64, u64)> = act.iter().map(|&(a, b)| (a, b + ext)).collect();
            let plan = segment_plan(&with_tail);
            let frames: Vec<u64> = v.bound.segments().iter().map(|s| s.frame).collect();
            prop_assert_eq!(plan.iter().map(|p| p.0).collect::<Vec<_>>(), frames);
            for &(a, b) in &with_tail {
                prop_assert!(plan.iter().any(|&(r, q)| r <= a && b <= q), "interval ({}, {}) split", a, b);
            }
            prop_assert!(act.iter().all(|&(a, b)| a < b));
        }
    }

    /// Compensated output equals raw output shifted by L.
    #[test]
    fn latency_compensation_is_a_pure_shift((case, latency) in (render_case(), 1u32..=199)) {
        for (i, cfg) in case.voices.iter().enumerate() {
            let mut cfg = cfg.clone();
            cfg.latency = latency;
            let v = ToyVoice::bind(&cfg, &case.track, case.out_rate, &case.tree, ["toy_a", "toy_b"][i], case.max_block);
            check_plan(&v)?;
            let len = v.render_len();
            let compensated = v.mono(len);
            let raw = render_mono(&mut v.backend(), Prepared { latency: 0 }, &v.bound, v.max_block, len + u64::from(latency));
            prop_assert_eq!(first_bit_difference(&compensated, &raw[latency as usize..]), None);
        }
    }

    /// Random variants of J1/J3: an event with a declared release, then
    /// another event after a gap shorter or longer than the release.
    #[test]
    fn release_survives_to_declared_tail(
        (timeline, out) in rate_pair(),
        tail in 1u32..=40,
        gap_frames in 1u64..=80,
        latency in select(vec![0u32, 1, 7, 64]),
        cell in select(vec![0u32, 7, 32]),
        phrase in any::<bool>(),
    ) {
        check_release(timeline, out, tail, gap_frames, latency, cell, phrase)?;
    }

    /// The honest toy stays under 1e-3 in every quiet part; the one whose
    /// release outlasts its declared tail does not.
    #[test]
    fn tail_overrun_flags_dishonest_tails(
        (timeline, out) in rate_pair(),
        tail in select(vec![0u32, 4, 30]),
        latency in select(vec![0u32, 7, 64]),
        cell in select(vec![0u32, 7, 32]),
        phrase in any::<bool>(),
        max_block in select(vec![64usize, 4096]),
    ) {
        let r = u64::from(timeline.hz());
        let track = Arc::new(fixed_track(
            timeline,
            3 * r,
            &["kick", "snare"],
            vec![gate_span(r / 2, r, Link::Detached)],
            vec![
                hit_event(r / 10, r / 10, r / 20, 0, 127),
                hit_event(3 * r / 2, 3 * r / 2, r / 5, 1, 120),
                hit_event(2 * r, 2 * r, (r / 100).max(1), 0, 100),
            ],
            true,
        ));
        let tree = SeedTree::new(Seed(99));
        let mut honest = ToyConfig::hits(&["kick", "snare"]);
        honest.phrase = phrase;
        honest.tail = tail;
        honest.release = tail;
        honest.latency = latency;
        honest.cell = cell;
        honest.slice_max_ms = 30;
        let dishonest = honest.clone().dishonest();
        prop_assert!(honest.honest() && !dishonest.honest());
        let h = ToyVoice::bind(&honest, &track, out, &tree, "audit", max_block).tail_overrun();
        let d = ToyVoice::bind(&dishonest, &track, out, &tree, "audit", max_block).tail_overrun();
        prop_assert!(h < 1e-3, "honest overrun {}", h);
        prop_assert!(d > 1e-3, "dishonest overrun {}", d);
    }
}

/// An event ending at 0.5 s with a declared release of `tail` frames, then a
/// second event `gap_frames` later. No reset falls inside the release, and the
/// release sounds exactly as it does without the second event.
fn check_release(
    timeline: SampleRate,
    out: SampleRate,
    tail: u32,
    gap_frames: u64,
    latency: u32,
    cell: u32,
    phrase: bool,
) -> Result<(), TestCaseError> {
    let r = u64::from(timeline.hz());
    let (on, off) = (r / 10, r / 2);
    let next = off + (gap_frames * r / 200).max(1);
    let build = |second: bool| {
        let (spans, hits) = if phrase {
            let mut spans = vec![gate_span(on, off, Link::Detached)];
            if second {
                spans.push(gate_span(next, next + r / 10, Link::Detached));
            }
            (spans, vec![])
        } else {
            let mut hits = vec![hit_event(on, on, off - on, 0, 110)];
            if second {
                hits.push(hit_event(next, next, r / 10, 0, 110));
            }
            (vec![], hits)
        };
        Arc::new(fixed_track(timeline, 2 * r, &["kick"], spans, hits, false))
    };
    let mut cfg = ToyConfig::hits(&["kick"]);
    cfg.phrase = phrase;
    cfg.tail = tail;
    cfg.release = tail;
    cfg.latency = latency;
    cfg.cell = cell;
    let tree = SeedTree::new(Seed(5));
    let both = ToyVoice::bind(&cfg, &build(true), out, &tree, "release", 512);
    let alone = ToyVoice::bind(&cfg, &build(false), out, &tree, "release", 512);
    check_plan(&both)?;

    let t = &both.bound;
    // The phrase's first trigger is its NoteOn; the release follows the NoteOff.
    let first = usize::from(phrase);
    let end = event_end(t, first);
    let release = release_samples(tail, out);
    let lat = u64::from(latency);
    for s in t.segments() {
        prop_assert!(
            !(s.start.0 > end && s.start.0 <= end + release + lat),
            "a reset at {} inside the release [{}, {}]",
            s.start.0,
            end,
            end + release + lat
        );
    }
    let second_at = t.triggers()[first + 1].at.0;
    let len = both.render_len().max(alone.render_len());
    let a = both.mono(len);
    let b = alone.mono(len);
    let stop = second_at.min(end + release) as usize;
    for n in end as usize..stop {
        // Equal values: quiet stretches may differ in the sign of zero.
        #[allow(clippy::float_cmp)]
        let same = a[n] == b[n];
        prop_assert!(same, "release cut at {}: {} vs {}", n, a[n], b[n]);
    }
    if release >= 8 {
        prop_assert!(
            b[end as usize..(end + release / 2) as usize]
                .iter()
                .any(|x| *x != 0.0),
            "the release is silent"
        );
    }
    Ok(())
}

/// J1/J3: a 100 ms release, then a 60 ms gap. The second event stays in the
/// release's segment, so nothing cuts it.
#[test]
fn release_survives_j1_j3() {
    let r = rate(48_000);
    for phrase in [true, false] {
        check_release(r, r, 20, 12, 0, 0, phrase).expect("J1/J3 release");
        check_release(r, r, 20, 12, 64, 32, phrase).expect("J1/J3 release with latency");
    }
}

/// J1's case: a backend with 64 samples of latency; the hit sounds at sample
/// 10000, not 10064, for all 480 samples.
#[test]
fn latency_j1_hit_sounds_on_time() {
    let r = rate(48_000);
    let track = Arc::new(fixed_track(
        r,
        48_000,
        &["kick"],
        vec![],
        vec![hit_event(10_000, 10_000, 480, 0, 120)],
        false,
    ));
    let tree = SeedTree::new(Seed(7));
    let plain = ToyConfig::hits(&["kick"]);
    let mut late = plain.clone();
    late.latency = 64;
    let v0 = ToyVoice::bind(&plain, &track, r, &tree, "kick", 256);
    let v64 = ToyVoice::bind(&late, &track, r, &tree, "kick", 256);
    let len = 12_000;
    let a = v0.mono(len);
    let b = v64.mono(len);
    assert_eq!(
        first_bit_difference(&a[10_000..10_480], &b[10_000..10_480]),
        None
    );
    assert!(
        b[..10_000].iter().all(|x| *x == 0.0),
        "sound before the hit"
    );
    assert!(b[10_480..].iter().all(|x| *x == 0.0), "sound after the hit");
    for w in b[10_000..10_480].chunks(16) {
        assert!(
            w.iter().any(|x| *x != 0.0),
            "a silent stretch inside the hit"
        );
    }
    // The plan is the same frame plan plus one frame of latency.
    assert_eq!(v0.bound.segments().len(), 2);
    assert_eq!(v64.bound.segments().len(), 2);
    assert_eq!(
        control_index(v64.bound.end().0, r).0,
        control_index(v0.bound.end().0, r).0 + 1
    );
}
