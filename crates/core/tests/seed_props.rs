//! core-types §3.2: the seed tree and noise.

mod common;

use std::collections::{BTreeMap, BTreeSet, HashSet};

use common::{
    any_track, aspect, check_frozen, event_seeds, region_probe_anchors, region_span, seed_tree,
};
use proptest::collection::vec;
use proptest::prelude::*;
use takedown_core::control::{ControlTrack, Curves, EventKind, HitEvent, TrackParts, U7, Velocity};
use takedown_core::hit::RoleName;
use takedown_core::seed::{Aspect, RegionRoll, Seed, SeedTree, TriggerGroup, VoiceSeeds};
use takedown_core::time::{TimelinePos, TimelineSpan};

const VOICES: [&str; 2] = ["kit/kick_body", "texture"];

fn role(name: &str) -> RoleName {
    RoleName::new(name).expect("a valid role name")
}

/// The phrase lane and two hit roles: every kind of trigger group.
fn groups(roles: &[RoleName]) -> Vec<TriggerGroup<'_>> {
    std::iter::once(TriggerGroup::Phrase)
        .chain(roles.iter().map(TriggerGroup::Role))
        .collect()
}

/// Everything observable that derives from aspect `a`, at the given anchors.
fn fingerprint(t: &SeedTree, a: Aspect, anchors: &[TimelinePos]) -> Vec<u64> {
    let mut out = vec![t.aspect(a).0];
    for at in anchors {
        out.push(t.aspect_at(a, *at).0);
    }
    let pick = |s: takedown_core::seed::VoiceSeeds| match a {
        Aspect::Body => Some(s.body.0),
        Aspect::Mangle => Some(s.mangle.0),
        _ => None,
    };
    let roles = [role("kick"), role("sub")];
    for v in VOICES {
        out.extend(pick(t.voice_seeds(v)));
        for at in anchors {
            for group in groups(&roles) {
                for ordinal in 0..2 {
                    out.extend(pick(t.trigger_seeds(v, group, *at, ordinal)));
                }
            }
        }
    }
    out
}

#[derive(Clone, Debug)]
enum Op {
    Roll(Aspect),
    Lock(Aspect),
    Unlock(Aspect),
    RollUnlocked,
    RollRegion(Aspect, TimelineSpan),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => aspect().prop_map(Op::Roll),
        1 => aspect().prop_map(Op::Lock),
        1 => aspect().prop_map(Op::Unlock),
        1 => Just(Op::RollUnlocked),
        2 => (aspect(), region_span()).prop_map(|(a, s)| Op::RollRegion(a, s)),
    ]
}

fn apply(t: &mut SeedTree, op: &Op) {
    match op {
        Op::Roll(a) => {
            t.roll(*a);
        }
        Op::Lock(a) => {
            t.locked.insert(*a);
        }
        Op::Unlock(a) => {
            t.locked.remove(a);
        }
        Op::RollUnlocked => t.roll_unlocked(),
        Op::RollRegion(a, s) => t.roll_region(*a, *s),
    }
}

/// A track with a seed tree, plus one fresh anchor nobody uses.
fn track_tree() -> impl Strategy<Value = (ControlTrack, SeedTree, u64)> {
    (any_track(), seed_tree(), any::<u64>())
}

