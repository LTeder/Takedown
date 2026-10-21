//! core-types §3.4, the parts phase 1 implements: ControlTrack validation,
//! its JSON form, canonical events, and NoteView.

mod common;

use std::cell::Cell;

use common::{any_track, curves, rate, splitmix, std_rate, track_parts};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use takedown_core::canonical::RoleMap;
use takedown_core::control::{
    ControlFile, ControlTrack, Curve, Curves, Event, EventKind, GateSpan, HitEvent, Lane, Link,
    MAX_HIT_MS, MAX_LEN, TrackError, TrackParts, U7, Velocity,
};
use takedown_core::hit::RoleName;
use takedown_core::ids::{Hash32, TakeId};
use takedown_core::time::{SampleRate, TimelinePos, control_index, frame_start};
use takedown_core::views::{covered_frames, derive_key, hit_view, note_view};

fn curve_bits(c: &Curves) -> Vec<Vec<u32>> {
    Curve::ALL
        .iter()
        .map(|k| c.get(*k).iter().map(|v| v.to_bits()).collect())
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum Breakage {
    Nan,
    Infinite,
    OutOfRange,
    CurveLength,
    LenZero,
    LenHuge,
    EmptySpan,
    OverlapSpan,
    UnsortedSpans,
    WrongLink,
    OffBeyondLen,
    ZeroDur,
    LongDur,
    Overflow,
    UnsortedHits,
    RoleIndex,
    UnsortedRoles,
    AnchorBeyond,
}

const BREAKAGES: [Breakage; 18] = [
    Breakage::Nan,
    Breakage::Infinite,
    Breakage::OutOfRange,
    Breakage::CurveLength,
    Breakage::LenZero,
    Breakage::LenHuge,
    Breakage::EmptySpan,
    Breakage::OverlapSpan,
    Breakage::UnsortedSpans,
    Breakage::WrongLink,
    Breakage::OffBeyondLen,
    Breakage::ZeroDur,
    Breakage::LongDur,
    Breakage::Overflow,
    Breakage::UnsortedHits,
    Breakage::RoleIndex,
    Breakage::UnsortedRoles,
    Breakage::AnchorBeyond,
];

fn simple_hit(at: u64) -> HitEvent {
    HitEvent {
        at: TimelinePos(at),
        anchor: TimelinePos(at),
        dur: 1,
        role: 0,
        vel: Velocity::new(100).unwrap(),
        bright: U7::new(64).unwrap(),
        breath: U7::new(0).unwrap(),
        mouth: U7::new(0).unwrap(),
    }
}

fn simple_span(on: u64, off: u64, link: Link) -> GateSpan {
    GateSpan {
        on: TimelinePos(on),
        off: TimelinePos(off),
        anchor: TimelinePos(on),
        vel: Velocity::new(100).unwrap(),
        link,
    }
}

fn ensure_hit(p: &mut TrackParts) {
    if p.roles.is_empty() {
        p.roles.push(RoleName::new("kick").unwrap());
    }
    if p.hits.is_empty() {
        p.hits.push(simple_hit(0));
    }
}

/// At least two spans; false when the track is too short to hold them.
fn ensure_two_spans(p: &mut TrackParts) -> bool {
    if p.spans.len() >= 2 {
        return true;
    }
    if p.len < 4 {
        return false;
    }
    p.spans = vec![
        simple_span(0, 1, Link::Detached),
        simple_span(2, 3, Link::Detached),
    ];
    true
}

/// Puts hits back in canonical order (as `new` would) and returns the index
/// the broken hit lands on.
fn resort(p: &mut TrackParts, i: usize) -> usize {
    let broken = p.hits[i];
    p.hits.sort_by_key(HitEvent::canonical_key);
    p.hits.iter().position(|h| *h == broken).unwrap()
}

/// Breaks exactly one invariant. Returns the error `new` must give and whether
/// JSON can carry the broken value (it writes NaN and infinities as null).
#[allow(clippy::too_many_lines)] // one arm per invariant
fn break_parts(mut p: TrackParts, b: Breakage, sel: u64) -> Option<(TrackParts, TrackError, bool)> {
    let curve = Curve::ALL[(sel % 6) as usize];
    let frame = (splitmix(sel) % p.curves.pitch_st.len() as u64) as usize;
    let pick = |n: usize| (splitmix(sel ^ 0x55) % n as u64) as usize;
    let (lo, hi) = curve.range();
    let (err, json_ok) = match b {
        Breakage::Nan => {
            p.curves.get_mut(curve)[frame] = f32::NAN;
            (
                TrackError::OutOfRange {
                    curve,
                    frame: frame as u64,
                },
                false,
            )
        }
        Breakage::Infinite => {
            p.curves.get_mut(curve)[frame] = if sel.is_multiple_of(2) {
                f32::INFINITY
            } else {
                f32::NEG_INFINITY
            };
            (
                TrackError::OutOfRange {
                    curve,
                    frame: frame as u64,
                },
                false,
            )
        }
        Breakage::OutOfRange => {
            p.curves.get_mut(curve)[frame] = if sel.is_multiple_of(2) {
                hi + 1.0
            } else {
                lo - 0.5
            };
            (
                TrackError::OutOfRange {
                    curve,
                    frame: frame as u64,
                },
                true,
            )
        }
        Breakage::CurveLength => {
            let v = p.curves.get_mut(curve);
            if sel.is_multiple_of(2) || v.len() == 1 {
                v.push(lo);
            } else {
                v.pop();
            }
            (TrackError::CurveLength { curve }, true)
        }
        Breakage::LenZero => {
            p.len = 0;
            (TrackError::Length, true)
        }
        Breakage::LenHuge => {
            p.len = MAX_LEN + 1;
            (TrackError::Length, true)
        }
        Breakage::EmptySpan => {
            if p.spans.is_empty() {
                p.spans.push(simple_span(0, 0, Link::Detached));
                (TrackError::EmptySpan(0), true)
            } else {
                let i = pick(p.spans.len());
                p.spans[i].off = p.spans[i].on;
                (TrackError::EmptySpan(i), true)
            }
        }
        Breakage::OverlapSpan => {
            if !ensure_two_spans(&mut p) {
                return None;
            }
            let i = 1 + pick(p.spans.len() - 1);
            p.spans[i].on = TimelinePos(p.spans[i - 1].off.0 - 1);
            (TrackError::SpanOrder(i), true)
        }
        Breakage::UnsortedSpans => {
            if !ensure_two_spans(&mut p) {
                return None;
            }
            let i = pick(p.spans.len() - 1);
            p.spans.swap(i, i + 1);
            p.spans[i].link = Link::Detached;
            (TrackError::SpanOrder(i + 1), true)
        }
        Breakage::WrongLink => {
            if p.spans.is_empty() {
                p.spans.push(simple_span(0, p.len.min(10), Link::Retrigger));
                (TrackError::LinkMismatch(0), true)
            } else {
                let i = pick(p.spans.len());
                let s = &mut p.spans[i];
                s.link = if s.link == Link::Detached {
                    Link::Retrigger
                } else {
                    Link::Detached
                };
                (TrackError::LinkMismatch(i), true)
            }
        }
        Breakage::OffBeyondLen => {
            let len = p.len;
            match p.spans.last_mut() {
                Some(s) => s.off = TimelinePos(len + 1),
                None => p.spans.push(simple_span(0, len + 1, Link::Detached)),
            }
            (TrackError::SpanBeyondEnd(p.spans.len() - 1), true)
        }
        Breakage::ZeroDur => {
            ensure_hit(&mut p);
            let i = pick(p.hits.len());
            p.hits[i].dur = 0;
            (TrackError::HitDuration(resort(&mut p, i)), true)
        }
        Breakage::LongDur => {
            ensure_hit(&mut p);
            let i = pick(p.hits.len());
            p.hits[i].dur = p.rate.samples_for_ms(MAX_HIT_MS) + 1;
            (TrackError::HitDuration(resort(&mut p, i)), true)
        }
        Breakage::Overflow => {
            ensure_hit(&mut p);
            let i = pick(p.hits.len());
            p.hits[i].at = TimelinePos(u64::MAX - 1);
            p.hits[i].dur = 2;
            (TrackError::HitBeyondEnd(resort(&mut p, i)), true)
        }
        Breakage::UnsortedHits => {
            ensure_hit(&mut p);
            if !p
                .hits
                .windows(2)
                .any(|w| w[0].canonical_key() < w[1].canonical_key())
            {
                let mut h = p.hits[0];
                h.vel = Velocity::new(if h.vel.get() == 1 { 2 } else { 1 }).unwrap();
                p.hits.push(h);
                p.hits.sort_by_key(HitEvent::canonical_key);
            }
            let j = p
                .hits
                .windows(2)
                .position(|w| w[0].canonical_key() < w[1].canonical_key())
                .unwrap();
            p.hits.swap(j, j + 1);
            (TrackError::HitOrder(j + 1), true)
        }
        Breakage::RoleIndex => {
            ensure_hit(&mut p);
            let i = pick(p.hits.len());
            p.hits[i].role = p.roles.len() as u8;
            (TrackError::RoleIndex(resort(&mut p, i)), true)
        }
        Breakage::UnsortedRoles => {
            match p.roles.len() {
                0 => {
                    p.roles = vec![
                        RoleName::new("snare").unwrap(),
                        RoleName::new("kick").unwrap(),
                    ];
                }
                1 => p.roles.push(RoleName::new("a").unwrap()),
                _ => p.roles.swap(0, 1),
            }
            (TrackError::Roles, true)
        }
        Breakage::AnchorBeyond => {
            let len = p.len;
            if !p.hits.is_empty() && (sel.is_multiple_of(2) || p.spans.is_empty()) {
                let i = pick(p.hits.len());
                p.hits[i].anchor = TimelinePos(len + 1);
                resort(&mut p, i);
            } else if let Some(s) = p.spans.first_mut() {
                s.anchor = TimelinePos(len + 1);
            } else {
                ensure_hit(&mut p);
                p.hits[0].anchor = TimelinePos(len + 1);
            }
            (TrackError::AnchorBeyondEnd, true)
        }
    };
    Some((p, err, json_ok))
}

/// The phrase events the spec defines, straight from the spans.
fn expected_phrase(spans: &[GateSpan]) -> Vec<Event> {
    let mut out = Vec::new();
    for (i, s) in spans.iter().enumerate() {
        let span = i as u32;
        let open = match s.link {
            Link::Detached => Some(EventKind::NoteOn { span, vel: s.vel }),
            Link::Retrigger => Some(EventKind::Retrigger { span, vel: s.vel }),
            Link::Legato => None,
        };
        if let Some(kind) = open {
            out.push(Event {
                at: s.on,
                anchor: s.anchor,
                kind,
            });
        }
        if spans.get(i + 1).is_none_or(|n| n.on != s.off) {
            out.push(Event {
                at: s.off,
                anchor: s.anchor,
                kind: EventKind::NoteOff { span },
            });
        }
    }
    out
}

fn check_notes(t: &ControlTrack, probes: &[u64]) -> Result<(), TestCaseError> {
    let notes = note_view(t);
    prop_assert_eq!(notes.len(), t.spans().len());
    let (mut note_len, mut span_len) = (0u64, 0u64);
    for (i, (n, s)) in notes.iter().zip(t.spans()).enumerate() {
        prop_assert_eq!(n.span, i);
        prop_assert_eq!((n.on, n.off, n.vel, n.link), (s.on, s.off, s.vel, s.link));
        prop_assert_eq!(&n.frames, &covered_frames(s.on, s.off, t.rate()));
        // Covered frames are exactly those whose first sample lies in [on, off).
        let fs = |k: u64| frame_start(k, t.rate());
        let (start, end) = (n.frames.start, n.frames.end);
        prop_assert!(start <= end);
        prop_assert!(fs(start) >= n.on.0);
        if start > 0 {
            prop_assert!(fs(start - 1) < n.on.0);
        }
        if end > start {
            prop_assert!(fs(end - 1) < n.off.0);
        }
        prop_assert!(fs(end) >= n.off.0);
        note_len += n.off.0 - n.on.0;
        span_len += s.off.0 - s.on.0;
    }
    prop_assert_eq!(note_len, span_len);
    let mut positions: Vec<u64> = probes.iter().map(|p| p % (t.len() + 1)).collect();
    for s in t.spans() {
        positions.extend([s.on.0.saturating_sub(1), s.on.0, s.off.0 - 1, s.off.0]);
    }
    for p in positions {
        let want = notes.iter().position(|n| n.on.0 <= p && p < n.off.0);
        prop_assert_eq!(t.gate_at(TimelinePos(p)), want, "position {}", p);
    }
    Ok(())
}

proptest! {
    #[test]
    fn control_track_json_roundtrip_is_exact(t in any_track()) {
        let json = serde_json::to_vec(&t).unwrap();
        let back: ControlTrack = serde_json::from_slice(&json).unwrap();
        prop_assert_eq!(curve_bits(back.curves()), curve_bits(t.curves()));
        prop_assert_eq!(back.content_id(), t.content_id());
        prop_assert_eq!(t.content_id(), t.clone().content_id());
        prop_assert_eq!(serde_json::to_vec(&back).unwrap(), json);
        let file = ControlFile { schema_version: 1, track: t };
        let text = serde_json::to_string(&file).unwrap();
        let back: ControlFile = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(back.track.content_id(), file.track.content_id());
        let extra = text.replacen('{', "{\"extra\":0,", 1);
        prop_assert!(serde_json::from_str::<ControlFile>(&extra).is_err());
    }

    #[test]
    fn invalid_tracks_are_refused(
        parts in std_rate().prop_flat_map(track_parts),
        which in 0usize..BREAKAGES.len(),
        sel in any::<u64>(),
    ) {
        let b = BREAKAGES[which];
        let Some((broken, err, json_ok)) = break_parts(parts, b, sel) else {
            return Ok(());
        };
        let made = ControlTrack::new(broken.clone());
        if matches!(b, Breakage::UnsortedHits) {
            // new sorts: the same hits in canonical order are a valid track.
            let mut sorted = broken.clone();
            sorted.hits.sort_by_key(HitEvent::canonical_key);
            prop_assert_eq!(made.map(ControlTrack::into_parts).ok(), Some(sorted));
        } else {
            prop_assert_eq!(made.err(), Some(err));
        }
        let json = serde_json::to_string(&broken).unwrap();
        let loaded = serde_json::from_str::<ControlTrack>(&json);
        prop_assert!(loaded.is_err(), "{:?} was loaded", b);
        if json_ok {
            let msg = loaded.unwrap_err().to_string();
            prop_assert!(msg.contains(&err.to_string()), "{:?}: {}", b, msg);
        }
    }

    #[test]
    fn events_are_canonical(t in any_track()) {
        let ev: Vec<Event> = t.events().collect();
        for w in ev.windows(2) {
            prop_assert!(w[0].at <= w[1].at);
            if w[0].at == w[1].at {
                prop_assert!(!(w[0].kind.lane() == Lane::Hits && w[1].kind.lane() == Lane::Phrase));
            }
        }
        let hits: Vec<u32> = ev
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Hit { hit } => {
                    let h = &t.hits()[hit as usize];
                    assert_eq!((e.at, e.anchor), (h.at, h.anchor));
                    Some(hit)
                }
                _ => None,
            })
            .collect();
        prop_assert_eq!(hits, (0..t.hits().len() as u32).collect::<Vec<_>>());
        let phrase: Vec<Event> = ev.iter().copied().filter(|e| e.kind.lane() == Lane::Phrase).collect();
        prop_assert_eq!(&phrase, &expected_phrase(t.spans()));
        // The gate opens and closes exactly once per note group.
        let mut open = false;
        for e in &phrase {
            match e.kind {
                EventKind::NoteOn { .. } => {
                    prop_assert!(!open);
                    open = true;
                }
                EventKind::Retrigger { .. } => prop_assert!(open),
                EventKind::NoteOff { .. } => {
                    prop_assert!(open);
                    open = false;
                }
                EventKind::Hit { .. } => unreachable!(),
            }
        }
        prop_assert!(!open);
        prop_assert_eq!(t.events().count(), ev.len());
    }

    #[test]
    fn gate_and_notes_agree(t in any_track(), probes in vec(any::<u64>(), 16)) {
        check_notes(&t, &probes)?;
    }

    #[test]
    fn boundaries_ignore_curves(
        (t, replacement) in any_track().prop_flat_map(|t| {
            let n = t.frames();
            (Just(t), curves(n))
        }),
    ) {
        let mut p = t.clone().into_parts();
        p.curves = replacement;
        let t2 = ControlTrack::new(p).unwrap();
        let (a, b) = (note_view(&t), note_view(&t2));
        prop_assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            prop_assert_eq!(
                (x.span, x.on, x.off, x.vel, x.link, &x.frames),
                (y.span, y.on, y.off, y.vel, y.link, &y.frames)
            );
        }
    }

    #[test]
    fn hit_view_follows_role_map(t in any_track()) {
        let map = RoleMap::gm_default();
        let views = hit_view(&t, &map);
        prop_assert_eq!(views.len(), t.hits().len());
        for (v, h) in views.iter().zip(t.hits()) {
            let role = &t.roles()[usize::from(h.role)];
            prop_assert_eq!(&v.role, role);
            prop_assert_eq!(v.key, map.keys.get(role).copied());
            prop_assert_eq!((v.at, v.dur, v.vel, v.bright, v.breath, v.mouth), (h.at, h.dur, h.vel, h.bright, h.breath, h.mouth));
        }
    }
}

