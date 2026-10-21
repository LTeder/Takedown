//! core-types §3.5: edits and their application. Edits find their targets by
//! anchor, never by index; they survive re-derivation; a dormant edit is kept
//! and reported until a target comes back.
#![allow(clippy::too_many_lines)] // property bodies read best in one piece

mod common;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use common::{ROLE_POOL, any_rate, rate, splitmix, std_rate, track_parts};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use takedown_core::control::{
    ControlTrack, Curves, GateSpan, HitEvent, Link, MAX_HIT_MS, MAX_ROLES, TrackError, TrackParts,
    U7, Velocity,
};
use takedown_core::edit::{
    Articulation, DEFAULT_TOL_MS, Dormancy, Edit, EditEntry, MIN_PIECE_MS, Outcome, SPLIT_SNAP_MS,
    apply_edits, default_tol,
};
use takedown_core::hit::{HitAnchor, RoleName};
use takedown_core::ids::EditId;
use takedown_core::time::{SampleRate, TimelinePos, TimelineSpan, frames_covering};
use takedown_core::views::covered_frames;

const CASES: u32 = 256;
/// The re-derivation jitter: every anchor and boundary moves by up to this.
const JITTER_MS: u32 = 5;

/// A runner for the properties written outside `proptest!`, which has no
/// source file to persist regressions next to.
fn runner() -> TestRunner {
    TestRunner::new(Config {
        cases: CASES,
        failure_persistence: None,
        ..Config::default()
    })
}

// ---------------------------------------------------------------------------
// Small builders
// ---------------------------------------------------------------------------

fn ms(r: SampleRate, ms: u32) -> u64 {
    r.samples_for_ms(ms)
}

fn role(name: &str) -> RoleName {
    RoleName::new(name).expect("a valid role name")
}

fn pool_role(raw: u64) -> RoleName {
    role(ROLE_POOL[(raw % ROLE_POOL.len() as u64) as usize])
}

fn vel(v: u8) -> Velocity {
    Velocity::new(v).expect("1..=127")
}

fn u7(v: u8) -> U7 {
    U7::new(v).expect("0..=127")
}

fn tspan(start: u64, end: u64) -> TimelineSpan {
    TimelineSpan::new(TimelinePos(start), TimelinePos(end)).expect("start <= end")
}

fn anchor(at: u64, tol: u32) -> HitAnchor {
    HitAnchor {
        at: TimelinePos(at),
        tol,
    }
}

fn articulation(raw: u64) -> Articulation {
    if raw.is_multiple_of(2) {
        Articulation::Retrigger
    } else {
        Articulation::Legato
    }
}

fn link_of(a: Articulation) -> Link {
    match a {
        Articulation::Retrigger => Link::Retrigger,
        Articulation::Legato => Link::Legato,
    }
}

/// A signed offset in `[-max, max]`.
fn signed(raw: u64, max: u64) -> i64 {
    (raw % (2 * max + 1)) as i64 - max as i64
}

fn gspan(on: u64, off: u64, link: Link) -> GateSpan {
    GateSpan {
        on: TimelinePos(on),
        off: TimelinePos(off),
        anchor: TimelinePos(on),
        vel: vel(90),
        link,
    }
}

fn hit(at: u64, dur: u64, role: u8) -> HitEvent {
    HitEvent {
        at: TimelinePos(at),
        anchor: TimelinePos(at),
        dur,
        role,
        vel: vel(100),
        bright: u7(10),
        breath: u7(20),
        mouth: u7(30),
    }
}

fn flat_curves(r: SampleRate, len: u64) -> Curves {
    let n = frames_covering(len, r) as usize;
    Curves {
        pitch_st: vec![48.0; n],
        amp_db: vec![-6.0; n],
        bright: vec![0.5; n],
        breath: vec![0.1; n],
        vowel_x: vec![0.0; n],
        vowel_y: vec![0.0; n],
    }
}

/// A track with flat curves and nothing on it.
fn blank(r: SampleRate, len: u64) -> TrackParts {
    TrackParts {
        rate: r,
        len,
        take: None,
        roles: vec![],
        curves: flat_curves(r, len),
        spans: vec![],
        hits: vec![],
    }
}

/// Adds `name` to the sorted role table, remapping hits, and returns its index.
fn ensure_role(p: &mut TrackParts, name: &RoleName) -> u8 {
    match p.roles.binary_search(name) {
        Ok(i) => i as u8,
        Err(i) => {
            p.roles.insert(i, name.clone());
            for h in &mut p.hits {
                if usize::from(h.role) >= i {
                    h.role += 1;
                }
            }
            i as u8
        }
    }
}

fn canonical(p: TrackParts) -> TrackParts {
    ControlTrack::new(p).expect("valid parts").into_parts()
}

fn entries(edits: &[Edit]) -> Vec<EditEntry> {
    edits
        .iter()
        .enumerate()
        .map(|(i, e)| EditEntry {
            id: EditId(100 + i as u32),
            edit: e.clone(),
        })
        .collect()
}

/// Applies `edits` to `draft`, checking one report per edit with its id, in
/// list order.
fn run(draft: &TrackParts, edits: &[Edit]) -> (TrackParts, Vec<Outcome>) {
    let list = entries(edits);
    let (t, reports) = apply_edits(draft.clone(), &list).expect("a valid draft");
    assert_eq!(reports.len(), list.len());
    for (r, e) in reports.iter().zip(&list) {
        assert_eq!(r.id, e.id, "reports keep list order and ids");
    }
    (t.into_parts(), reports.iter().map(|r| r.outcome).collect())
}

fn run1(draft: &TrackParts, edit: &Edit) -> (TrackParts, Outcome) {
    let (t, o) = run(draft, std::slice::from_ref(edit));
    (t, o[0])
}

/// A hit with its role by name: what an edit may change, independent of
/// role-table shifts.
type Named = (u64, u64, u64, RoleName, u8, u8, u8, u8);

fn named_hit(p: &TrackParts, h: &HitEvent) -> Named {
    (
        h.at.0,
        h.anchor.0,
        h.dur,
        p.roles[usize::from(h.role)].clone(),
        h.vel.get(),
        h.bright.get(),
        h.breath.get(),
        h.mouth.get(),
    )
}

fn named(p: &TrackParts) -> Vec<Named> {
    let mut v: Vec<Named> = p.hits.iter().map(|h| named_hit(p, h)).collect();
    v.sort();
    v
}

fn role_of<'a>(p: &'a TrackParts, h: &HitEvent) -> &'a RoleName {
    &p.roles[usize::from(h.role)]
}

/// The span containing `pos` (test-side oracle).
fn gate_at(spans: &[GateSpan], pos: u64) -> Option<usize> {
    spans.iter().position(|s| (s.on.0..s.off.0).contains(&pos))
}

/// The union of the spans as disjoint, non-abutting intervals.
fn gate_union(spans: &[GateSpan]) -> Vec<(u64, u64)> {
    let mut out: Vec<(u64, u64)> = Vec::new();
    for s in spans {
        match out.last_mut() {
            Some(last) if last.1 == s.on.0 => last.1 = s.off.0,
            _ => out.push((s.on.0, s.off.0)),
        }
    }
    out
}

/// `u` minus `[s, e)`, as disjoint, non-abutting intervals.
fn subtract(u: &[(u64, u64)], s: u64, e: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    for &(a, b) in u {
        if a < s {
            out.push((a, b.min(s)));
        }
        if b > e {
            out.push((a.max(e), b));
        }
    }
    out
}

/// The positions of internal boundaries (abutting pairs).
fn boundaries(spans: &[GateSpan]) -> Vec<u64> {
    (1..spans.len())
        .filter(|&i| spans[i - 1].off == spans[i].on)
        .map(|i| spans[i].on.0)
        .collect()
}

/// Each consecutive pair's junction: (off of the first, on of the second).
fn junctions(spans: &[GateSpan]) -> Vec<(u64, u64)> {
    (1..spans.len())
        .map(|i| (spans[i - 1].off.0, spans[i].on.0))
        .collect()
}

fn dist_to(x: u64, a: u64, b: u64) -> u64 {
    if x < a { a - x } else { x.saturating_sub(b) }
}

// ---------------------------------------------------------------------------
// Edit generation
// ---------------------------------------------------------------------------

/// Raw choices for one edit, made concrete against the track it will meet.
#[derive(Clone, Copy, Debug)]
struct EditPlan {
    kind: u8,
    a: u64,
    b: u64,
    c: u64,
}

fn edit_plan() -> impl Strategy<Value = EditPlan> {
    (0u8..7, any::<u64>(), any::<u64>(), any::<u64>()).prop_map(|(kind, a, b, c)| EditPlan {
        kind,
        a,
        b,
        c,
    })
}

const KIND_NAMES: [&str; 7] = [
    "force_pitch",
    "split_at",
    "join_at",
    "force_role",
    "delete_hit",
    "add_hit",
    "mute_span",
];