proptest! {
    /// ★ CLAUDE.md: seed-tree independence.
    #[test]
    fn seed_tree_independence(
        mut t in seed_tree(),
        a in aspect(),
        ops in vec(op(), 0..24),
    ) {
        let anchors = region_probe_anchors(&t);
        for op in ops {
            // Rolls of a itself are the one thing allowed to move a.
            let touches_a = matches!(op, Op::Roll(b) | Op::RollRegion(b, _) if b == a);
            if touches_a {
                continue;
            }
            let before = fingerprint(&t, a, &anchors);
            let a_rolls = matches!(op, Op::RollUnlocked) && !t.locked.contains(&a);
            apply(&mut t, &op);
            let after = fingerprint(&t, a, &anchors);
            if a_rolls {
                prop_assert_ne!(before[0], after[0], "roll_unlocked must roll an unlocked aspect");
            } else {
                prop_assert_eq!(before, after, "{:?} moved aspect {:?}", op, a);
            }
        }
        prop_assert!(t.validate().is_ok());
    }

    #[test]
    fn undo_restores(
        mut t in seed_tree(),
        a in aspect(),
        k in 1u32..5,
        span in region_span(),
    ) {
        t.locked.remove(&a);
        let anchors = region_probe_anchors(&t);
        let before = t.clone();
        let prints: Vec<_> = Aspect::ALL.iter().map(|x| fingerprint(&t, *x, &anchors)).collect();

        // Counter: roll, then set the counter back.
        let old = t.rolls.get(&a).copied();
        for _ in 0..k {
            prop_assert!(t.roll(a));
        }
        prop_assert_ne!(t.aspect(a), before.aspect(a));
        match old {
            Some(n) => t.rolls.insert(a, n),
            None => t.rolls.remove(&a),
        };
        // An explicit zero means the same as a missing entry.
        let mut zero = t.clone();
        zero.rolls.entry(a).or_insert(0);
        prop_assert_eq!(zero.aspect(a), t.aspect(a));

        // Region: roll, then set the region's counter back.
        let old_region = t.regions.iter().find(|r| r.aspect == a && r.span == span).map(|r| r.roll);
        for _ in 0..k {
            t.roll_region(a, span);
        }
        match old_region {
            Some(n) => t.regions.iter_mut().filter(|r| r.aspect == a && r.span == span).for_each(|r| r.roll = n),
            None => t.regions.retain(|r| !(r.aspect == a && r.span == span)),
        }
        prop_assert_eq!(&t, &before);
        let after: Vec<_> = Aspect::ALL.iter().map(|x| fingerprint(&t, *x, &anchors)).collect();
        prop_assert_eq!(prints, after);
    }

    #[test]
    fn region_rolls_are_local(
        mut t in seed_tree(),
        a in aspect(),
        span in region_span(),
        extra in vec(0u64..3_100, 0..8),
    ) {
        t.locked.remove(&a);
        let mut anchors = region_probe_anchors(&t);
        let (s, e) = (span.start().0, span.end().0);
        anchors.extend([s.saturating_sub(1), s, e.saturating_sub(1), e, e + 1].map(TimelinePos));
        anchors.extend(extra.into_iter().map(TimelinePos));
        let before = t.clone();
        t.roll_region(a, span);

        for v in VOICES {
            prop_assert_eq!(t.voice_seeds(v), before.voice_seeds(v));
        }
        for at in &anchors {
            for b in Aspect::ALL {
                let moved = t.aspect_at(b, *at) != before.aspect_at(b, *at);
                prop_assert_eq!(moved, b == a && span.contains(*at), "aspect {:?} at {}", b, at.0);
            }
            let roles = [role("kick"), role("sub")];
            for v in VOICES {
                for group in groups(&roles) {
                    let x = t.trigger_seeds(v, group, *at, 0);
                    let y = before.trigger_seeds(v, group, *at, 0);
                    let inside = span.contains(*at);
                    prop_assert_eq!(x.body != y.body, a == Aspect::Body && inside);
                    prop_assert_eq!(x.mangle != y.mangle, a == Aspect::Mangle && inside);
                }
            }
        }
    }

    #[test]
    fn region_order_is_irrelevant(
        root in any::<u64>(),
        regions in vec((aspect(), region_span()), 0..6),
    ) {
        let mut fwd = SeedTree::new(Seed(root));
        let mut rev = SeedTree::new(Seed(root));
        for (a, s) in &regions {
            fwd.roll_region(*a, *s);
        }
        for (a, s) in regions.iter().rev() {
            rev.roll_region(*a, *s);
        }
        prop_assert_eq!(&fwd, &rev);
        prop_assert!(fwd.validate().is_ok());
        for at in region_probe_anchors(&fwd) {
            for a in Aspect::ALL {
                prop_assert_eq!(fwd.aspect_at(a, at), rev.aspect_at(a, at));
            }
        }
    }

    #[test]
    fn trigger_seeds_key_on_anchor((track, tree, salt) in track_tree()) {
        let base = event_seeds(&track, &tree, VOICES[0]);
        let used: BTreeSet<u64> = base.iter().map(|e| e.1.0).collect();
        let mut parts = track.clone().into_parts();
        let mut removed = None;
        if !parts.roles.is_empty() && salt % 2 == 0 {
            // Insert a hit at a fresh anchor, which shifts the index of every later hit.
            let fresh = (0..=parts.len).map(|i| (salt / 2).wrapping_add(i) % parts.len).find(|p| !used.contains(p));
            if let Some(at) = fresh {
                parts.hits.push(HitEvent {
                    at: TimelinePos(at),
                    anchor: TimelinePos(at),
                    dur: 1,
                    role: 0,
                    vel: Velocity::new(100).unwrap(),
                    bright: U7::new(64).unwrap(),
                    breath: U7::new(0).unwrap(),
                    mouth: U7::new(0).unwrap(),
                });
            }
        } else if !parts.hits.is_empty() {
            // Remove a hit that is alone in its (role, anchor) group.
            let i = (salt % parts.hits.len() as u64) as usize;
            let (anchor, r) = (parts.hits[i].anchor, parts.hits[i].role);
            if parts.hits.iter().filter(|h| h.anchor == anchor && h.role == r).count() == 1 {
                removed = Some((Some(parts.roles[usize::from(r)].clone()), anchor));
                parts.hits.remove(i);
            }
        }
        let edited = ControlTrack::new(parts).unwrap();
        let after: BTreeMap<_, _> = event_seeds(&edited, &tree, VOICES[0])
            .into_iter()
            .map(|(l, a, o, s)| ((l, a, o), s))
            .collect();
        for (group, anchor, ordinal, seeds) in base {
            if removed.as_ref() == Some(&(group.clone(), anchor)) {
                continue;
            }
            prop_assert_eq!(after.get(&(group, anchor, ordinal)), Some(&seeds));
        }
    }

    #[test]
    fn same_anchor_events_decorrelate(
        tree in seed_tree(),
        anchor in 0u64..3_000,
        n in 2u32..=3,
    ) {
        // Layered hits of distinct roles, repeats of one role, and the
        // phrase lane: every (voice, group, ordinal) gets its own seeds.
        let roles = [role("kick"), role("sub"), role("snare")];
        let mut body = HashSet::new();
        let mut mangle = HashSet::new();
        for v in VOICES {
            for group in groups(&roles) {
                for ordinal in 0..n {
                    let s = tree.trigger_seeds(v, group, TimelinePos(anchor), ordinal);
                    prop_assert!(body.insert(s.body));
                    prop_assert!(mangle.insert(s.mangle));
                }
            }
        }
    }

    #[test]
    fn noise_is_counter_based(
        seed in any::<u64>(),
        stream in any::<u64>(),
        offset in any::<u64>(),
        chunks in vec(1u64..=700, 1..16),
    ) {
        let ns = Seed(seed).noise(stream);
        let mut n = offset;
        let mut seq = Vec::new();
        for c in chunks {
            // A fresh handle per chunk, as a backend would hold across blocks.
            let chunk_stream = Seed(seed).noise(stream);
            for i in 0..c {
                seq.push(chunk_stream.u64_at(n.wrapping_add(i)));
            }
            n = n.wrapping_add(c);
        }
        for (i, v) in seq.iter().enumerate() {
            let at = offset.wrapping_add(i as u64);
            prop_assert_eq!(*v, ns.u64_at(at));
            let u = ns.unit_at(at);
            let b = ns.bipolar_at(at);
            prop_assert!((0.0..1.0).contains(&u));
            prop_assert!((-1.0..1.0).contains(&b));
            // Exact grids: 2^-24 and 2^-23.
            prop_assert!((u * 16_777_216.0).fract() == 0.0);
            prop_assert!((b * 8_388_608.0).fract() == 0.0);
        }
        prop_assert_ne!(Seed(seed).noise(stream), Seed(seed).noise(stream.wrapping_add(1)));
    }

    #[test]
    fn noise_statistics(seed in any::<u64>(), stream in any::<u64>(), offset in any::<u64>()) {
        const N: u64 = 1_000_000;
        let ns = Seed(seed).noise(stream);
        let (mut sum, mut sum_sq, mut lag, mut bip) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut prev = f64::from(ns.unit_at(offset)) - 0.5;
        for i in 0..N {
            let n = offset.wrapping_add(i);
            let x = f64::from(ns.unit_at(n)) - 0.5;
            sum += x;
            sum_sq += x * x;
            if i > 0 {
                lag += x * prev;
            }
            prev = x;
            bip += f64::from(ns.bipolar_at(n));
        }
        let n = N as f64;
        let mean = sum / n;
        let var = sum_sq / n - mean * mean;
        let r1 = (lag / (n - 1.0) - mean * mean) / var;
        // Standard errors: 2.9e-4 for the mean, 1e-3 for r1, 5.8e-4 for bipolar.
        prop_assert!(mean.abs() < 3e-3, "mean {}", mean + 0.5);
        prop_assert!(r1.abs() < 7e-3, "lag-1 correlation {}", r1);
        prop_assert!((bip / n).abs() < 6e-3, "bipolar mean {}", bip / n);
        prop_assert!((var - 1.0 / 12.0).abs() < 2e-3, "variance {}", var);
    }

    #[test]
    fn derive_separates_labels_and_indices(parent in any::<u64>()) {
        // Every label over {a, b, \0, 1} up to length 4 (341 labels), with
        // indices whose bytes collide with label bytes: 10,230 children.
        let alphabet = ["a", "b", "\0", "1"];
        let mut labels = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..4 {
            frontier = frontier
                .iter()
                .flat_map(|s| alphabet.iter().map(move |c| format!("{s}{c}")))
                .collect();
            labels.extend(frontier.iter().cloned());
        }
        let p = Seed(parent);
        let mut seen = HashSet::new();
        for label in &labels {
            for idx in (0..15).chain([0x61, 0x6161, 0x31, 1 << 56, u64::MAX].into_iter()).chain(0x6200..0x620A) {
                prop_assert!(seen.insert(p.derive(label, idx)), "collision at {:?}/{}", label, idx);
            }
        }
        prop_assert!(seen.len() >= 10_000);
    }
}

