//! core-types §3.6: `region_reroll_locality`,
//! `inserting_a_hit_keeps_other_seeds`, `moving_a_trigger_shifts_its_output`
//! and `hit_voices_never_read_curves`, plus
//! `retiming_a_layered_sibling_keeps_the_voice`: a hit's seeds count only the
//! hits of its own role at its anchor.

use std::sync::Arc;

use proptest::prelude::*;
use proptest::sample::select;
use takedown_core::control::{ControlTrack, HitEvent, MAX_HIT_MS};
use takedown_core::seed::{Aspect, Seed, SeedTree};
use takedown_core::time::{RenderPos, SampleRate, TimelinePos, TimelineSpan};
use takedown_render::contract::testkit::{
    RenderCase, ToyConfig, ToyVoice, VOICE_NAMES, aspect, curves, first_bit_difference,
    fixed_track, hit_event, rate, render_case, render_cases, std_rate,
};
use takedown_render::contract::{BoundKind, BoundTrack, BoundTrigger};

fn is_hit(t: &BoundTrigger) -> bool {
    matches!(t.kind, BoundKind::Hit { .. })
}

/// Every anchor in the track.
fn anchors(t: &ControlTrack) -> Vec<u64> {
    t.hits()
        .iter()
        .map(|h| h.anchor.0)
        .chain(t.spans().iter().map(|s| s.anchor.0))
        .collect()
}

fn span(a: u64, b: u64) -> TimelineSpan {
    TimelineSpan::new(TimelinePos(a.min(b)), TimelinePos(a.max(b))).expect("ordered")
}