fn kind_of(e: &Edit) -> usize {
    match e {
        Edit::ForcePitch { .. } => 0,
        Edit::SplitAt { .. } => 1,
        Edit::JoinAt { .. } => 2,
        Edit::ForceRole { .. } => 3,
        Edit::DeleteHit { .. } => 4,
        Edit::AddHit { .. } => 5,
        Edit::MuteSpan { .. } => 6,
    }
}

/// Anywhere in the track, or a little past its end.
fn any_pos(t: &TrackParts, raw: u64) -> u64 {
    raw % (t.len + t.len / 8 + 2)
}

/// Within 2 ms of an existing anchor most of the time, else anywhere.
fn hit_pos(t: &TrackParts, a: u64, b: u64) -> u64 {
    if t.hits.is_empty() || a.is_multiple_of(4) {
        return any_pos(t, b);
    }
    let h = &t.hits[((a >> 2) % t.hits.len() as u64) as usize];
    h.anchor.0.saturating_add_signed(signed(b, ms(t.rate, 2)))
}

fn tol_for(t: &TrackParts, raw: u64) -> u32 {
    match raw % 5 {
        0 => 0,
        1 => ms(t.rate, 10) as u32,
        _ => default_tol(t.rate),
    }
}

fn pick<T: Copy>(v: &[T], raw: u64) -> T {
    v[(raw % v.len() as u64) as usize]
}

/// Inside a span, near or on a boundary, at an earlier split, or anywhere.
fn split_pos(t: &TrackParts, a: u64, b: u64, prior: &[Edit]) -> u64 {
    let spans = &t.spans;
    let bs = boundaries(spans);
    let earlier: Vec<u64> = prior
        .iter()
        .filter_map(|e| match e {
            Edit::SplitAt { at, .. } => Some(at.0),
            _ => None,
        })
        .collect();
    // Spans with room for two pieces that stay clear of the minimum under
    // the re-derivation jitter.
    let margin = ms(t.rate, MIN_PIECE_MS + JITTER_MS) + 1;
    let roomy: Vec<GateSpan> = spans
        .iter()
        .copied()
        .filter(|s| s.off.0 - s.on.0 > 2 * margin)
        .collect();
    match a % 6 {
        0 if !roomy.is_empty() => {
            let s = pick(&roomy, a >> 3);
            s.on.0 + margin + b % (s.off.0 - s.on.0 - 2 * margin)
        }
        0 | 1 if !spans.is_empty() => {
            let s = pick(spans, a >> 3);
            s.on.0 + b % (s.off.0 - s.on.0)
        }
        2 | 3 if !bs.is_empty() => {
            pick(&bs, a >> 3).saturating_add_signed(signed(b, ms(t.rate, 25)))
        }
        4 if !earlier.is_empty() => pick(&earlier, a >> 3),
        _ => any_pos(t, b),
    }
}

/// One edit, aimed at the track it will be applied to.
fn make_edit(t: &TrackParts, p: &EditPlan, prior: &[Edit]) -> Edit {
    const ST: [f32; 7] = [-64.0, 192.0, 60.0, 47.5, -0.0, 300.0, -100.0];
    let r = t.rate;
    let EditPlan { kind, a, b, c } = *p;
    match kind {
        0 => {
            let start = any_pos(t, a);
            let len = match c % 4 {
                0 => 0,
                1 => b % ms(r, 50),
                2 => b % (t.len / 2 + 1),
                _ => u64::MAX - start,
            };
            Edit::ForcePitch {
                span: tspan(start, start.saturating_add(len)),
                st: ST[((c >> 8) % ST.len() as u64) as usize],
            }
        }
        1 => Edit::SplitAt {
            at: TimelinePos(split_pos(t, a, b, prior)),
            articulation: articulation(c),
        },
        2 => {
            let js = junctions(&t.spans);
            // Junctions between two spans over 40 ms long: no other junction
            // competes inside the tolerance.
            let long = |i: usize| t.spans[i].off.0 - t.spans[i].on.0 > ms(r, 40);
            let lone: Vec<(u64, u64)> = (1..t.spans.len())
                .filter(|&i| long(i - 1) && long(i))
                .map(|i| js[i - 1])
                .collect();
            let at = if js.is_empty() || a.is_multiple_of(8) {
                any_pos(t, b)
            } else {
                let from = if lone.is_empty() || (a >> 3).is_multiple_of(4) {
                    &js
                } else {
                    &lone
                };
                let (x, y) = pick(from, a >> 5);
                let base = if c.is_multiple_of(2) { x } else { y };
                base.saturating_add_signed(signed(b, ms(r, 5)))
            };
            let tol = match (c >> 4) % 6 {
                0 => 0,
                1 => ms(r, 5),
                _ => u64::from(default_tol(r)),
            };
            Edit::JoinAt {
                at: TimelinePos(at),
                tol: tol as u32,
            }
        }
        3 => Edit::ForceRole {
            hit: anchor(hit_pos(t, a, b), tol_for(t, c)),
            role: pool_role(c >> 8),
        },
        4 => Edit::DeleteHit {
            hit: anchor(hit_pos(t, a, b), tol_for(t, c)),
        },
        5 => Edit::AddHit {
            hit: anchor(hit_pos(t, a, b), tol_for(t, c)),
            dur: match (c >> 16) % 4 {
                0 => 0,
                1 => 1 + b % ms(r, 300),
                2 => u64::MAX,
                _ => ms(r, MAX_HIT_MS) + 1,
            },
            role: pool_role(c >> 8),
            vel: vel(1 + ((c >> 24) % 127) as u8),
        },
        _ => {
            // Near a hit or a span edge, but mostly 6 to 10 ms clear of it, so
            // the re-derivation jitter cannot carry it across the mute's edge.
            let clear = ms(r, 8) as i64;
            let start = match a % 3 {
                1 => hit_pos(t, a >> 2, b).saturating_add_signed(-clear),
                2 if !t.spans.is_empty() => {
                    let s = pick(&t.spans, a >> 2);
                    let edge = if b.is_multiple_of(2) { s.on.0 } else { s.off.0 };
                    let side = if (b >> 1).is_multiple_of(2) {
                        clear
                    } else {
                        -clear
                    };
                    edge.saturating_add_signed(side + signed(b >> 2, ms(r, 2)))
                }
                _ => any_pos(t, b),
            };
            let len = match c % 4 {
                0 => 0,
                1 => (b >> 8) % ms(r, 60),
                2 => (b >> 8) % (t.len / 4 + 1),
                _ => t.len,
            };
            Edit::MuteSpan {
                span: tspan(start, start.saturating_add(len)),
            }
        }
    }
}

/// Edits made one at a time against the track as the earlier ones left it.
fn build_edits(t: &TrackParts, plans: &[EditPlan]) -> Vec<Edit> {
    let mut state = canonical(t.clone());
    let mut edits = Vec::new();
    for p in plans {
        let e = make_edit(&state, p, &edits);
        state = run1(&state, &e).0;
        edits.push(e);
    }
    edits
}

// ---------------------------------------------------------------------------
// Re-derivation
// ---------------------------------------------------------------------------

/// Unique per-hit tags in (bright, breath), so a hit can be followed through a
/// re-derivation. Tags have breath >= 1; an added hit has breath 0.
fn tag_hits(p: &mut TrackParts) {
    for (i, h) in p.hits.iter_mut().enumerate() {
        let k = i + 1;
        h.bright = u7((k % 128) as u8);
        h.breath = u7(1 + (k / 128) as u8);
    }
}

fn tag(h: &HitEvent) -> (u8, u8) {
    (h.bright.get(), h.breath.get())
}

fn is_added(h: &HitEvent) -> bool {
    h.breath.get() == 0
}

/// Second candidates 15 to 20 ms from existing hits: two hits inside one
/// tolerance window.
fn add_decoys(p: &mut TrackParts, raws: &[u64]) {
    if p.hits.is_empty() {
        return;
    }
    let (r, len) = (p.rate, p.len);
    let mut extra = Vec::new();
    for &raw in raws {
        let h = pick(&p.hits, raw);
        let d = ms(r, 15) + (raw >> 8) % (ms(r, 5) + 1);
        let at = if (raw >> 40) & 1 == 0 {
            h.anchor.0.saturating_sub(d)
        } else {
            h.anchor.0 + d
        }
        .min(len - 1);
        extra.push(HitEvent {
            at: TimelinePos(at),
            anchor: TimelinePos(at),
            dur: (1 + (raw >> 16) % ms(r, 80)).min(len - at),
            ..h
        });
    }
    p.hits.extend(extra);
}