/// Per (group, anchor), the events in canonical order: the hit (`None` for
/// a phrase event) and its seeds.
type Groups = BTreeMap<(Option<RoleName>, TimelinePos), Vec<(Option<HitEvent>, VoiceSeeds)>>;

fn groups_of(track: &ControlTrack, tree: &SeedTree, voice: &str) -> Groups {
    let mut out = Groups::new();
    for (e, (group, anchor, _, seeds)) in track.events().zip(event_seeds(track, tree, voice)) {
        let hit = match e.kind {
            EventKind::Hit { hit } => Some(track.hits()[hit as usize]),
            EventKind::NoteOn { .. } | EventKind::Retrigger { .. } | EventKind::NoteOff { .. } => {
                None
            }
        };
        out.entry((group, anchor)).or_default().push((hit, seeds));
    }
    out
}

/// A track, a seed tree, a hit to move (preferring one layered with another
/// role at its anchor) and where to move it: just before or just after a
/// sibling, or anywhere.
fn retime(track: &ControlTrack, salt: u64, to: u64) -> Option<(usize, ControlTrack)> {
    let hits = track.hits();
    if hits.is_empty() {
        return None;
    }
    let siblings = |i: usize| -> Vec<u64> {
        hits.iter()
            .filter(|h| h.anchor == hits[i].anchor && h.role != hits[i].role)
            .map(|h| h.at.0)
            .collect()
    };
    let layered: Vec<usize> = (0..hits.len())
        .filter(|&i| !siblings(i).is_empty())
        .collect();
    let i = if layered.is_empty() {
        (salt % hits.len() as u64) as usize
    } else {
        layered[(salt % layered.len() as u64) as usize]
    };
    let room = track.len() - hits[i].dur;
    let near = siblings(i);
    let wanted = match (to % 3, near.is_empty()) {
        (0, false) => near[((to / 3) % near.len() as u64) as usize].saturating_sub(1),
        (1, false) => near[((to / 3) % near.len() as u64) as usize] + 1,
        _ => (to / 3) % (room + 1),
    };
    let mut parts = track.clone().into_parts();
    parts.hits[i].at = TimelinePos(wanted.min(room));
    Some((
        i,
        ControlTrack::new(parts).expect("a retimed hit stays valid"),
    ))
}