/// A render case, an aspect (mostly the two a backend sees) and a region:
/// around an existing anchor, or anywhere.
fn case_and_region() -> impl Strategy<Value = (RenderCase, Aspect, TimelineSpan)> {
    render_case().prop_flat_map(|c| {
        let len = c.track.len();
        let found = anchors(&c.track);
        let around = if found.is_empty() {
            Just(0u64).boxed()
        } else {
            select(found).boxed()
        };
        let region = prop_oneof![
            2 => (around, 0..=len / 4 + 1, 1..=len / 4 + 2)
                .prop_map(|(a, before, after)| span(a.saturating_sub(before), a + after)),
            1 => (0..=len, 0..=len).prop_map(|(a, b)| span(a, b)),
        ];
        let which = prop_oneof![
            3 => Just(Aspect::Body),
            3 => Just(Aspect::Mangle),
            1 => aspect(),
        ];
        (Just(c), which, region)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// Output is bit-identical before the first trigger anchored in the
    /// region and in every segment that contains no such trigger. Rolling
    /// Mangle or a region changes the output of a voice that uses those seeds.
    #[test]
    fn region_reroll_locality((case, which, region) in case_and_region()) {
        let mut rolled = case.tree.clone();
        rolled.roll_region(which, region);
        let changed = rolled != case.tree;
        let other = RenderCase { tree: rolled, ..case.clone() };
        let seeds_aspect = matches!(which, Aspect::Body | Aspect::Mangle);
        for (i, (v, w)) in case.bind().iter().zip(other.bind().iter()).enumerate() {
            prop_assert_eq!(v.bound.voice_seeds(), w.bound.voice_seeds(), "voice seeds saw a region roll");
            prop_assert_eq!(v.bound.segments(), w.bound.segments());
            let mut affected: Vec<BoundTrigger> = Vec::new();
            for (x, y) in v.bound.triggers().iter().zip(w.bound.triggers()) {
                prop_assert_eq!((x.at, x.anchor), (y.at, y.anchor));
                if changed && seeds_aspect && region.contains(x.anchor) {
                    prop_assert_ne!(x.seeds, y.seeds, "an anchor inside the region kept its seeds");
                    affected.push(*x);
                } else {
                    prop_assert_eq!(x.seeds, y.seeds, "an anchor outside the region changed seeds");
                }
            }
            let len = v.render_len();
            let a = v.mono(len);
            let b = w.mono(len);
            let first = affected.iter().map(|t| t.at.0).min().unwrap_or(u64::MAX).min(len) as usize;
            prop_assert_eq!(first_bit_difference(&a[..first], &b[..first]), None, "changed before the region's first trigger");
            let segs = v.bound.segments();
            for (k, s) in segs.iter().enumerate() {
                let start = s.start.0.min(len);
                let end = segs.get(k + 1).map_or(len, |n| n.start.0).min(len);
                if affected.iter().any(|t| t.at.0 >= s.start.0 && t.at.0 < end) {
                    continue;
                }
                prop_assert_eq!(
                    first_bit_difference(&a[start as usize..end as usize], &b[start as usize..end as usize]),
                    None,
                    "segment {} has no trigger in the region but changed", k
                );
            }
            if affected.iter().any(is_hit) {
                prop_assert!(first_bit_difference(&a, &b).is_some(), "a region roll over a hit is inaudible");
            }
            let mut mangled = case.tree.clone();
            if mangled.roll(Aspect::Mangle) && v.bound.triggers().iter().any(is_hit) {
                let m = ToyVoice::bind(&v.cfg, &case.track, case.out_rate, &mangled, VOICE_NAMES[i], case.max_block);
                prop_assert!(first_bit_difference(&a, &m.mono(len)).is_some(), "a Mangle roll is inaudible");
            }
        }
    }
}

/// The triggers of `with` other than the one anchored at `new` (a hit), which
/// must match `without` exactly: time, lead, anchor, seeds, kind and slice
/// contents.
fn check_kept_seeds(
    without: &BoundTrack,
    with: &BoundTrack,
    new: TimelinePos,
) -> Result<Option<usize>, TestCaseError> {
    let fresh: Vec<usize> = with
        .triggers()
        .iter()
        .enumerate()
        .filter(|(_, g)| g.anchor == new && is_hit(g))
        .map(|(i, _)| i)
        .collect();
    prop_assert!(fresh.len() <= 1);
    let kept: Vec<&BoundTrigger> = with
        .triggers()
        .iter()
        .enumerate()
        .filter(|(i, _)| !fresh.contains(i))
        .map(|(_, g)| g)
        .collect();
    prop_assert_eq!(kept.len(), without.triggers().len());
    for (x, y) in without.triggers().iter().zip(kept) {
        prop_assert_eq!(
            (x.at, x.lead.to_bits(), x.anchor),
            (y.at, y.lead.to_bits(), y.anchor)
        );
        prop_assert_eq!(x.seeds, y.seeds, "an existing trigger changed seeds");
        let strip = |k: BoundKind| match k {
            BoundKind::NoteOn { vel, .. } => BoundKind::NoteOn { vel, slice: None },
            BoundKind::Retrigger { vel, .. } => BoundKind::Retrigger { vel, slice: None },
            BoundKind::Hit {
                role,
                vel,
                bright,
                breath,
                mouth,
                dur,
                ..
            } => BoundKind::Hit {
                role,
                vel,
                bright,
                breath,
                mouth,
                dur,
                slice: None,
            },
            BoundKind::NoteOff => BoundKind::NoteOff,
        };
        prop_assert_eq!(strip(x.kind), strip(y.kind));
        let slice =
            |t: &BoundTrack, g: &BoundTrigger| g.kind.slice().map(|id| t.slice(id).to_vec());
        prop_assert_eq!(slice(without, x), slice(with, y));
    }
    Ok(fresh.first().copied())
}

/// Output changes only from the new hit's segment start to the first reset
/// point both plans share.
fn check_insert_output(
    v: &ToyVoice,
    w: &ToyVoice,
    fresh: Option<usize>,
) -> Result<(), TestCaseError> {
    let len = v.render_len().max(w.render_len());
    let a = v.mono(len);
    let b = w.mono(len);
    let Some(i) = fresh else {
        prop_assert_eq!(
            first_bit_difference(&a, &b),
            None,
            "an unrouted hit changed the voice"
        );
        return Ok(());
    };
    let at = w.bound.triggers()[i].at;
    let from = w.bound.reset_point_before(at).0 as usize;
    prop_assert_eq!(
        first_bit_difference(&a[..from], &b[..from]),
        None,
        "changed before the new hit's segment"
    );
    let shared = w
        .bound
        .segments()
        .iter()
        .map(|s| s.start)
        .filter(|s| *s > at && v.bound.segments().iter().any(|x| x.start == *s))
        .min();
    if let Some(r) = shared {
        let r = r.0 as usize;
        prop_assert_eq!(
            first_bit_difference(&a[r..], &b[r..]),
            None,
            "changed after the first shared reset {}",
            r
        );
    }
    Ok(())
}

/// A render case and a hit to insert: (at, dur, anchor offset, role choice).
fn case_and_insert() -> impl Strategy<Value = (RenderCase, u64, u64, i64, usize)> {
    render_case().prop_flat_map(|c| {
        let len = c.track.len();
        (
            Just(c),
            0..len,
            1u64..=u64::MAX,
            -1_000i64..=1_000,
            any::<usize>(),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// Existing triggers keep their seeds; output changes only from the new
    /// hit's segment start to the first reset point both plans share.
    #[test]
    fn inserting_a_hit_keeps_other_seeds((case, at, dur, jitter, pick) in case_and_insert()) {
        let t = &case.track;
        // No assumptions (rejections), so a 5,000-case soak never aborts: a
        // track without roles has no hit to insert.
        if t.roles().is_empty() {
            return Ok(());
        }
        // Prefer a role the first voice takes.
        let routed: Vec<usize> = (0..t.roles().len()).filter(|&r| case.voices[0].roles.contains(&t.roles()[r])).collect();
        let role = if routed.is_empty() { pick % t.roles().len() } else { routed[pick % routed.len()] };
        let cap = t.rate().samples_for_ms(MAX_HIT_MS);
        let dur = 1 + dur % cap.min(t.len() - at);
        // The nearest anchor at or after the jittered one that no hit uses.
        let used: std::collections::BTreeSet<u64> = t.hits().iter().map(|h| h.anchor.0).collect();
        let wanted = at.saturating_add_signed(jitter).min(t.len());
        let Some(anchor) = (wanted..=t.len()).chain(0..wanted).find(|a| !used.contains(a)) else {
            return Ok(());
        };
        let mut parts = (**t).clone().into_parts();
        let mut hit = hit_event(at, anchor, dur, role as u8, 1 + (pick % 127) as u8);
        hit.breath = takedown_core::control::U7::new((pick % 128) as u8).expect("7-bit");
        parts.hits.push(hit);
        let inserted = RenderCase { track: Arc::new(ControlTrack::new(parts).expect("still valid")), ..case.clone() };
        for (v, w) in case.bind().iter().zip(inserted.bind().iter()) {
            let fresh = check_kept_seeds(&v.bound, &w.bound, TimelinePos(anchor))?;
            check_insert_output(v, w, fresh)?;
        }
    }
}

/// J1's case on A: two hits in separate segments, then a hit inserted between
/// them that merges all three into one segment. The later hit keeps its
/// seeds, and nothing before the merged segment changes.
#[test]
fn inserting_a_hit_that_merges_segments() {
    let r = rate(48_000);
    let ms = |m: u64| m * 48;
    let tree = SeedTree::new(Seed(0xa11c_e5ee_d000_0001));
    let lead_in = hit_event(ms(100), ms(100), ms(30), 0, 90);
    let a = hit_event(ms(200), ms(200), ms(50), 0, 100);
    let b = hit_event(ms(320), ms(321), ms(50), 1, 100);
    let c = hit_event(ms(270), ms(268), ms(40), 0, 100);
    let track = |hits: Vec<HitEvent>| {
        Arc::new(fixed_track(
            r,
            ms(1_000),
            &["kick", "snare"],
            vec![],
            hits,
            true,
        ))
    };
    let before = track(vec![lead_in, a, b]);
    let after = track(vec![lead_in, a, b, c]);
    let mut cfg = ToyConfig::hits(&["kick", "snare"]);
    cfg.slice_max_ms = 30;
    let v = ToyVoice::bind(&cfg, &before, r, &tree, "kit", 256);
    let w = ToyVoice::bind(&cfg, &after, r, &tree, "kit", 256);
    assert_eq!(v.bound.segments().len(), 4, "{:?}", v.bound.segments());
    assert_eq!(
        w.bound.segments().len(),
        3,
        "the insertion merges: {:?}",
        w.bound.segments()
    );
    let fresh = check_kept_seeds(&v.bound, &w.bound, TimelinePos(ms(268))).expect("seeds kept");
    assert!(fresh.is_some());
    check_insert_output(&v, &w, fresh).expect("output local to the merged segment");
}

/// One isolated hit moved by `d` with the same anchor: in its segment the
/// output is the original shifted by `d`; everything else is unchanged.
fn check_move(
    r: SampleRate,
    cfg: &ToyConfig,
    at: u64,
    dur: u64,
    d: i64,
    take: bool,
) -> Result<(), TestCaseError> {
    let hz = u64::from(r.hz());
    let len = 3 * hz;
    let far = |at| hit_event(at, at, (hz / 100).max(1), 0, 90);
    let original = Arc::new(fixed_track(
        r,
        len,
        &["kick"],
        vec![],
        vec![far(0), hit_event(at, at, dur, 0, 127), far(5 * hz / 2)],
        take,
    ));
    let moved_at = at.checked_add_signed(d).expect("in range");
    let mut moved_parts = (*original).clone().into_parts();
    for h in &mut moved_parts.hits {
        if h.at.0 == at {
            h.at = TimelinePos(moved_at);
        }
    }
    let moved = Arc::new(ControlTrack::new(moved_parts).expect("valid"));
    let tree = SeedTree::new(Seed(0x5eed));
    let v = ToyVoice::bind(cfg, &original, r, &tree, "mover", 333);
    let w = ToyVoice::bind(cfg, &moved, r, &tree, "mover", 333);
    let a = v.mono(len);
    let b = w.mono(len);
    let (k, k2) = (
        v.bound.segment_at(RenderPos(at)),
        w.bound.segment_at(RenderPos(moved_at)),
    );
    let seg = |t: &BoundTrack, k: usize| {
        (
            t.segments()[k].start.0,
            t.segments().get(k + 1).map_or(len, |s| s.start.0),
        )
    };
    let (s, e) = seg(&v.bound, k);
    let (s2, e2) = seg(&w.bound, k2);
    let release = u64::from(cfg.release) * hz / 200;
    prop_assert!(
        s <= at && at + dur + release <= e,
        "the hit is not isolated"
    );
    prop_assert!(s2 <= moved_at && moved_at + dur + release <= e2);
    let mut compared = 0;
    for n in s..e {
        let m = n.wrapping_add_signed(d);
        if m >= s2 && m < e2 {
            prop_assert_eq!(
                a[n as usize].to_bits(),
                b[m as usize].to_bits(),
                "sample {} moved to {}",
                n,
                m
            );
            compared += 1;
        }
    }
    prop_assert!(compared >= dur);
    let lo = s.min(s2) as usize;
    let hi = e.max(e2) as usize;
    prop_assert_eq!(first_bit_difference(&a[..lo], &b[..lo]), None);
    prop_assert_eq!(first_bit_difference(&a[hi..], &b[hi..]), None);
    prop_assert!(
        a[at as usize..(at + dur) as usize]
            .iter()
            .any(|x| *x != 0.0),
        "the hit is silent"
    );
    Ok(())
}

fn mover(latency: u32, tail: u32, slice_max_ms: u32) -> ToyConfig {
    let mut cfg = ToyConfig::hits(&["kick"]);
    cfg.voice_noise = false;
    cfg.latency = latency;
    cfg.tail = tail;
    cfg.release = tail;
    cfg.slice_max_ms = slice_max_ms;
    cfg
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// Hit-only toy with voice-level noise off, timeline rate equal to the
    /// output rate.
    #[test]
    fn moving_a_trigger_shifts_its_output(
        r in std_rate(),
        latency in select(vec![0u32, 1, 7, 64]),
        tail in select(vec![0u32, 4, 30]),
        slice_max in select(vec![0u32, 30, u32::MAX]),
        take in any::<bool>(),
        (at_frac, dur_frac, d_frac) in (0.0f64..1.0, 0.0f64..1.0, -1.0f64..1.0),
    ) {
        let hz = u64::from(r.hz());
        let at = hz / 2 + (at_frac * hz as f64) as u64;
        let dur = 1 + (dur_frac * (hz / 5) as f64) as u64;
        let max_d = (hz * 30 / 1000) as i64;
        let d = (d_frac * max_d as f64) as i64;
        let d = if d == 0 { 1 } else { d };
        check_move(r, &mover(latency, tail, slice_max), at, dur, d, take)?;
    }
}

/// J2's regression: a one-sample move had changed 2000 of 2000 samples.
#[test]
fn moving_a_hit_by_one_sample() {
    for (latency, tail) in [(0, 0), (64, 30)] {
        let cfg = mover(latency, tail, 30);
        check_move(rate(48_000), &cfg, 30_000, 2_000, 1, true).expect("shift by one");
        check_move(rate(48_000), &cfg, 30_000, 2_000, -1, true).expect("shift back by one");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// A hit-only voice has no cursor, and its output is unchanged when every
    /// curve is replaced.
    #[test]
    fn hit_voices_never_read_curves(
        (case, replacement) in render_case().prop_flat_map(|c| {
            let frames = c.track.frames();
            (Just(c), curves(frames))
        })
    ) {
        let mut parts = (*case.track).clone().into_parts();
        parts.curves = replacement;
        let other = Arc::new(ControlTrack::new(parts).expect("same shape, valid values"));
        for (i, cfg) in case.voices.iter().enumerate() {
            let mut cfg = cfg.clone();
            cfg.phrase = false;
            let v = ToyVoice::bind(&cfg, &case.track, case.out_rate, &case.tree, VOICE_NAMES[i], case.max_block);
            let w = ToyVoice::bind(&cfg, &other, case.out_rate, &case.tree, VOICE_NAMES[i], case.max_block);
            for p in [0, 1, v.bound.end().0 / 2, v.bound.end().0, u64::MAX / 4] {
                prop_assert!(v.bound.cursor(RenderPos(p)).is_none(), "a hit voice has a cursor");
            }
            prop_assert_eq!(v.bound.triggers(), w.bound.triggers());
            prop_assert_eq!(v.bound.segments(), w.bound.segments());
            let len = v.render_len();
            prop_assert_eq!(first_bit_difference(&v.mono(len), &w.mono(len)), None, "a hit voice read the curves");
        }
    }
}

/// Both voices' bound tracks agree in every trigger (seeds included),
/// segment, end and slice, and render bit-identically.
fn check_voice_unchanged(v: &ToyVoice, w: &ToyVoice) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        v.bound.triggers(),
        w.bound.triggers(),
        "a trigger or its seeds changed"
    );
    prop_assert_eq!(v.bound.segments(), w.bound.segments());
    prop_assert_eq!(v.bound.end(), w.bound.end());
    prop_assert_eq!(v.bound.slices(), w.bound.slices());
    let len = v.render_len().max(w.render_len());
    prop_assert_eq!(
        first_bit_difference(&v.mono(len), &w.mono(len)),
        None,
        "the voice's output changed"
    );
    Ok(())
}

/// A render case plus raw choices for [`retiming_a_layered_sibling_keeps_the_voice`].
fn case_and_sibling() -> impl Strategy<Value = (RenderCase, usize, u64, u64, u64, u64)> {
    (
        render_case(),
        any::<usize>(),
        any::<u64>(),
        1u64..=2_000,
        1u64..=2_000,
        any::<u64>(),
    )
}

/// A hit of `role` at `anchor`, with `dur` and `at` clamped so it fits the
/// track.
fn layered(len: u64, at: u64, anchor: u64, dur: u64, role: u8, vel: u8) -> HitEvent {
    let dur = dur.clamp(1, len);
    hit_event(at.min(len - dur), anchor, dur, role, vel)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// A voice routes role r1 but not r2. A hit of r2 layered on a hit of r1
    /// (same anchor) moves from just before the r1 hit to just after it, and
    /// changes its duration and velocity: the voice's triggers, seeds,
    /// slices and output stay bit-identical.
    #[test]
    fn retiming_a_layered_sibling_keeps_the_voice(
        (case, pick, raw_anchor, before_d, after_d, salt) in case_and_sibling()
    ) {
        let t = &case.track;
        let n = t.roles().len();
        // No rejections: a track with under two roles has no sibling to move.
        if n < 2 {
            return Ok(());
        }
        let len = t.len();
        let r1 = pick % n;
        let r2 = (r1 + 1 + (pick / n) % (n - 1)) % n;
        let mut cfg = case.voices[0].clone();
        cfg.roles.remove(&t.roles()[r2]);
        cfg.roles.insert(t.roles()[r1].clone());
        // An existing hit of r1 to layer on, or a new one.
        let existing: Vec<HitEvent> = t.hits().iter().filter(|h| usize::from(h.role) == r1).copied().collect();
        let mut parts = (**t).clone().into_parts();
        let base = if existing.is_empty() || salt % 3 == 0 {
            let anchor = raw_anchor % (len + 1);
            let h = layered(len, anchor, anchor, 1 + salt % 4_800, r1 as u8, 100);
            parts.hits.push(h);
            h
        } else {
            existing[(raw_anchor % existing.len() as u64) as usize]
        };
        let cap = t.rate().samples_for_ms(MAX_HIT_MS);
        let dur = |k: u64| 1 + (salt / 7 + k) % cap.min(len);
        let early = layered(len, base.at.0.saturating_sub(before_d), base.anchor.0, dur(0), r2 as u8, 90);
        let late = layered(len, base.at.0 + after_d, base.anchor.0, dur(1), r2 as u8, 30);
        let track_with = |sibling: HitEvent| {
            let mut p = parts.clone();
            p.hits.push(sibling);
            Arc::new(ControlTrack::new(p).expect("still valid"))
        };
        let (before, after) = (track_with(early), track_with(late));
        let v = ToyVoice::bind(&cfg, &before, case.out_rate, &case.tree, VOICE_NAMES[0], case.max_block);
        let w = ToyVoice::bind(&cfg, &after, case.out_rate, &case.tree, VOICE_NAMES[0], case.max_block);
        prop_assert!(v.bound.triggers().iter().any(|g| g.anchor == base.anchor && is_hit(g)));
        check_voice_unchanged(&v, &w)?;
    }
}

/// The review's repro at 48 kHz: a kick and a sub layered on the onset at
/// 4800; the sub's pre-roll moves it to 4700. A voice routing only "kick"
/// keeps its trigger seeds, its slices and its output, bit for bit.
#[test]
fn moving_a_layered_sub_keeps_the_kick_voice() {
    let r = rate(48_000);
    let track = |sub_at: u64| {
        Arc::new(fixed_track(
            r,
            48_000,
            &["kick", "sub"],
            vec![],
            vec![
                hit_event(4_800, 4_800, 4_800, 0, 110),
                hit_event(sub_at, 4_800, 9_600, 1, 100),
            ],
            true,
        ))
    };
    let tree = SeedTree::new(Seed(0x5eed_0f5a));
    let mut cfg = ToyConfig::hits(&["kick"]);
    cfg.slice_max_ms = 30;
    let v = ToyVoice::bind(&cfg, &track(4_800), r, &tree, "kit", 256);
    let w = ToyVoice::bind(&cfg, &track(4_700), r, &tree, "kit", 256);
    assert_eq!(v.bound.triggers().len(), 1);
    check_voice_unchanged(&v, &w).expect("the kick is untouched by the sub's move");
}
