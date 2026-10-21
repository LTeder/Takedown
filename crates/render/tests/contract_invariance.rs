//! core-types §3.6: ★ `block_size_invariance`, `event_sequence_invariance`
//! and ★ `chunked_equals_monolithic`, plus the sparse ten-minute case.

use std::sync::Arc;

use proptest::prelude::*;
use takedown_core::control::Link;
use takedown_core::seed::VoiceSeeds;
use takedown_core::seed::{Seed, SeedTree};
use takedown_core::time::{RenderPos, frame_start, rescale_ceil};
use takedown_render::contract::testkit::{
    FIXED_BLOCKS, RenderCase, ToyBackend, ToyConfig, ToyVoice, blocks, first_bit_difference,
    fixed_track, gate_span, hit_event, rate, render_case, render_cases,
};
use takedown_render::contract::{
    BoundTrack, BoundTrigger, Driver, DriverEvent, EventCursor, Prepared, RenderBackend,
};

/// A render case with two block sequences sized to its `max_block`.
fn case_and_blocks() -> impl Strategy<Value = (RenderCase, Vec<usize>, Vec<usize>)> {
    render_case().prop_flat_map(|c| {
        let m = c.max_block;
        (Just(c), blocks(m), blocks(m))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// The monolithic render and every segmentation are bit-identical.
    #[test]
    fn block_size_invariance((case, a, b) in case_and_blocks()) {
        for v in case.bind() {
            let len = v.render_len();
            let mono = v.mono(len);
            for seq in [&a, &b, &FIXED_BLOCKS.to_vec()] {
                let out = v.blocks(len, seq);
                prop_assert_eq!(
                    first_bit_difference(&mono, &out), None,
                    "blocks {:?}, segments {:?}", seq, v.bound.segments()
                );
            }
        }
    }

    /// A fresh backend started at every segment start, fed random blocks,
    /// reproduces the monolithic suffix bit for bit.
    #[test]
    fn chunked_equals_monolithic((case, a, b) in case_and_blocks()) {
        for v in case.bind() {
            let len = v.render_len();
            let mono = v.mono(len);
            for (k, seg) in v.bound.segments().iter().enumerate() {
                let seq = if k % 2 == 0 { &a } else { &b };
                let out = v.from_segment(k, len, seq);
                let start = seg.start.0 as usize;
                prop_assert_eq!(
                    first_bit_difference(&mono[start..], &out), None,
                    "segment {} of {:?}", k, v.bound.segments()
                );
            }
        }
    }

    /// The driver's event sequence is the same for any block segmentation and
    /// any `max_run` once adjacent runs are merged, and it follows the order
    /// rules at every sample.
    #[test]
    fn event_sequence_invariance(
        (case, a, b) in case_and_blocks(),
        run_a in 1usize..=5_000,
        run_b in 1usize..=64,
    ) {
        for v in case.bind() {
            let t = &v.bound;
            let len = v.render_len();
            let cell = v.cfg.cell;
            let whole = cursor_events(t, cell, usize::MAX, &[usize::MAX], len);
            check_order(t, cell, usize::MAX, &whole, len)?;
            let merged = merge_runs(&whole);
            for (max_run, seq) in [(run_a, &a), (run_b, &b), (v.max_block, &FIXED_BLOCKS.to_vec())] {
                let evs = cursor_events(t, cell, max_run, seq, len);
                check_order(t, cell, max_run, &evs, len)?;
                prop_assert_eq!(&merge_runs(&evs), &merged, "max_run {}, blocks {:?}", max_run, seq);
            }
            // Through the driver, the backend sees the same sequence without
            // the silences, in backend time, including the latency pre-roll.
            let mut rec = Recorder { inner: v.backend(), log: Vec::new() };
            let mut d = Driver::new(&rec.info(), v.prepared, v.max_block);
            d.start(&mut rec, t, 0);
            let mut out = vec![0.0; len as usize];
            let mut o = 0;
            for &k in a.iter().cycle() {
                if o >= out.len() {
                    break;
                }
                let k = k.clamp(1, out.len() - o);
                d.process(&mut rec, t, &mut out[o..o + k]);
                o += k;
            }
            let backend_end = len + u64::from(v.prepared.latency);
            let expected: Vec<DriverEvent> = merge_runs(&cursor_events(t, cell, usize::MAX, &[usize::MAX], backend_end))
                .into_iter()
                .filter(|e| !matches!(e, DriverEvent::Silence { .. }))
                .collect();
            prop_assert_eq!(merge_runs(&rec.log), expected);
            prop_assert!(rec.log.iter().all(|e| match e {
                DriverEvent::Run { len, .. } => *len <= v.max_block,
                _ => true,
            }), "a run longer than max_block");
        }
    }
}

/// Logs every hook call as a driver event.
struct Recorder {
    inner: ToyBackend,
    log: Vec<DriverEvent>,
}

impl RenderBackend for Recorder {
    fn info(&self) -> takedown_render::contract::BackendInfo {
        self.inner.info()
    }

    fn prepare(&mut self, rate: takedown_core::time::SampleRate, max_block: usize) -> Prepared {
        self.inner.prepare(rate, max_block)
    }

    fn reset(&mut self, track: &BoundTrack, seeds: VoiceSeeds, at: RenderPos) {
        self.log.push(DriverEvent::Reset { at });
        self.inner.reset(track, seeds, at);
    }

    fn cell(&mut self, track: &BoundTrack, at: RenderPos) {
        self.log.push(DriverEvent::Cell { at });
        self.inner.cell(track, at);
    }

    fn trigger(&mut self, track: &BoundTrack, trig: &BoundTrigger) {
        let index = track
            .triggers()
            .iter()
            .position(|t| std::ptr::eq(t, trig))
            .expect("the driver hands out the track's own triggers");
        self.log.push(DriverEvent::Trigger { index });
        self.inner.trigger(track, trig);
    }

    fn run(&mut self, track: &BoundTrack, start: RenderPos, out: &mut [f32]) {
        self.log.push(DriverEvent::Run {
            start,
            len: out.len(),
        });
        self.inner.run(track, start, out);
    }
}

/// Every event of `[0, len)` in backend time, with blocks cycled.
fn cursor_events(
    t: &BoundTrack,
    cell: u32,
    max_run: usize,
    blocks: &[usize],
    len: u64,
) -> Vec<DriverEvent> {
    let mut c = EventCursor::new(t, RenderPos(0), cell, max_run);
    let mut out = Vec::new();
    let mut pos = 0u64;
    for &k in blocks.iter().cycle() {
        if pos >= len {
            break;
        }
        let end = pos.saturating_add(k.max(1) as u64).min(len);
        while let Some(e) = c.next_event(t, RenderPos(end)) {
            out.push(e);
        }
        pos = end;
    }
    out
}

/// Adjacent runs (and adjacent silences) merged into one.
fn merge_runs(evs: &[DriverEvent]) -> Vec<DriverEvent> {
    let mut out: Vec<DriverEvent> = Vec::with_capacity(evs.len());
    for &e in evs {
        match (out.last_mut(), e) {
            (Some(DriverEvent::Run { start, len }), DriverEvent::Run { start: s2, len: l2 })
            | (
                Some(DriverEvent::Silence { start, len }),
                DriverEvent::Silence { start: s2, len: l2 },
            ) if start.0 + *len as u64 == s2.0 => *len += l2,
            _ => out.push(e),
        }
    }
    out
}

/// The order rules of §2.15 hold for an unmerged event list over `[0, len)`.
fn check_order(
    t: &BoundTrack,
    cell: u32,
    max_run: usize,
    evs: &[DriverEvent],
    len: u64,
) -> Result<(), TestCaseError> {
    let segs = t.segments();
    let mut pos = 0u64;
    // 0: nothing yet at pos; 1: reset; 2: cell; 3: triggers.
    let mut stage = 0;
    let mut next_trigger = 0usize;
    let mut resets = Vec::new();
    for e in evs {
        match *e {
            DriverEvent::Reset { at } => {
                prop_assert!(at.0 == pos && stage == 0, "reset out of order at {}", at.0);
                resets.push(at);
                stage = 1;
            }
            DriverEvent::Cell { at } => {
                prop_assert!(at.0 == pos && stage <= 1, "cell out of order at {}", at.0);
                prop_assert!(
                    cell > 1 && at.0 % u64::from(cell) == 0,
                    "cell off the grid at {}",
                    at.0
                );
                let s = &segs[t.segment_at(at)];
                prop_assert!(at < s.quiet_from, "cell in a quiet part at {}", at.0);
                stage = 2;
            }
            DriverEvent::Trigger { index } => {
                prop_assert_eq!(index, next_trigger, "triggers skipped or repeated");
                prop_assert_eq!(t.triggers()[index].at.0, pos);
                next_trigger += 1;
                stage = 3;
            }
            DriverEvent::Run { start, len: n } | DriverEvent::Silence { start, len: n } => {
                prop_assert_eq!(start.0, pos, "runs are not contiguous");
                prop_assert!(
                    n >= 1 && n <= max_run,
                    "run of {} with max_run {}",
                    n,
                    max_run
                );
                let s = &segs[t.segment_at(start)];
                let quiet = start >= s.quiet_from;
                prop_assert_eq!(matches!(e, DriverEvent::Silence { .. }), quiet);
                let stop = start.0 + n as u64;
                if !quiet {
                    prop_assert!(stop <= s.quiet_from.0, "a run crosses quiet_from");
                }
                // No event strictly inside the run.
                prop_assert!(
                    segs.iter()
                        .all(|g| g.start.0 <= start.0 || g.start.0 >= stop)
                );
                prop_assert!(
                    t.triggers()
                        .iter()
                        .all(|g| g.at.0 <= start.0 || g.at.0 >= stop)
                );
                if cell > 1 && !quiet {
                    prop_assert!(stop - start.0 <= u64::from(cell));
                    prop_assert!((start.0 + 1..stop).all(|p| p % u64::from(cell) != 0));
                }
                pos = stop;
                stage = 0;
            }
        }
    }
    prop_assert_eq!(pos, len, "events do not cover the render");
    let starts: Vec<RenderPos> = segs.iter().map(|s| s.start).filter(|s| s.0 < len).collect();
    prop_assert_eq!(resets, starts, "resets are not exactly the segment starts");
    let due = t.triggers().iter().filter(|g| g.at.0 < len).count();
    prop_assert_eq!(next_trigger, due, "triggers missing");
    Ok(())
}

/// Ten minutes at 44.1 kHz rendered at 48 kHz: a few short phrases and hits,
/// a hit ending at the very end, and minutes of quiet in between. Compared
/// block by block against the monolithic render, so only one full buffer is
/// held.
#[test]
fn block_size_invariance_sparse_ten_minutes() {
    let timeline = rate(44_100);
    let out_rate = rate(48_000);
    let len = 600 * 44_100;
    let s = |secs: f64| (secs * 44_100.0) as u64;
    let hits = vec![
        hit_event(s(10.0), s(10.0) - 300, s(0.05), 0, 110),
        hit_event(s(10.02), s(10.0) - 300, s(0.4), 1, 90),
        hit_event(s(95.5), s(95.5), s(2.0), 1, 127),
        hit_event(s(200.25), s(200.2), s(0.3), 0, 60),
        hit_event(s(333.0), s(333.0), s(60.0), 2, 40),
        hit_event(s(480.0), s(480.01), 1, 0, 100),
        hit_event(len - s(0.03), len - s(0.03), s(0.03), 2, 100),
    ];
    let spans = vec![
        gate_span(s(30.0), s(30.5), Link::Detached),
        gate_span(s(30.5), s(30.8), Link::Retrigger),
        gate_span(s(30.8), s(31.0), Link::Legato),
        gate_span(s(400.0), s(402.0), Link::Detached),
        gate_span(s(402.05), s(402.1), Link::Detached),
    ];
    let track = Arc::new(fixed_track(
        timeline,
        len,
        &["kick", "snare", "zap"],
        spans,
        hits,
        true,
    ));
    let mut tree = SeedTree::new(Seed(0x1234_5678_9abc_def0));
    tree.roll_region(
        takedown_core::seed::Aspect::Body,
        takedown_core::time::TimelineSpan::new(
            takedown_core::time::TimelinePos(s(95.0)),
            takedown_core::time::TimelinePos(s(96.0)),
        )
        .expect("ordered span"),
    );
    let mut phrase = ToyConfig::hits(&["kick", "zap"]);
    phrase.phrase = true;
    phrase.latency = 64;
    phrase.cell = 32;
    phrase.tail = 30;
    phrase.release = 30;
    phrase.slice_max_ms = 30;
    let mut hitter = ToyConfig::hits(&["snare", "zap"]);
    hitter.latency = 7;
    hitter.cell = 7;
    hitter.tail = 4;
    hitter.release = 4;
    hitter.slice_max_ms = u32::MAX;
    for (cfg, name) in [(phrase, "ten_a"), (hitter, "ten_b")] {
        let v = ToyVoice::bind(&cfg, &track, out_rate, &tree, name, 1024);
        let full = rescale_ceil(len, timeline, out_rate).0;
        assert!(v.bound.end().0 <= full + frame_start(40, out_rate));
        assert!(v.bound.segments().len() >= 5, "{:?}", v.bound.segments());
        let mono = v.mono(full);
        assert!(mono.iter().any(|x| *x != 0.0), "the sparse case is silent");
        for seq in [&FIXED_BLOCKS[..], &[4093, 1, 8192, 333][..]] {
            assert_eq!(
                streamed_difference(&v, 0, &mono, seq),
                None,
                "blocks {seq:?}"
            );
        }
        for k in 1..v.bound.segments().len() {
            assert_eq!(
                streamed_difference(&v, k, &mono, &[2_000, 77]),
                None,
                "segment {k}"
            );
        }
    }
}

/// Renders from segment `k` to the end of `mono` in cycled blocks and returns
/// the first output position that differs from `mono`.
fn streamed_difference(v: &ToyVoice, k: usize, mono: &[f32], seq: &[usize]) -> Option<usize> {
    let mut b = v.backend();
    let mut d = Driver::new(&b.info(), v.prepared, v.max_block);
    d.start(&mut b, &v.bound, k);
    let mut pos = d.position().expect("started").0 as usize;
    let mut buf = vec![0.0f32; seq.iter().copied().max().unwrap_or(1)];
    for &n in seq.iter().cycle() {
        if pos >= mono.len() {
            return None;
        }
        let n = n.min(mono.len() - pos);
        d.process(&mut b, &v.bound, &mut buf[..n]);
        if let Some(i) = first_bit_difference(&mono[pos..pos + n], &buf[..n]) {
            return Some(pos + i);
        }
        pos += n;
    }
    None
}