proptest! {
    /// Moving one hit's `at` (pre-roll, timing feel, a Reaper nudge) leaves
    /// the seeds of every other (role, anchor) group untouched, so a layered
    /// sibling of another role never re-rolls. The moved hit keeps its own
    /// seeds when it is alone in its group.
    #[test]
    fn retiming_a_hit_keeps_other_roles_seeds(
        (track, tree, salt) in track_tree(),
        to in any::<u64>(),
    ) {
        let Some((i, edited)) = retime(&track, salt, to) else {
            return Ok(());
        };
        let moved = track.hits()[i];
        let own = (Some(track.roles()[usize::from(moved.role)].clone()), moved.anchor);
        for v in VOICES {
            let before = groups_of(&track, &tree, v);
            let after = groups_of(&edited, &tree, v);
            prop_assert_eq!(before.keys().collect::<Vec<_>>(), after.keys().collect::<Vec<_>>());
            for (key, events) in &before {
                if *key == own {
                    if events.len() == 1 {
                        prop_assert_eq!(after[key][0].1, events[0].1, "a lone hit's seeds followed its at");
                    }
                    continue;
                }
                prop_assert_eq!(&after[key], events, "group {:?} changed when another hit moved", key);
            }
        }
    }
}

/// The review's repro: a kick and a sub layered on one onset at 48 kHz. The
/// sub's pre-roll moves it 100 samples earlier; the kick keeps its seeds, and
/// the two roles never share seeds.
#[test]
fn moving_a_layered_sub_keeps_the_kick() {
    let r = takedown_core::time::SampleRate::new(48_000).unwrap();
    let len = 48_000;
    let frames = takedown_core::time::frames_covering(len, r) as usize;
    let flat = |v: f32| vec![v; frames];
    let hit = |at: u64, role: u8| HitEvent {
        at: TimelinePos(at),
        anchor: TimelinePos(4_800),
        dur: 2_400,
        role,
        vel: Velocity::new(100).unwrap(),
        bright: U7::new(64).unwrap(),
        breath: U7::new(0).unwrap(),
        mouth: U7::new(0).unwrap(),
    };
    let track = |sub_at: u64| {
        ControlTrack::new(TrackParts {
            rate: r,
            len,
            take: None,
            roles: vec![role("kick"), role("sub")],
            curves: Curves {
                pitch_st: flat(60.0),
                amp_db: flat(-6.0),
                bright: flat(0.5),
                breath: flat(0.0),
                vowel_x: flat(0.0),
                vowel_y: flat(0.0),
            },
            spans: vec![],
            hits: vec![hit(4_800, 0), hit(sub_at, 1)],
        })
        .unwrap()
    };
    let tree = SeedTree::new(Seed(0x5eed));
    let kick_of = |t: &ControlTrack| {
        event_seeds(t, &tree, VOICES[0])
            .into_iter()
            .find(|e| e.0 == Some(role("kick")))
            .unwrap()
    };
    let sub_of = |t: &ControlTrack| {
        event_seeds(t, &tree, VOICES[0])
            .into_iter()
            .find(|e| e.0 == Some(role("sub")))
            .unwrap()
    };
    let (together, moved) = (track(4_800), track(4_700));
    assert_eq!(kick_of(&together), kick_of(&moved));
    assert_eq!(sub_of(&together), sub_of(&moved));
    assert_eq!(kick_of(&together).2, 0);
    assert_ne!(kick_of(&together).3, sub_of(&together).3);
}