fn lower_median(values: &[f32]) -> f32 {
    let mut v = values.to_vec();
    v.sort_by(f32::total_cmp);
    v[(v.len() - 1) / 2]
}

/// Perturbations smaller than each median's distance to a half-semitone edge
/// never change a key. Notes within 1e-3 st of an edge are counted, not
/// silently skipped, and frames no note reads are perturbed freely.
#[test]
fn keys_stable_under_small_perturbation() {
    const EDGE: f32 = 1e-3;
    let checked = Cell::new(0u64);
    let near_edge = Cell::new(0u64);
    let mut runner = TestRunner::default();
    runner
        .run(&(any_track(), any::<u64>()), |(t, salt)| {
            let notes = note_view(&t);
            let pitch = &t.curves().pitch_st;
            let last = pitch.len() as u64 - 1;
            let mut bound = vec![f32::INFINITY; pitch.len()];
            for n in &notes {
                let frames = if n.frames.is_empty() {
                    let rep = control_index(n.on.0, t.rate()).0.min(last);
                    rep..rep + 1
                } else {
                    n.frames.clone()
                };
                let m = lower_median(&pitch[frames.start as usize..frames.end as usize]);
                let d = 0.5 - (m - m.round()).abs();
                let b = if d < EDGE {
                    near_edge.set(near_edge.get() + 1);
                    0.0
                } else {
                    checked.set(checked.get() + 1);
                    0.45 * d
                };
                for f in frames {
                    bound[f as usize] = bound[f as usize].min(b);
                }
            }
            let (lo, hi) = Curve::PitchSt.range();
            let mut p = t.clone().into_parts();
            for (i, v) in p.curves.pitch_st.iter_mut().enumerate() {
                let u = (splitmix(salt ^ i as u64) >> 40) as f32 / 16_777_216.0 * 2.0 - 1.0;
                let b = if bound[i].is_finite() { bound[i] } else { 5.0 };
                *v = (*v + u * b).clamp(lo, hi);
            }
            let t2 = ControlTrack::new(p).unwrap();
            let keys = |t: &ControlTrack| note_view(t).iter().map(|n| n.key).collect::<Vec<_>>();
            prop_assert_eq!(keys(&t), keys(&t2));
            Ok(())
        })
        .unwrap();
    eprintln!(
        "keys_stable_under_small_perturbation: {} notes checked, {} within {EDGE} st of an edge",
        checked.get(),
        near_edge.get()
    );
    assert!(
        checked.get() > 10 * near_edge.get().max(1),
        "the test is mostly vacuous"
    );
}