/// The same take re-derived: every span edge, span anchor and hit anchor
/// (with its `at`) jittered by up to `j` samples, and a different pitch curve.
/// Abutting spans stay abutting and gaps stay gaps; hits sharing an anchor
/// move together, so their relative order holds.
fn rederive(t: &TrackParts, salt: u64, j: u64) -> TrackParts {
    let len = t.len;
    let mut out = t.clone();
    let edges: Vec<u64> = t
        .spans
        .iter()
        .flat_map(|g| [g.on.0, g.off.0])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut moved = BTreeMap::new();
    for (k, &p) in edges.iter().enumerate() {
        // Each edge moves at most half-way to its neighbours, so edges stay
        // strictly increasing.
        let down = if k == 0 {
            p
        } else {
            (p - edges[k - 1] - 1) / 2
        }
        .min(j);
        let up = if k + 1 == edges.len() {
            len - p
        } else {
            (edges[k + 1] - p - 1) / 2
        }
        .min(j);
        let r = splitmix(salt ^ p) % (down + up + 1);
        moved.insert(p, p - down + r);
    }
    for g in &mut out.spans {
        g.on = TimelinePos(moved[&g.on.0]);
        g.off = TimelinePos(moved[&g.off.0]);
        let d = signed(splitmix(salt ^ g.anchor.0 ^ 0xA5A5_A5A5), j);
        g.anchor = TimelinePos(g.anchor.0.saturating_add_signed(d).min(len));
    }
    let mut room: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    for h in &t.hits {
        let down = h.anchor.0.min(h.at.0).min(j);
        let up = (len - h.anchor.0).min(len - h.at.0 - h.dur).min(j);
        room.entry(h.anchor.0)
            .and_modify(|(d, u)| {
                *d = (*d).min(down);
                *u = (*u).min(up);
            })
            .or_insert((down, up));
    }
    let delta: BTreeMap<u64, i64> = room
        .iter()
        .map(|(&a, &(d, u))| {
            let r = splitmix(salt.rotate_left(17) ^ a) % (d + u + 1);
            (a, r as i64 - d as i64)
        })
        .collect();
    for h in &mut out.hits {
        let d = delta[&h.anchor.0];
        h.anchor = TimelinePos(h.anchor.0.saturating_add_signed(d));
        h.at = TimelinePos(h.at.0.saturating_add_signed(d));
    }
    for (f, v) in out.curves.pitch_st.iter_mut().enumerate() {
        *v = 36.0 + (splitmix(salt ^ (f as u64 / 5)) % 48) as f32 + 0.25;
    }
    canonical(out)
}

/// Whether "the nearest candidate within `tol`, the earlier on a tie" picks
/// the same candidate (or none) when each candidate's distance may move by up
/// to its jitter. `cands` are (distance, jitter) in tie-break order.
fn choice_is_robust(cands: &[(u64, u64)], tol: u64) -> bool {
    let possible: Vec<usize> = (0..cands.len())
        .filter(|&k| cands[k].0 <= tol.saturating_add(cands[k].1))
        .collect();
    let Some(&best) = possible.iter().min_by_key(|&&k| (cands[k].0, k)) else {
        return true;
    };
    let (db, jb) = cands[best];
    if db + jb > tol {
        return false;
    }
    possible.iter().filter(|&&k| k != best).all(|&k| {
        let (d, jk) = cands[k];
        d.saturating_sub(jk) > db + jb || (jk == 0 && jb == 0)
    })
}

fn nearest_within(cands: &[(u64, u64)], tol: u64) -> Option<usize> {
    (0..cands.len())
        .filter(|&k| cands[k].0 <= tol)
        .min_by_key(|&k| (cands[k].0, k))
}

fn hit_jitter(h: &HitEvent, j: u64) -> u64 {
    if is_added(h) { 0 } else { j }
}

/// Whether edit `e` meets the same targets, with the same outcome, on every
/// re-derivation that moves each non-exact position by up to `j`. Exact
/// positions are those an earlier edit placed: span edges in `exact`, and the
/// anchors and `at` of added hits.
fn robust(t: &TrackParts, e: &Edit, j: u64, exact: &BTreeSet<u64>) -> bool {
    let je = |p: u64| if exact.contains(&p) { 0 } else { j };
    match e {
        Edit::ForcePitch { .. } => true,
        Edit::ForceRole { hit, .. } | Edit::DeleteHit { hit } | Edit::AddHit { hit, .. } => {
            let mut values: BTreeMap<u64, u64> = BTreeMap::new();
            for h in &t.hits {
                let jh = hit_jitter(h, j);
                values
                    .entry(h.anchor.0)
                    .and_modify(|v| *v = (*v).max(jh))
                    .or_insert(jh);
            }
            let cands: Vec<(u64, u64)> = values
                .iter()
                .map(|(&v, &jh)| (v.abs_diff(hit.at.0), jh))
                .collect();
            choice_is_robust(&cands, u64::from(hit.tol))
        }
        Edit::JoinAt { at, tol } => {
            let cands: Vec<(u64, u64)> = junctions(&t.spans)
                .iter()
                .map(|&(x, y)| (dist_to(at.0, x, y), je(x).max(je(y))))
                .collect();
            choice_is_robust(&cands, u64::from(*tol))
        }
        Edit::SplitAt { at, .. } => {
            let (s, at) = (&t.spans, at.0);
            let clear = |p: u64| je(p) == 0 || at.abs_diff(p) > je(p);
            // Coverage: only edges that border silence decide it.
            for (i, g) in s.iter().enumerate() {
                let opens = i == 0 || s[i - 1].off < g.on;
                let closes = i + 1 == s.len() || s[i + 1].on > g.off;
                if (opens && !clear(g.on.0)) || (closes && !clear(g.off.0)) {
                    return false;
                }
            }
            let Some(c) = gate_at(s, at) else {
                return true;
            };
            let snap = ms(t.rate, SPLIT_SNAP_MS);
            let idx: Vec<usize> = (1..s.len()).filter(|&i| s[i - 1].off == s[i].on).collect();
            let cands: Vec<(u64, u64)> = idx
                .iter()
                .map(|&i| (s[i].on.0.abs_diff(at), je(s[i].on.0)))
                .collect();
            if !choice_is_robust(&cands, snap) {
                return false;
            }
            let (first, second) = match nearest_within(&cands, snap) {
                Some(k) => (idx[k] - 1, idx[k]),
                None => (c, c),
            };
            let min = i128::from(ms(t.rate, MIN_PIECE_MS));
            let piece_ok = |len: i128, jp: u64| {
                let jp = i128::from(jp);
                jp == 0 || len - jp >= min || len + jp < min
            };
            let (start, end) = (s[first].on.0, s[second].off.0);
            piece_ok(i128::from(at) - i128::from(start), je(start))
                && piece_ok(i128::from(end) - i128::from(at), je(end))
        }
        Edit::MuteSpan { span } => {
            if span.is_empty() || span.start().0 >= t.len {
                return true;
            }
            let (s, e) = (span.start().0, span.end().0);
            let clear = |p: u64, jp: u64| jp == 0 || (p.abs_diff(s) > jp && p.abs_diff(e) > jp);
            t.spans
                .iter()
                .all(|g| clear(g.on.0, je(g.on.0)) && clear(g.off.0, je(g.off.0)))
                && t.hits.iter().all(|h| {
                    let jh = hit_jitter(h, j);
                    clear(h.anchor.0, jh) && clear(h.at.0, jh)
                })
        }
    }
}

/// Records the span edges an applied edit placed exactly.
fn note_exact(e: &Edit, o: Outcome, exact: &mut BTreeSet<u64>) {
    match (e, o) {
        (Edit::SplitAt { at, .. }, Outcome::Applied) => {
            exact.insert(at.0);
        }
        (Edit::MuteSpan { span }, Outcome::Applied) => {
            exact.insert(span.start().0);
            exact.insert(span.end().0);
        }
        _ => {}
    }
}

/// Derived hits by tag, and added hits.
fn split_hits(p: &TrackParts) -> (BTreeMap<(u8, u8), Named>, Vec<Named>) {
    let mut derived = BTreeMap::new();
    let mut added = Vec::new();
    for h in &p.hits {
        let n = named_hit(p, h);
        if is_added(h) {
            added.push(n);
        } else {
            assert!(derived.insert(tag(h), n).is_none(), "tags are unique");
        }
    }
    added.sort();
    (derived, added)
}