/// The frozen tree used by the fixture.
fn frozen_tree() -> SeedTree {
    let mut t = SeedTree::new(Seed(0x0123_4567_89AB_CDEF));
    t.rolls.insert(Aspect::Body, 3);
    t.rolls.insert(Aspect::PitchReading, 1);
    t.locked.insert(Aspect::Mangle);
    t.regions.push(RegionRoll {
        aspect: Aspect::Body,
        span: TimelineSpan::new(TimelinePos(48_000), TimelinePos(96_000)).unwrap(),
        roll: 2,
    });
    t
}

#[test]
fn frozen_seeds_and_noise() {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| out.push((format!("seed/{k}"), v));
    for (parent, label, idx) in [
        (0u64, "", 0u64),
        (0, "aspect/body", 0),
        (u64::MAX, "kit/kick_body", 7),
        (0x0123_4567_89AB_CDEF, "ordinal", 2),
    ] {
        put(
            &format!("derive/{parent:016x}/{label:?}/{idx}"),
            Seed(parent).derive(label, idx).to_string(),
        );
    }
    let t = frozen_tree();
    for a in Aspect::ALL {
        put(
            &format!("tree/aspect/{}", a.name()),
            t.aspect(a).to_string(),
        );
        put(
            &format!("tree/aspect_at_60000/{}", a.name()),
            t.aspect_at(a, TimelinePos(60_000)).to_string(),
        );
    }
    let v = t.voice_seeds("kit/kick_body");
    put("tree/voice/body", v.body.to_string());
    put("tree/voice/mangle", v.mangle.to_string());
    let phrase = t.trigger_seeds(
        "kit/kick_body",
        TriggerGroup::Phrase,
        TimelinePos(60_000),
        1,
    );
    put("tree/trigger/phrase/body", phrase.body.to_string());
    put("tree/trigger/phrase/mangle", phrase.mangle.to_string());
    let kick = role("kick");
    let hit = t.trigger_seeds(
        "kit/kick_body",
        TriggerGroup::Role(&kick),
        TimelinePos(60_000),
        1,
    );
    put("tree/trigger/hits/kick/body", hit.body.to_string());
    put("tree/trigger/hits/kick/mangle", hit.mangle.to_string());
    let ns = Seed(0xDEAD_BEEF).noise(3);
    for n in [0u64, 1, 2, 1 << 40, u64::MAX] {
        put(&format!("noise/u64/{n}"), format!("{:016x}", ns.u64_at(n)));
        put(
            &format!("noise/unit/{n}"),
            format!("{:08x}", ns.unit_at(n).to_bits()),
        );
        put(
            &format!("noise/bipolar/{n}"),
            format!("{:08x}", ns.bipolar_at(n).to_bits()),
        );
    }
    check_frozen("seed/", &out);
}