#[test]
fn derive_key_is_the_rounded_lower_median() {
    let pitch = [60.0, 72.0, 61.4, 59.0, 61.6, -70.0, 200.0];
    assert_eq!(derive_key(&pitch, 0..4, 0), 60); // 59, 60, 61.4, 72 -> 60
    assert_eq!(derive_key(&pitch, 2..5, 0), 61); // 59, 61.4, 61.6 -> 61.4
    assert_eq!(derive_key(&pitch, 3..3, 4), 62); // empty: rep frame 4
    assert_eq!(derive_key(&pitch, 5..6, 0), 0); // clamped low
    assert_eq!(derive_key(&pitch, 6..7, 0), 127); // clamped high
    assert_eq!(derive_key(&pitch, 6..100, 0), 127); // clipped to the curve
    assert_eq!(derive_key(&[], 0..1, 0), 60);
}

fn regression_track(r: SampleRate, len: u64, spans: Vec<GateSpan>) -> ControlTrack {
    let n = takedown_core::time::frames_covering(len, r) as usize;
    ControlTrack::new(TrackParts {
        rate: r,
        len,
        take: None,
        roles: vec![],
        curves: Curves {
            pitch_st: vec![48.0; n],
            amp_db: vec![-6.0; n],
            bright: vec![0.5; n],
            breath: vec![0.1; n],
            vowel_x: vec![0.0; n],
            vowel_y: vec![0.0; n],
        },
        spans,
        hits: vec![],
    })
    .unwrap()
}