/// The two results agree up to the jitter: same structure, every derived
/// position within `j`, every position an edit placed exactly equal, and the
/// pitch frames ForcePitch wrote bit for bit.
fn agree(
    r0: &TrackParts,
    r1: &TrackParts,
    edits: &[Edit],
    outcomes: &[Outcome],
    j: u64,
    exact: &BTreeSet<u64>,
) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        (r0.rate, r0.len, r0.take, &r0.roles),
        (r1.rate, r1.len, r1.take, &r1.roles)
    );
    prop_assert_eq!(&r0.curves.amp_db, &r1.curves.amp_db);
    prop_assert_eq!(&r0.curves.bright, &r1.curves.bright);
    prop_assert_eq!(&r0.curves.breath, &r1.curves.breath);
    prop_assert_eq!(&r0.curves.vowel_x, &r1.curves.vowel_x);
    prop_assert_eq!(&r0.curves.vowel_y, &r1.curves.vowel_y);
    let frames = r0.curves.pitch_st.len() as u64;
    let mut forced = vec![false; frames as usize];
    for (e, o) in edits.iter().zip(outcomes) {
        if let (Edit::ForcePitch { span, .. }, Outcome::Applied) = (e, o) {
            let f = covered_frames(span.start(), span.end(), r0.rate);
            for k in f.start.min(frames)..f.end.min(frames) {
                forced[k as usize] = true;
            }
        }
    }
    for (k, _) in forced.iter().enumerate().filter(|(_, f)| **f) {
        prop_assert_eq!(
            r0.curves.pitch_st[k].to_bits(),
            r1.curves.pitch_st[k].to_bits()
        );
    }
    prop_assert_eq!(r0.spans.len(), r1.spans.len());
    for (a, b) in r0.spans.iter().zip(&r1.spans) {
        prop_assert_eq!((a.vel, a.link), (b.vel, b.link));
        for (x, y) in [(a.on.0, b.on.0), (a.off.0, b.off.0)] {
            if exact.contains(&x) {
                prop_assert_eq!(x, y, "a SplitAt or MuteSpan edge moved");
            } else {
                prop_assert!(x.abs_diff(y) <= j, "edge {x} vs {y}");
            }
        }
        prop_assert!(a.anchor.0.abs_diff(b.anchor.0) <= j);
    }
    let (d0, a0) = split_hits(r0);
    let (d1, a1) = split_hits(r1);
    prop_assert_eq!(&a0, &a1, "added hits agree exactly");
    prop_assert_eq!(d0.keys().collect::<Vec<_>>(), d1.keys().collect::<Vec<_>>());
    for (k, x) in &d0 {
        let y = &d1[k];
        prop_assert_eq!((&x.3, x.4), (&y.3, y.4), "role and velocity");
        prop_assert!(x.0.abs_diff(y.0) <= j, "at");
        prop_assert!(x.1.abs_diff(y.1) <= j, "anchor");
        prop_assert!(x.2.abs_diff(y.2) <= j, "dur");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §3.5
// ---------------------------------------------------------------------------

/// Every edit has the same outcome on a re-derivation of the same take, the
/// results agree up to the jitter, and SplitAt boundaries agree exactly.
/// Edits whose outcome the jitter could flip (a target within 5 ms of the
/// tolerance edge, two candidates within 10 ms of equidistant) are left out,
/// and counted so the test cannot pass vacuously.
#[test]
fn edits_survive_rederivation() {
    let planned = Cell::new(0u64);
    let kept: [Cell<u64>; 7] = std::array::from_fn(|_| Cell::new(0));
    let landed: [Cell<u64>; 7] = std::array::from_fn(|_| Cell::new(0));
    let mut runner = runner();
    let strategy = std_rate().prop_flat_map(|r| {
        (
            track_parts(r),
            vec(any::<u64>(), 0..4),
            vec(edit_plan(), 0..=10),
            any::<u64>(),
        )
    });
    runner
        .run(&strategy, |(parts, decoys, plans, salt)| {
            let j = ms(parts.rate, JITTER_MS);
            let mut base = parts;
            add_decoys(&mut base, &decoys);
            tag_hits(&mut base);
            let base = canonical(base);

            let mut state = base.clone();
            let mut edits = Vec::new();
            let mut outcomes = Vec::new();
            let mut exact = BTreeSet::new();
            for p in &plans {
                planned.set(planned.get() + 1);
                let e = make_edit(&state, p, &edits);
                if !robust(&state, &e, j, &exact) {
                    continue;
                }
                let (next, o) = run1(&state, &e);
                let k = kind_of(&e);
                kept[k].set(kept[k].get() + 1);
                if !matches!(o, Outcome::Dormant(_)) {
                    landed[k].set(landed[k].get() + 1);
                }
                note_exact(&e, o, &mut exact);
                state = next;
                edits.push(e);
                outcomes.push(o);
            }
            // Applying the list at once is the fold of applying each edit.
            let (r0, o0) = run(&base, &edits);
            prop_assert_eq!(&r0, &state);
            prop_assert_eq!(&o0, &outcomes);

            let again = rederive(&base, salt, j);
            let (r1, o1) = run(&again, &edits);
            prop_assert_eq!(&o0, &o1, "edits {:?}", edits);
            agree(&r0, &r1, &edits, &o0, j, &exact)
        })
        .unwrap();
    let total: u64 = kept.iter().map(Cell::get).sum();
    eprintln!(
        "edits_survive_rederivation: {} planned, {total} kept",
        planned.get()
    );
    for (k, name) in KIND_NAMES.iter().enumerate() {
        let (kept, landed) = (kept[k].get(), landed[k].get());
        eprintln!("  {name:<12} kept {kept:>5}  applied/merged/no-op {landed:>5}");
    }
    for (k, name) in KIND_NAMES.iter().enumerate() {
        assert!(
            landed[k].get() >= 25,
            "{name} rarely lands; the test is mostly vacuous"
        );
    }
    assert!(2 * total >= planned.get(), "most edits are filtered out");
}

/// Whether a hit at `[at, at + dur)` is out of every edit's reach.
fn unrelated(edits: &[Edit], at: u64, dur: u64) -> bool {
    edits.iter().all(|e| match e {
        Edit::ForceRole { hit, .. } | Edit::DeleteHit { hit } | Edit::AddHit { hit, .. } => {
            at.abs_diff(hit.at.0) > u64::from(hit.tol)
        }
        Edit::MuteSpan { span } => {
            span.is_empty() || at + dur <= span.start().0 || at >= span.end().0
        }
        Edit::ForcePitch { .. } | Edit::SplitAt { .. } | Edit::JoinAt { .. } => true,
    })
}

proptest! {
    #![proptest_config(Config::with_cases(CASES))]

    /// Hits inserted anywhere in the draft, out of every edit's reach, change
    /// no outcome and no edited hit.
    #[test]
    fn edits_ignore_index_shifts(
        (parts, plans, extra) in std_rate().prop_flat_map(|r| {
            (
                track_parts(r),
                vec(edit_plan(), 0..=10),
                vec((any::<u64>(), any::<u64>(), any::<u64>()), 1..=8),
            )
        }),
    ) {
        let r = parts.rate;
        let edits = build_edits(&parts, &plans);
        let (ra, oa) = run(&parts, &edits);

        let mut b = parts.clone();
        if b.roles.is_empty() {
            b.roles.push(role("perc"));
        }
        let mut inserted = Vec::new();
        for &(x, y, z) in &extra {
            let at = x % b.len;
            let dur = (1 + y % ms(r, 120)).min(b.len - at);
            if !unrelated(&edits, at, dur) {
                continue;
            }
            let h = HitEvent {
                at: TimelinePos(at),
                anchor: TimelinePos(at),
                dur,
                role: (z % b.roles.len() as u64) as u8,
                vel: vel(1 + ((z >> 8) % 127) as u8),
                bright: u7(((z >> 16) % 128) as u8),
                breath: u7(((z >> 24) % 128) as u8),
                mouth: u7(((z >> 32) % 128) as u8),
            };
            inserted.push(named_hit(&b, &h));
            let idx = ((z >> 40) % (b.hits.len() as u64 + 1)) as usize;
            b.hits.insert(idx, h);
        }
        let (rb, ob) = run(&b, &edits);
        prop_assert_eq!(&oa, &ob);
        prop_assert_eq!(&ra.spans, &rb.spans);
        prop_assert_eq!(&ra.curves, &rb.curves);
        let mut expect = named(&ra);
        expect.extend(inserted);
        expect.sort();
        prop_assert_eq!(named(&rb), expect);
    }

    /// Splitting twice equals splitting once; a split within 20 ms of a
    /// boundary moves it; a piece under 10 ms is `TooShort`; a split in
    /// silence is `NoTarget` (J2 regression on C).
    #[test]
    fn split_semantics(
        (parts, how, raw, art) in std_rate().prop_flat_map(|r| {
            (track_parts(r), 0u8..6, any::<u64>(), any::<u64>())
        }),
    ) {
        let (r, len) = (parts.rate, parts.len);
        let spans = parts.spans.clone();
        let bs = boundaries(&spans);
        let at = match how {
            0 if !spans.is_empty() => {
                let s = pick(&spans, raw >> 32);
                s.on.0 + raw % (s.off.0 - s.on.0)
            }
            1 if !bs.is_empty() => pick(&bs, raw >> 32).saturating_add_signed(signed(raw, ms(r, 25))),
            2 if !bs.is_empty() => pick(&bs, raw),
            3 => {
                // Silence: inside a gap, before the first span or after the
                // last, or past the end.
                let mut gaps = Vec::new();
                let mut prev = 0;
                for s in &spans {
                    if s.on.0 > prev {
                        gaps.push((prev, s.on.0));
                    }
                    prev = s.off.0;
                }
                gaps.push((prev, len + 100));
                let (a, b) = pick(&gaps, raw >> 32);
                a + raw % (b - a)
            }
            4 if !spans.is_empty() => {
                let s = pick(&spans, raw >> 32);
                let edge = if raw.is_multiple_of(2) { s.on.0 } else { s.off.0 };
                edge.saturating_add_signed(signed(raw >> 1, ms(r, 15)))
            }
            _ => any_pos(&parts, raw),
        };
        let articulation = articulation(art);
        let edit = Edit::SplitAt { at: TimelinePos(at), articulation };

        // The oracle, straight from §2.10.
        let snap = ms(r, SPLIT_SNAP_MS);
        let min = ms(r, MIN_PIECE_MS);
        let nearest = (1..spans.len())
            .filter(|&i| spans[i - 1].off == spans[i].on)
            .map(|i| (spans[i].on.0.abs_diff(at), i))
            .filter(|&(d, _)| d <= snap)
            .min()
            .map(|(_, i)| i);
        let expect = match gate_at(&spans, at) {
            None => Outcome::Dormant(Dormancy::NoTarget),
            Some(c) => {
                let (first, second) = nearest.map_or((c, c), |i| (i - 1, i));
                let (start, end) = (spans[first].on.0, spans[second].off.0);
                if at.saturating_sub(start) < min || end.saturating_sub(at) < min {
                    Outcome::Dormant(Dormancy::TooShort)
                } else {
                    Outcome::Applied
                }
            }
        };
        if how == 3 {
            prop_assert_eq!(expect, Outcome::Dormant(Dormancy::NoTarget));
        }

        let (once, o1) = run1(&parts, &edit);
        prop_assert_eq!(o1, expect);
        let (twice, o2) = run(&parts, &[edit.clone(), edit]);
        prop_assert_eq!(&twice, &once, "splitting twice equals splitting once");
        prop_assert_eq!(o2, vec![o1, o1]);

        let after = &once.spans;
        if o1 == Outcome::Applied {
            // The gate is unchanged; only a boundary appears or moves.
            prop_assert_eq!(gate_union(after), gate_union(&spans));
            let i = after.iter().position(|s| s.on.0 == at).expect("a span starts at the split");
            prop_assert!(i > 0);
            prop_assert_eq!(after[i - 1].off.0, at);
            prop_assert_eq!(after[i].link, link_of(articulation));
            if let Some(b) = nearest {
                // A boundary within 20 ms moved to `at`.
                prop_assert_eq!(after.len(), spans.len());
                let old = spans[b].on.0;
                prop_assert!(old == at || !boundaries(after).contains(&old));
                prop_assert_eq!(after[i].anchor, spans[b].anchor);
            } else {
                prop_assert_eq!(after.len(), spans.len() + 1);
                prop_assert_eq!(after[i].anchor.0, at, "the new span anchors at the split");
                prop_assert!(at - after[i - 1].on.0 >= min && after[i].off.0 - at >= min);
            }
        } else {
            prop_assert_eq!(&once, &canonical(parts.clone()));
        }
    }

    /// An edit whose target vanishes is reported dormant and kept; once a
    /// target is back within tolerance it applies again.
    #[test]
    fn dormant_edit_resurrects(
        (parts, kind, x_raw, d_raw, e_raw) in std_rate().prop_flat_map(|r| {
            (
                track_parts(r).prop_filter("room for the scenario", move |p| {
                    p.len >= r.samples_for_ms(300)
                }),
                0u8..7,
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
            )
        }),
    ) {
        let (r, len) = (parts.rate, parts.len);
        let tol = default_tol(r);
        let tol64 = u64::from(tol);
        let min = ms(r, MIN_PIECE_MS);
        let applied = Outcome::Applied;
        let (edit, derivations, expected): (Edit, [TrackParts; 3], [Outcome; 3]) = match kind {
            0 | 1 => {
                let x = x_raw % len;
                let mut gone = parts.clone();
                gone.hits.retain(|h| h.anchor.0.abs_diff(x) > 2 * tol64);
                let forced = pool_role(e_raw);
                let detected = pool_role(e_raw + 1);
                let with_hit = |at: u64| {
                    let mut p = gone.clone();
                    let k = ensure_role(&mut p, &detected);
                    p.hits.push(hit(at, (1 + d_raw % ms(r, 100)).min(len - at), k));
                    canonical(p)
                };
                let back = x.saturating_add_signed(signed(d_raw >> 8, tol64)).min(len - 1);
                let edit = if kind == 0 {
                    Edit::ForceRole { hit: anchor(x, tol), role: forced }
                } else {
                    Edit::DeleteHit { hit: anchor(x, tol) }
                };
                (edit, [with_hit(x), gone.clone(), with_hit(back)], [applied, Outcome::Dormant(Dormancy::NoTarget), applied])
            }
            2 => {
                let x = min + x_raw % (len - 2 * min);
                let mut gone = parts.clone();
                gone.spans.clear();
                let around = |raw: u64| {
                    let mut p = gone.clone();
                    let before = min + raw % (x - min + 1);
                    let after = min + (raw >> 32) % (len - x - min + 1);
                    p.spans.push(gspan(x - before, x + after, Link::Detached));
                    p
                };
                (
                    Edit::SplitAt { at: TimelinePos(x), articulation: articulation(e_raw) },
                    [around(d_raw), gone.clone(), around(splitmix(d_raw))],
                    [applied, Outcome::Dormant(Dormancy::NoTarget), applied],
                )
            }
            3 => {
                let gap = if e_raw.is_multiple_of(2) { 0 } else { 1 + (e_raw >> 1) % ms(r, 10) };
                let x = tol64 + 1 + x_raw % (len - 2 * tol64 - gap - 2);
                let mut gone = parts.clone();
                gone.spans = vec![gspan(0, len, Link::Detached)];
                let pair = |a: u64| {
                    let mut p = gone.clone();
                    let link = if gap == 0 { Link::Retrigger } else { Link::Detached };
                    p.spans = vec![gspan(0, a, Link::Detached), gspan(a + gap, len, link)];
                    p
                };
                let back = x.saturating_add_signed(signed(d_raw, tol64));
                (
                    Edit::JoinAt { at: TimelinePos(x), tol },
                    [pair(x), gone.clone(), pair(back)],
                    [applied, Outcome::Dormant(Dormancy::NoTarget), applied],
                )
            }
            _ => {
                // Time edits go dormant only when the track ends before them.
                let x = ms(r, 100) + x_raw % ms(r, 100);
                let l = 1 + d_raw % ms(r, 50);
                let edit = match kind {
                    4 => Edit::AddHit {
                        hit: anchor(x, tol),
                        dur: l,
                        role: pool_role(e_raw),
                        vel: vel(64),
                    },
                    5 => Edit::MuteSpan { span: tspan(x, x + l) },
                    _ => Edit::ForcePitch { span: tspan(x, x + l), st: 72.0 },
                };
                let longer = |extra: u64| {
                    let mut p = blank(r, x + 1 + extra);
                    p.spans.push(gspan(0, p.len, Link::Detached));
                    p
                };
                (
                    edit,
                    [longer(e_raw % ms(r, 500)), blank(r, x - (e_raw >> 8) % x), longer(l)],
                    [applied, Outcome::Dormant(Dormancy::OutOfRange), applied],
                )
            }
        };
        for (draft, want) in derivations.iter().zip(expected) {
            let (after, got) = run1(draft, &edit);
            prop_assert_eq!(got, want, "{:?}", edit);
            match (&edit, got) {
                (Edit::ForceRole { hit: a, role: name }, Outcome::Applied) => {
                    let near: Vec<&HitEvent> = after
                        .hits
                        .iter()
                        .filter(|h| h.anchor.0.abs_diff(a.at.0) <= u64::from(a.tol))
                        .collect();
                    prop_assert_eq!(near.len(), 1);
                    prop_assert_eq!(role_of(&after, near[0]), name);
                }
                (Edit::DeleteHit { hit: a }, Outcome::Applied) => {
                    prop_assert!(after.hits.iter().all(|h| h.anchor.0.abs_diff(a.at.0) > u64::from(a.tol)));
                }
                (Edit::SplitAt { at, .. }, Outcome::Applied) => {
                    prop_assert!(boundaries(&after.spans).contains(&at.0));
                }
                (Edit::JoinAt { .. }, Outcome::Applied) => {
                    prop_assert!(after.spans == [gspan(0, len, Link::Detached)], "{:?}", after.spans);
                }
                (_, Outcome::Dormant(_)) => prop_assert_eq!(&after, &canonical(draft.clone())),
                _ => {}
            }
        }
    }

    /// After MuteSpan no span, hit anchor or hit tail remains inside the
    /// span, and nothing outside it changes.
    #[test]
    fn mute_span_silences(
        (parts, how, s_raw, l_raw) in std_rate().prop_flat_map(|r| {
            (track_parts(r), 0u8..6, any::<u64>(), any::<u64>())
        }),
    ) {
        let (r, len) = (parts.rate, parts.len);
        let start = match how {
            0 | 1 => s_raw % len,
            2 if !parts.hits.is_empty() => {
                pick(&parts.hits, s_raw >> 8).anchor.0.saturating_add_signed(signed(s_raw, ms(r, 10)))
            }
            3 if !parts.spans.is_empty() => {
                let g = pick(&parts.spans, s_raw >> 8);
                let edge = if s_raw.is_multiple_of(2) { g.on.0 } else { g.off.0 };
                edge.saturating_add_signed(signed(s_raw >> 1, ms(r, 10)))
            }
            4 => len + s_raw % 1_000,
            _ => 0,
        };
        let l = match how {
            1 => 0,
            5 => len + l_raw % 1_000,
            _ => l_raw % (len / 3 + 2),
        };
        let span = tspan(start, start + l);
        let (s, e) = (span.start().0, span.end().0);
        let (after, o) = run1(&parts, &Edit::MuteSpan { span });
        if span.is_empty() || s >= len {
            prop_assert_eq!(o, Outcome::Dormant(Dormancy::OutOfRange));
            prop_assert_eq!(&after, &canonical(parts));
            return Ok(());
        }
        prop_assert_eq!(o, Outcome::Applied);
        for g in &after.spans {
            prop_assert!(g.off.0 <= s || g.on.0 >= e, "a span sounds inside the mute");
        }
        for h in &after.hits {
            prop_assert!(!span.contains(h.anchor), "a hit is anchored inside the mute");
            prop_assert!(!span.contains(h.at), "a hit fires inside the mute");
            prop_assert!(h.at.0 >= e || h.at.0 + h.dur <= s, "a hit tail rings into the mute");
        }
        // Outside the span the gate and the hits are as they were.
        prop_assert_eq!(gate_union(&after.spans), subtract(&gate_union(&parts.spans), s, e));
        let mut expect: Vec<Named> = parts
            .hits
            .iter()
            .filter(|h| !span.contains(h.anchor) && !span.contains(h.at))
            .map(|h| {
                let mut n = named_hit(&parts, h);
                if h.at.0 < s {
                    n.2 = n.2.min(s - h.at.0);
                }
                n
            })
            .collect();
        expect.sort();
        prop_assert_eq!(named(&after), expect);
        prop_assert_eq!(&after.curves, &parts.curves);
    }

    /// Rebasing from a to b >= a and back gives the original edits.
    #[test]
    fn rebase_up_and_down_is_identity(
        (edit, (a, b)) in (any_edit(), rate_pair()),
    ) {
        let (a, b) = (a.min(b), a.max(b));
        prop_assert_eq!(&edit.rebased(a, b).rebased(b, a), &edit);
        prop_assert_eq!(&edit.rebased(a, a), &edit);
    }
}

fn rate_pair() -> impl Strategy<Value = (SampleRate, SampleRate)> {
    prop_oneof![
        (std_rate(), std_rate()),
        (any_rate(), any_rate()),
        Just((rate(44_100), rate(48_000))),
    ]
}

/// Edits with positions up to 2^40 and tolerances small enough that scaling
/// up to 1.536 MHz cannot saturate.
fn any_edit() -> impl Strategy<Value = Edit> {
    let p = || prop_oneof![0u64..1_000, 0u64..=(1u64 << 40)];
    let tol = || prop_oneof![Just(0u32), 0u32..2_000, 0u32..=(u32::MAX / 1_536)];
    let span = move || (p(), p()).prop_map(|(x, y)| tspan(x.min(y), x.max(y)));
    let hit = move || (p(), tol()).prop_map(|(at, tol)| anchor(at, tol));
    let name = || any::<u64>().prop_map(pool_role);
    prop_oneof![
        (span(), -1_000.0f32..1_000.0).prop_map(|(span, st)| Edit::ForcePitch { span, st }),
        (p(), any::<u64>()).prop_map(|(at, a)| Edit::SplitAt {
            at: TimelinePos(at),
            articulation: articulation(a),
        }),
        (p(), tol()).prop_map(|(at, tol)| Edit::JoinAt {
            at: TimelinePos(at),
            tol
        }),
        (hit(), name()).prop_map(|(hit, role)| Edit::ForceRole { hit, role }),
        hit().prop_map(|hit| Edit::DeleteHit { hit }),
        (
            hit(),
            prop_oneof![1u64..10, 1u64..=(1u64 << 40)],
            name(),
            1u8..=127
        )
            .prop_map(|(hit, dur, role, v)| Edit::AddHit {
                hit,
                dur,
                role,
                vel: vel(v)
            }),
        span().prop_map(|span| Edit::MuteSpan { span }),
    ]
}

/// One check for AddHit against a re-detected hit: on the original derivation
/// (no hit near `x`) it inserts; on the re-derivation, with a hit detected at
/// `y`, 1 to 20 ms away and with any role, it merges, leaving exactly one hit
/// near `x`, carrying the edit's role.
fn check_add_hit_merge(
    base: &TrackParts,
    x: u64,
    y: u64,
    detected: &RoleName,
    forced: &RoleName,
    salt: u64,
) -> Result<(), TestCaseError> {
    let r = base.rate;
    let tol = default_tol(r);
    let near = |p: &TrackParts| -> Vec<HitEvent> {
        p.hits
            .iter()
            .filter(|h| h.anchor.0.abs_diff(x) <= u64::from(tol))
            .copied()
            .collect()
    };
    let edit = Edit::AddHit {
        hit: anchor(x, tol),
        dur: ms(r, 120),
        role: forced.clone(),
        vel: vel(100),
    };

    let (first, o) = run1(base, &edit);
    prop_assert_eq!(o, Outcome::Applied);
    let one = near(&first);
    prop_assert_eq!(one.len(), 1);
    prop_assert_eq!((one[0].at.0, one[0].anchor.0), (x, x));
    prop_assert_eq!(role_of(&first, &one[0]), forced);

    let mut again = if salt == 0 {
        base.clone()
    } else {
        rederive(base, salt, ms(r, JITTER_MS))
    };
    let k = ensure_role(&mut again, detected);
    again.hits.push(hit(y, ms(r, 80).min(again.len - y), k));
    let (second, o) = run1(&again, &edit);
    let want = if detected == forced {
        Outcome::NoOp
    } else {
        Outcome::Merged
    };
    prop_assert_eq!(o, want);
    let one = near(&second);
    prop_assert_eq!(one.len(), 1, "a flam: the AddHit doubled a re-detected hit");
    prop_assert_eq!(one[0].anchor.0, y, "the re-detected hit is kept");
    prop_assert_eq!(role_of(&second, &one[0]), forced);
    Ok(())
}

/// AddHit plus a re-detected hit 1 to 20 ms away with any role: exactly one
/// hit near the anchor, carrying the edit's role; `Merged` (or `NoOp` when
/// the detector already chose that role). Includes J2's 48000/48144 case.
#[test]
fn add_hit_merges_with_rederived_hit() {
    // J2: an AddHit at 48000 and the same onset re-detected at 48144 (3 ms).
    let r = rate(48_000);
    let base = blank(r, 96_000);
    check_add_hit_merge(&base, 48_000, 48_144, &role("snare"), &role("kick"), 0).unwrap();
    check_add_hit_merge(&base, 48_000, 48_144, &role("kick"), &role("kick"), 0).unwrap();

    let mut runner = runner();
    let strategy = std_rate().prop_flat_map(|r| {
        (
            track_parts(r).prop_filter("room for the window", move |p| {
                p.len >= r.samples_for_ms(200)
            }),
            any::<u64>(),
            1u32..=20,
            any::<bool>(),
            any::<u64>(),
            any::<u64>(),
            any::<u64>(),
        )
    });
    runner
        .run(
            &strategy,
            |(parts, x_raw, away_ms, later, detected, forced, salt)| {
                let r = parts.rate;
                let len = parts.len;
                let tol = u64::from(default_tol(r));
                let away = ms(r, away_ms).max(1);
                let x = away + x_raw % (len - 2 * away);
                let y = if later { x + away } else { x - away };
                let mut base = parts;
                // A clear window: nothing else within twice the tolerance.
                base.hits.retain(|h| h.anchor.0.abs_diff(x) > 2 * tol);
                tag_hits(&mut base);
                let base = canonical(base);
                check_add_hit_merge(&base, x, y, &pool_role(detected), &pool_role(forced), salt)
            },
        )
        .unwrap();
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

fn track48(len: u64, spans: Vec<GateSpan>, roles: &[&str], hits: Vec<HitEvent>) -> TrackParts {
    let mut p = blank(rate(48_000), len);
    p.spans = spans;
    p.roles = roles.iter().map(|n| role(n)).collect();
    p.hits = hits;
    p
}

#[test]
fn default_tol_converts_by_rate() {
    assert_eq!(DEFAULT_TOL_MS, 25);
    assert_eq!(default_tol(rate(48_000)), 1_200);
    assert_eq!(default_tol(rate(44_100)), 1_103); // 1102.5 rounds half up
    assert_eq!(default_tol(rate(1_000)), 25);
}

#[test]
fn an_invalid_draft_is_the_only_error() {
    let mut p = blank(rate(48_000), 1_000);
    p.len = 0;
    let err = apply_edits(p, &entries(&[Edit::DeleteHit { hit: anchor(0, 0) }])).unwrap_err();
    assert_eq!(err, TrackError::Length);
}

#[test]
fn no_edits_gives_the_sorted_draft() {
    let p = track48(
        10_000,
        vec![],
        &["kick", "snare"],
        vec![hit(500, 10, 1), hit(100, 10, 0)],
    );
    let (t, reports) = apply_edits(p.clone(), &[]).unwrap();
    assert!(reports.is_empty());
    assert_eq!(t, ControlTrack::new(p).unwrap());
}

#[test]
fn reports_keep_ids_and_order_and_a_later_edit_wins() {
    let p = track48(10_000, vec![], &["kick"], vec![hit(1_000, 10, 0)]);
    let list = vec![
        EditEntry {
            id: EditId(7),
            edit: Edit::ForceRole {
                hit: anchor(1_000, 0),
                role: role("snare"),
            },
        },
        EditEntry {
            id: EditId(3),
            edit: Edit::DeleteHit {
                hit: anchor(5_000, 10),
            },
        },
        EditEntry {
            id: EditId(9),
            edit: Edit::ForceRole {
                hit: anchor(1_005, 10),
                role: role("clap"),
            },
        },
    ];
    let (t, reports) = apply_edits(p, &list).unwrap();
    let got: Vec<(u32, Outcome)> = reports.iter().map(|r| (r.id.0, r.outcome)).collect();
    assert_eq!(
        got,
        vec![
            (7, Outcome::Applied),
            (3, Outcome::Dormant(Dormancy::NoTarget)),
            (9, Outcome::Applied),
        ]
    );
    assert_eq!(t.roles()[usize::from(t.hits()[0].role)], role("clap"));
    // Roles once added stay in the table, sorted.
    assert_eq!(t.roles(), &[role("clap"), role("kick"), role("snare")]);
}

#[test]
fn split_snaps_to_the_earlier_of_two_equidistant_boundaries() {
    let p = track48(
        10_000,
        vec![
            gspan(0, 1_000, Link::Detached),
            gspan(1_000, 2_000, Link::Retrigger),
            gspan(2_000, 3_000, Link::Retrigger),
        ],
        &[],
        vec![],
    );
    let (t, o) = run1(
        &p,
        &Edit::SplitAt {
            at: TimelinePos(1_500),
            articulation: Articulation::Legato,
        },
    );
    assert_eq!(o, Outcome::Applied);
    let edges: Vec<(u64, u64, Link)> = t.spans.iter().map(|s| (s.on.0, s.off.0, s.link)).collect();
    assert_eq!(
        edges,
        vec![
            (0, 1_500, Link::Detached),
            (1_500, 2_000, Link::Legato),
            (2_000, 3_000, Link::Retrigger),
        ]
    );
    // The moved span keeps its anchor.
    assert_eq!(t.spans[1].anchor.0, 1_000);
}

#[test]
fn split_edge_cases() {
    let r = rate(48_000);
    let min = ms(r, MIN_PIECE_MS); // 480
    let p = track48(
        100_000,
        vec![
            gspan(10_000, 20_000, Link::Detached),
            gspan(30_000, 30_300, Link::Detached),
            gspan(30_300, 40_000, Link::Retrigger),
        ],
        &[],
        vec![],
    );
    let split = |at: u64| {
        run1(
            &p,
            &Edit::SplitAt {
                at: TimelinePos(at),
                articulation: Articulation::Retrigger,
            },
        )
        .1
    };
    // At a Detached span's start there is no boundary to snap to.
    assert_eq!(split(10_000), Outcome::Dormant(Dormancy::TooShort));
    assert_eq!(
        split(10_000 + min - 1),
        Outcome::Dormant(Dormancy::TooShort)
    );
    assert_eq!(split(10_000 + min), Outcome::Applied);
    assert_eq!(split(20_000 - min), Outcome::Applied);
    assert_eq!(
        split(20_000 - min + 1),
        Outcome::Dormant(Dormancy::TooShort)
    );
    // Gaps, the span's own end, and past the track are silence.
    assert_eq!(split(5_000), Outcome::Dormant(Dormancy::NoTarget));
    assert_eq!(split(20_000), Outcome::Dormant(Dormancy::NoTarget));
    assert_eq!(split(100_000), Outcome::Dormant(Dormancy::NoTarget));
    assert_eq!(split(u64::MAX), Outcome::Dormant(Dormancy::NoTarget));
    // Within 20 ms (960) of the 30,300 boundary: it moves, if the pieces allow.
    assert_eq!(split(30_200), Outcome::Dormant(Dormancy::TooShort));
    assert_eq!(split(30_300 + 900), Outcome::Applied);
    assert_eq!(split(30_300 + 961), Outcome::Applied); // a fresh split instead
    let (t, _) = run1(
        &p,
        &Edit::SplitAt {
            at: TimelinePos(31_200),
            articulation: Articulation::Legato,
        },
    );
    assert_eq!(t.spans.len(), 3);
    assert_eq!((t.spans[1].off.0, t.spans[2].on.0), (31_200, 31_200));
    assert_eq!(t.spans[2].link, Link::Legato);
}

#[test]
fn join_fills_the_gap_and_keeps_the_first_span() {
    let mut a = gspan(100, 200, Link::Detached);
    a.anchor = TimelinePos(90);
    a.vel = vel(10);
    let mut b = gspan(300, 400, Link::Detached);
    b.vel = vel(20);
    let c = gspan(400, 500, Link::Legato);
    let p = track48(1_000, vec![a, b, c], &[], vec![]);
    let (t, o) = run1(
        &p,
        &Edit::JoinAt {
            at: TimelinePos(250),
            tol: 10,
        },
    );
    assert_eq!(o, Outcome::Applied);
    assert_eq!(t.spans.len(), 2);
    assert_eq!(
        t.spans[0],
        GateSpan {
            on: TimelinePos(100),
            off: TimelinePos(400),
            anchor: TimelinePos(90),
            vel: vel(10),
            link: Link::Detached,
        }
    );
    assert_eq!(t.spans[1].link, Link::Legato);
    // A junction farther than tol is no target.
    let (_, o) = run1(
        &p,
        &Edit::JoinAt {
            at: TimelinePos(150),
            tol: 49,
        },
    );
    assert_eq!(o, Outcome::Dormant(Dormancy::NoTarget));
    // Equidistant junctions: the earlier one.
    let p = track48(
        1_000,
        vec![
            gspan(0, 100, Link::Detached),
            gspan(100, 200, Link::Retrigger),
            gspan(200, 300, Link::Retrigger),
        ],
        &[],
        vec![],
    );
    let (t, _) = run1(
        &p,
        &Edit::JoinAt {
            at: TimelinePos(150),
            tol: 50,
        },
    );
    let edges: Vec<(u64, u64)> = t.spans.iter().map(|s| (s.on.0, s.off.0)).collect();
    assert_eq!(edges, vec![(0, 200), (200, 300)]);
}

#[test]
fn force_role_inserts_the_role_sorted_and_remaps() {
    let p = track48(
        10_000,
        vec![],
        &["hat_closed", "snare"],
        vec![hit(100, 10, 0), hit(200, 10, 1), hit(300, 10, 1)],
    );
    let (t, o) = run1(
        &p,
        &Edit::ForceRole {
            hit: anchor(200, 0),
            role: role("kick"),
        },
    );
    assert_eq!(o, Outcome::Applied);
    assert_eq!(
        t.roles,
        vec![role("hat_closed"), role("kick"), role("snare")]
    );
    let names: Vec<&str> = t.hits.iter().map(|h| role_of(&t, h).as_str()).collect();
    assert_eq!(names, vec!["hat_closed", "kick", "snare"]);
    // Already that role: NoOp, nothing changes.
    let (u, o) = run1(
        &t,
        &Edit::ForceRole {
            hit: anchor(200, 0),
            role: role("kick"),
        },
    );
    assert_eq!(o, Outcome::NoOp);
    assert_eq!(u, t);
}

#[test]
fn a_full_role_table_leaves_the_edit_dormant() {
    let names: Vec<String> = (0..MAX_ROLES).map(|i| format!("r{i:03}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let p = track48(10_000, vec![], &refs, vec![hit(100, 10, 0)]);
    let (t, o) = run1(
        &p,
        &Edit::ForceRole {
            hit: anchor(100, 0),
            role: role("zzz"),
        },
    );
    assert_eq!(o, Outcome::Dormant(Dormancy::OutOfRange));
    assert_eq!(t, canonical(p.clone()));
    let (_, o) = run1(
        &p,
        &Edit::AddHit {
            hit: anchor(5_000, 0),
            dur: 10,
            role: role("zzz"),
            vel: vel(1),
        },
    );
    assert_eq!(o, Outcome::Dormant(Dormancy::OutOfRange));
    // An existing role still works.
    let (_, o) = run1(
        &p,
        &Edit::ForceRole {
            hit: anchor(100, 0),
            role: role("r127"),
        },
    );
    assert_eq!(o, Outcome::Applied);
}

#[test]
fn hit_targets_nearest_anchor_then_earlier_then_first_in_canonical_order() {
    // Two hits share anchor 1,000 (at 1,000 and 1,096); another sits at 1,200.
    let mut late = hit(1_096, 10, 0);
    late.anchor = TimelinePos(1_000);
    let p = track48(
        10_000,
        vec![],
        &["kick", "snare"],
        vec![late, hit(1_000, 10, 0), hit(1_200, 10, 0)],
    );
    let force = |at: u64, tol: u32| {
        let (t, o) = run1(
            &p,
            &Edit::ForceRole {
                hit: anchor(at, tol),
                role: role("snare"),
            },
        );
        assert_eq!(o, Outcome::Applied);
        t.hits
            .iter()
            .filter(|h| role_of(&t, h).as_str() == "snare")
            .map(|h| (h.at.0, h.anchor.0))
            .collect::<Vec<_>>()
    };
    // Equidistant from 1,000 and 1,200: the earlier anchor, and of its two
    // hits the first in canonical order.
    assert_eq!(force(1_100, 100), vec![(1_000, 1_000)]);
    assert_eq!(force(1_150, 100), vec![(1_200, 1_200)]);
    let (_, o) = run1(
        &p,
        &Edit::DeleteHit {
            hit: anchor(1_100, 99),
        },
    );
    assert_eq!(o, Outcome::Dormant(Dormancy::NoTarget));
}

#[test]
fn add_hit_inserts_with_defaults_and_clamps_dur() {
    let r = rate(48_000);
    let len = 10 * 48_000 * 60; // ten minutes
    let p = blank(r, len);
    let add = |at: u64, dur: u64| {
        let (t, o) = run1(
            &p,
            &Edit::AddHit {
                hit: anchor(at, 0),
                dur,
                role: role("kick"),
                vel: vel(77),
            },
        );
        (t, o)
    };
    let (t, o) = add(1_000, 0);
    assert_eq!(o, Outcome::Applied);
    assert_eq!(t.roles, vec![role("kick")]);
    let h = t.hits[0];
    assert_eq!(
        (h.at.0, h.anchor.0, h.dur, h.role, h.vel),
        (1_000, 1_000, 1, 0, vel(77))
    );
    assert_eq!((h.bright.get(), h.breath.get(), h.mouth.get()), (64, 0, 0));
    assert_eq!(add(1_000, u64::MAX).0.hits[0].dur, ms(r, MAX_HIT_MS));
    assert_eq!(add(len - 100, u64::MAX).0.hits[0].dur, 100);
    assert_eq!(add(len - 1, 5).0.hits[0].dur, 1);
    assert_eq!(add(len, 5).1, Outcome::Dormant(Dormancy::OutOfRange));
    assert_eq!(add(u64::MAX, 5).1, Outcome::Dormant(Dormancy::OutOfRange));
}

#[test]
fn add_hit_merges_with_a_hit_at_the_very_end() {
    let p = track48(1_000, vec![], &["snare"], vec![hit(990, 10, 0)]);
    let (t, o) = run1(
        &p,
        &Edit::AddHit {
            hit: anchor(1_005, 20),
            dur: 10,
            role: role("kick"),
            vel: vel(1),
        },
    );
    assert_eq!(o, Outcome::Merged);
    assert_eq!(t.hits.len(), 1);
    assert_eq!(role_of(&t, &t.hits[0]), &role("kick"));
}

#[test]
fn mute_span_clips_splits_and_relinks() {
    let mut a = gspan(0, 1_000, Link::Detached);
    a.anchor = TimelinePos(5);
    let mut b = gspan(1_000, 2_000, Link::Retrigger);
    b.anchor = TimelinePos(1_010);
    let p = track48(3_000, vec![a, b], &[], vec![]);
    let edges = |t: &TrackParts| -> Vec<(u64, u64, u64, Link)> {
        t.spans
            .iter()
            .map(|s| (s.on.0, s.off.0, s.anchor.0, s.link))
            .collect()
    };
    // Straddling: two pieces, the second new and anchored where it starts.
    let (t, o) = run1(
        &p,
        &Edit::MuteSpan {
            span: tspan(500, 700),
        },
    );
    assert_eq!(o, Outcome::Applied);
    assert_eq!(
        edges(&t),
        vec![
            (0, 500, 5, Link::Detached),
            (700, 1_000, 700, Link::Detached),
            (1_000, 2_000, 1_010, Link::Retrigger),
        ]
    );
    // Over the boundary: the second span is clipped, keeps its anchor and is
    // now after a gap.
    let (t, _) = run1(
        &p,
        &Edit::MuteSpan {
            span: tspan(900, 1_100),
        },
    );
    assert_eq!(
        edges(&t),
        vec![
            (0, 900, 5, Link::Detached),
            (1_100, 2_000, 1_010, Link::Detached)
        ]
    );
    // Covering everything, and past the end.
    let (t, o) = run1(
        &p,
        &Edit::MuteSpan {
            span: tspan(0, u64::MAX),
        },
    );
    assert_eq!(o, Outcome::Applied);
    assert!(t.spans.is_empty());
    // Empty, or starting at or past the end: out of range.
    for span in [
        tspan(500, 500),
        tspan(3_000, 4_000),
        tspan(u64::MAX, u64::MAX),
    ] {
        let (t, o) = run1(&p, &Edit::MuteSpan { span });
        assert_eq!(o, Outcome::Dormant(Dormancy::OutOfRange));
        assert_eq!(t, canonical(p.clone()));
    }
}

#[test]
fn mute_span_removes_hits_inside_and_cuts_tails() {
    let mut moved = hit(400, 50, 0); // anchored before the mute, fires inside it
    moved.at = TimelinePos(520);
    let p = track48(
        3_000,
        vec![],
        &["kick"],
        vec![
            hit(100, 1_000, 0), // rings into the mute: cut to 400
            hit(500, 10, 0),    // anchored at its start: removed
            hit(699, 10, 0),    // last sample inside: removed
            hit(700, 10, 0),    // at its end: kept
            hit(450, 30, 0),    // ends before it: kept
            moved,
        ],
    );
    let (t, _) = run1(
        &p,
        &Edit::MuteSpan {
            span: tspan(500, 700),
        },
    );
    let kept: Vec<(u64, u64)> = t.hits.iter().map(|h| (h.at.0, h.dur)).collect();
    assert_eq!(kept, vec![(100, 400), (450, 30), (700, 10)]);
}

#[test]
fn force_pitch_covers_frames_and_clamps() {
    // 44.1 kHz: frame k starts at ceil(220.5 k).
    let r = rate(44_100);
    let len = 2_205; // ten frames' worth; frames_covering gives 10
    let p = blank(r, len);
    let pitch = |span: TimelineSpan, st: f32| {
        let (t, o) = run1(&p, &Edit::ForcePitch { span, st });
        (t.curves.pitch_st, o)
    };
    // [221, 662): frames 1 (221) and 2 (441); frame 3 starts at 662.
    let (v, o) = pitch(tspan(221, 662), 60.0);
    assert_eq!(o, Outcome::Applied);
    let changed: Vec<usize> = (0..v.len())
        .filter(|&k| v[k].to_bits() != 48f32.to_bits())
        .collect();
    assert_eq!(changed, vec![1, 2]);
    // Out-of-range values clamp to the pitch range.
    let (v, _) = pitch(tspan(0, 1), 300.0);
    assert_eq!(v[0].to_bits(), 192f32.to_bits());
    let (v, _) = pitch(tspan(0, 1), -300.0);
    assert_eq!(v[0].to_bits(), (-64f32).to_bits());
    // Past the end, every remaining frame: only frame 9 starts at or after
    // 1,900 (at 1,985).
    let (v, _) = pitch(tspan(1_900, u64::MAX), 50.0);
    let changed: Vec<usize> = (0..v.len())
        .filter(|&k| v[k].to_bits() != 48f32.to_bits())
        .collect();
    assert_eq!(changed, vec![9]);
    // A span with no frame inside still lands on the track.
    let (v, o) = pitch(tspan(222, 223), 50.0);
    assert_eq!(o, Outcome::Applied);
    assert!(v.iter().all(|x| x.to_bits() == 48f32.to_bits()));
    // Non-finite, empty, or past the end: out of range.
    for (span, st) in [
        (tspan(0, 100), f32::NAN),
        (tspan(0, 100), f32::INFINITY),
        (tspan(50, 50), 60.0),
        (tspan(len, len + 10), 60.0),
    ] {
        assert_eq!(pitch(span, st).1, Outcome::Dormant(Dormancy::OutOfRange));
    }
}

#[test]
fn rebase_keeps_dur_positive_and_saturates_tol() {
    let (lo, hi) = (rate(1_000), rate(1_536_000));
    let e = Edit::AddHit {
        hit: anchor(1, u32::MAX),
        dur: 1,
        role: role("kick"),
        vel: vel(1),
    };
    match e.rebased(hi, lo) {
        Edit::AddHit { hit, dur, .. } => {
            assert_eq!((hit.at.0, dur), (0, 1));
            assert_eq!(hit.tol, 2_796_203); // u32::MAX * 1000 / 1536000, rounded
        }
        other => panic!("{other:?}"),
    }
    match e.rebased(lo, hi) {
        Edit::AddHit { hit, dur, .. } => {
            assert_eq!((hit.at.0, hit.tol, dur), (1_536, u32::MAX, 1_536));
        }
        other => panic!("{other:?}"),
    }
    // 44.1 kHz to 48 kHz and back.
    let e = Edit::MuteSpan {
        span: tspan(44_100, 88_200),
    };
    assert_eq!(
        e.rebased(rate(44_100), rate(48_000)),
        Edit::MuteSpan {
            span: tspan(48_000, 96_000)
        }
    );
}