#[test]
fn seed_tree_json_round_trips_and_is_strict() {
    let t = frozen_tree();
    let json = serde_json::to_string(&t).unwrap();
    assert_eq!(serde_json::from_str::<SeedTree>(&json).unwrap(), t);
    let extra = json.replacen('{', "{\"extra\":1,", 1);
    assert!(serde_json::from_str::<SeedTree>(&extra).is_err());
    assert!(serde_json::from_str::<Seed>("\"0123456789ABCDEF\"").is_err());
    assert!(serde_json::from_str::<Seed>("\"123\"").is_err());
    assert_eq!(
        serde_json::from_str::<Seed>("\"00000000000000ff\"").unwrap(),
        Seed(255)
    );
}

proptest! {
    /// Regression: `b = mix(a ^ stream)` made `u64_at(0)` of stream 0 zero for
    /// every seed (the one zero of a stream sat within `stream` of index 0).
    #[test]
    fn noise_has_no_fixed_zero(root in any::<u64>()) {
        let seed = Seed(root);
        for stream in 0..64u64 {
            let ns = seed.noise(stream);
            for n in 0..64u64 {
                prop_assert_ne!(ns.u64_at(n), 0, "stream {} index {}", stream, n);
            }
        }
        prop_assert_ne!(seed.noise(0).u64_at(0), Seed(root ^ 1).noise(0).u64_at(0));
    }
}