/// J2: a one-second audible hum must export exactly one note covering it.
#[test]
fn gated_hum_is_one_note() {
    let t = regression_track(
        rate(48_000),
        57_600,
        vec![simple_span(4_800, 52_800, Link::Detached)],
    );
    let notes = note_view(&t);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].off.0 - notes[0].on.0, 48_000);
    assert_eq!(notes[0].key, 48);
    let kinds: Vec<EventKind> = t.events().map(|e| e.kind).collect();
    assert!(matches!(
        kinds[..],
        [
            EventKind::NoteOn { span: 0, .. },
            EventKind::NoteOff { span: 0 }
        ]
    ));
    check_notes(&t, &[0, 4_799, 4_800, 52_799, 52_800]).unwrap();
}

/// C: a retriggered phrase must keep every segment, 95,000 of 95,000 samples.
#[test]
fn retriggered_phrase_keeps_every_segment() {
    let t = regression_track(
        rate(48_000),
        95_000,
        vec![
            simple_span(0, 48_000, Link::Detached),
            simple_span(48_000, 95_000, Link::Retrigger),
        ],
    );
    let notes = note_view(&t);
    assert_eq!(notes.len(), 2);
    assert_eq!(notes.iter().map(|n| n.off.0 - n.on.0).sum::<u64>(), 95_000);
    let kinds: Vec<EventKind> = t.events().map(|e| e.kind).collect();
    assert!(matches!(
        kinds[..],
        [
            EventKind::NoteOn { span: 0, .. },
            EventKind::Retrigger { span: 1, .. },
            EventKind::NoteOff { span: 1 }
        ]
    ));
    check_notes(&t, &[47_999, 48_000, 94_999, 95_000]).unwrap();
}

#[test]
fn slice_span_clips_to_the_track() {
    let r = rate(48_000);
    let mut p = regression_track(r, 1_000, vec![]).into_parts();
    assert_eq!(
        ControlTrack::new(p.clone())
            .unwrap()
            .slice_span(TimelinePos(10), 50),
        None
    );
    p.take = Some(TakeId::from_hash(Hash32([9; 32])));
    let t = ControlTrack::new(p).unwrap();
    let s = t.slice_span(TimelinePos(980), 50).unwrap();
    assert_eq!((s.start().0, s.end().0), (980, 1_000));
    let s = t.slice_span(TimelinePos(1_000), u64::MAX).unwrap();
    assert!(s.is_empty());
}
