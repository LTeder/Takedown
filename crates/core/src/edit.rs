//! Time-anchored edits and their application (core-types §2.10, principle 7).
//!
//! Anchors are timeline samples, the one clock no slider, tempo, render rate
//! or re-segmentation can move. Edits apply in list order to the lens's draft
//! after derivation and before timing feel and perceptual-centre pre-roll move
//! `at`. An edit takes the nearest anchor within its tolerance, the earlier one
//! on a tie, and is never retargeted outside it. An edit with no target is
//! dormant: kept, reported, retried on every derivation, never deleted.

use serde::{Deserialize, Serialize};

use crate::control::{
    ControlTrack, Curve, GateSpan, HitEvent, Link, MAX_HIT_MS, MAX_ROLES, TrackError, TrackParts,
    U7, Velocity,
};
use crate::hit::{HitAnchor, RoleName};
use crate::ids::EditId;
use crate::key::{KeyError, KeyHash, KeyWriter};
use crate::time::{SampleRate, TimelinePos, TimelineSpan, rescale_round};
use crate::views::covered_frames;

/// The onset F1 window.
pub const DEFAULT_TOL_MS: u32 = 25;
/// A SplitAt this close to an internal boundary moves the boundary instead.
pub const SPLIT_SNAP_MS: u32 = 20;
/// The shortest piece a SplitAt may leave.
pub const MIN_PIECE_MS: u32 = 10;

/// [`DEFAULT_TOL_MS`] in samples at `rate`: the tolerance a new hit edit gets.
pub fn default_tol(rate: SampleRate) -> u32 {
    // At most 25 ms at 1.536 MHz, 38,400 samples.
    u32::try_from(rate.samples_for_ms(DEFAULT_TOL_MS)).unwrap_or(u32::MAX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Articulation {
    Retrigger,
    Legato,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    /// Every frame whose time lies in `span`.
    ForcePitch {
        span: TimelineSpan,
        st: f32,
    },
    SplitAt {
        at: TimelinePos,
        articulation: Articulation,
    },
    JoinAt {
        at: TimelinePos,
        tol: u32,
    },
    ForceRole {
        hit: HitAnchor,
        role: RoleName,
    },
    DeleteHit {
        hit: HitAnchor,
    },
    AddHit {
        hit: HitAnchor,
        dur: u64,
        role: RoleName,
        vel: Velocity,
    },
    MuteSpan {
        span: TimelineSpan,
    },
}

impl KeyHash for Articulation {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let name = match self {
            Self::Retrigger => "retrigger",
            Self::Legato => "legato",
        };
        w.variant(name, |_| Ok(()))
    }
}

/// The edit op; variants by their serde names, fields as records.
impl KeyHash for Edit {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        match self {
            Self::ForcePitch { span, st } => w.variant("force_pitch", |r| {
                r.field("span", span)?;
                r.field("st", st)
            }),
            Self::SplitAt { at, articulation } => w.variant("split_at", |r| {
                r.field("at", at)?;
                r.field("articulation", articulation)
            }),
            Self::JoinAt { at, tol } => w.variant("join_at", |r| {
                r.field("at", at)?;
                r.field("tol", tol)
            }),
            Self::ForceRole { hit, role } => w.variant("force_role", |r| {
                r.field("hit", hit)?;
                r.field("role", role)
            }),
            Self::DeleteHit { hit } => w.variant("delete_hit", |r| r.field("hit", hit)),
            Self::AddHit {
                hit,
                dur,
                role,
                vel,
            } => w.variant("add_hit", |r| {
                r.field("hit", hit)?;
                r.field("dur", dur)?;
                r.field("role", role)?;
                r.field("vel", vel)
            }),
            Self::MuteSpan { span } => w.variant("mute_span", |r| r.field("span", span)),
        }
    }
}

/// An edit and its local id. The id is outside keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditEntry {
    pub id: EditId,
    pub edit: Edit,
}

/// Keys only the op: the id is outside keys.
impl KeyHash for EditEntry {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { id: _, edit } = self;
        edit.write_key(w)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dormancy {
    NoTarget,
    TooShort,
    OutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Applied,
    Merged,
    NoOp,
    Dormant(Dormancy),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditReport {
    pub id: EditId,
    pub outcome: Outcome,
}

/// Applies `edits` in list order to the lens's draft, so a later edit wins.
///
/// The draft is validated (and its hits sorted) first; an invalid draft is the
/// only error. Every edit then either applies or is reported dormant, and the
/// result is validated again. Pure: the same draft and edits give the same
/// track and the same reports, one report per edit, in list order.
///
/// Calls the spec leaves open, made here:
/// - A hit edit targets one hit: the first in canonical order among the hits
///   at the nearest anchor ([`HitAnchor::resolve`] over the sorted hits).
/// - ForcePitch writes the frames NoteView counts as covered (those whose
///   `frame_start` lies in the span), clamps `st` into the pitch range, and is
///   `Dormant(OutOfRange)` for a non-finite `st`.
/// - ForceRole is `NoOp` when the hit already has the role, like AddHit. A
///   new role that would overflow [`MAX_ROLES`] makes ForceRole and AddHit
///   `Dormant(OutOfRange)`.
/// - AddHit merges by anchor before checking range, so a hit at the very end
///   still absorbs an AddHit placed just past it.
/// - MuteSpan also removes hits that fire inside the span (this differs from
///   "anchored inside" only once `at` has moved). The second piece of a
///   straddling span is a new span anchored where it starts, as in SplitAt.
pub fn apply_edits(
    draft: TrackParts,
    edits: &[EditEntry],
) -> Result<(ControlTrack, Vec<EditReport>), TrackError> {
    let mut t = ControlTrack::new(draft)?.into_parts();
    let reports = edits
        .iter()
        .map(|e| EditReport {
            id: e.id,
            outcome: apply(&mut t, &e.edit),
        })
        .collect();
    Ok((ControlTrack::new(t)?, reports))
}

impl Edit {
    /// Positions via `rescale_round`, tolerances scaled (saturating at
    /// `u32::MAX`), `dur` kept >= 1. With `to >= from`, rebasing up and back
    /// down is the identity.
    #[must_use]
    pub fn rebased(&self, from: SampleRate, to: SampleRate) -> Edit {
        let pos = |p: TimelinePos| TimelinePos(rescale_round(p.0, from, to));
        let span = |s: TimelineSpan| {
            TimelineSpan::new(pos(s.start()), pos(s.end()))
                .expect("rescale_round is monotone, so start <= end survives")
        };
        let tol = |t: u32| u32::try_from(rescale_round(u64::from(t), from, to)).unwrap_or(u32::MAX);
        let anchor = |h: HitAnchor| HitAnchor {
            at: pos(h.at),
            tol: tol(h.tol),
        };
        match self {
            Self::ForcePitch { span: s, st } => Self::ForcePitch {
                span: span(*s),
                st: *st,
            },
            Self::SplitAt { at, articulation } => Self::SplitAt {
                at: pos(*at),
                articulation: *articulation,
            },
            Self::JoinAt { at, tol: t } => Self::JoinAt {
                at: pos(*at),
                tol: tol(*t),
            },
            Self::ForceRole { hit, role } => Self::ForceRole {
                hit: anchor(*hit),
                role: role.clone(),
            },
            Self::DeleteHit { hit } => Self::DeleteHit { hit: anchor(*hit) },
            Self::AddHit {
                hit,
                dur,
                role,
                vel,
            } => Self::AddHit {
                hit: anchor(*hit),
                dur: rescale_round(*dur, from, to).max(1),
                role: role.clone(),
                vel: *vel,
            },
            Self::MuteSpan { span: s } => Self::MuteSpan { span: span(*s) },
        }
    }
}

/// What an added hit starts with: bright 64, breath 0, mouth 0.
const ADDED_BRIGHT: U7 = U7::new(64).expect("64 is a 7-bit value");
const ADDED_BREATH: U7 = U7::new(0).expect("0 is a 7-bit value");
const ADDED_MOUTH: U7 = U7::new(0).expect("0 is a 7-bit value");

/// Applies one edit to valid, canonical parts and leaves them valid and
/// canonical.
fn apply(t: &mut TrackParts, edit: &Edit) -> Outcome {
    match edit {
        Edit::ForcePitch { span, st } => force_pitch(t, *span, *st),
        Edit::SplitAt { at, articulation } => split_at(t, *at, *articulation),
        Edit::JoinAt { at, tol } => join_at(t, *at, u64::from(*tol)),
        Edit::ForceRole { hit, role } => match target(&t.hits, *hit) {
            Some(i) => set_role(t, i, role, Outcome::Applied),
            None => Outcome::Dormant(Dormancy::NoTarget),
        },
        Edit::DeleteHit { hit } => match target(&t.hits, *hit) {
            Some(i) => {
                t.hits.remove(i);
                Outcome::Applied
            }
            None => Outcome::Dormant(Dormancy::NoTarget),
        },
        Edit::AddHit {
            hit,
            dur,
            role,
            vel,
        } => add_hit(t, *hit, *dur, role, *vel),
        Edit::MuteSpan { span } => mute_span(t, *span),
    }
}

/// Whether a time edit's span touches the track at all.
fn touches_track(t: &TrackParts, span: TimelineSpan) -> bool {
    !span.is_empty() && span.start().0 < t.len
}

fn force_pitch(t: &mut TrackParts, span: TimelineSpan, st: f32) -> Outcome {
    if !touches_track(t, span) || !st.is_finite() {
        return Outcome::Dormant(Dormancy::OutOfRange);
    }
    let (lo, hi) = Curve::PitchSt.range();
    let st = st.clamp(lo, hi);
    let pitch = &mut t.curves.pitch_st;
    let frames = covered_frames(span.start(), span.end(), t.rate);
    let end = usize::try_from(frames.end)
        .unwrap_or(usize::MAX)
        .min(pitch.len());
    let start = usize::try_from(frames.start).unwrap_or(usize::MAX).min(end);
    pitch[start..end].fill(st);
    Outcome::Applied
}

/// The index of the span containing `pos`.
fn gate_at(spans: &[GateSpan], pos: TimelinePos) -> Option<usize> {
    let i = spans.partition_point(|s| s.on <= pos).checked_sub(1)?;
    (pos < spans[i].off).then_some(i)
}

fn split_at(t: &mut TrackParts, at: TimelinePos, articulation: Articulation) -> Outcome {
    let snap = t.rate.samples_for_ms(SPLIT_SNAP_MS);
    let min_piece = t.rate.samples_for_ms(MIN_PIECE_MS);
    let link = match articulation {
        Articulation::Retrigger => Link::Retrigger,
        Articulation::Legato => Link::Legato,
    };
    let spans = &mut t.spans;
    let Some(covering) = gate_at(spans, at) else {
        return Outcome::Dormant(Dormancy::NoTarget);
    };
    // The nearest internal boundary (the on of span i abutting span i - 1)
    // within the snap distance. Boundaries increase with i, so the lower i is
    // the earlier boundary on a tie.
    let boundary = (1..spans.len())
        .filter(|&i| spans[i - 1].off == spans[i].on)
        .map(|i| (spans[i].on.0.abs_diff(at.0), i))
        .filter(|&(d, _)| d <= snap)
        .min();
    // The spans whose outer edges bound the two pieces.
    let (first, second) = match boundary {
        Some((_, i)) => (i - 1, i),
        None => (covering, covering),
    };
    let (start, end) = (spans[first].on.0, spans[second].off.0);
    if at.0.saturating_sub(start) < min_piece || end.saturating_sub(at.0) < min_piece {
        return Outcome::Dormant(Dormancy::TooShort);
    }
    if boundary.is_some() {
        spans[first].off = at;
        spans[second].on = at;
        spans[second].link = link;
    } else {
        let whole = spans[covering];
        spans[covering].off = at;
        spans.insert(
            covering + 1,
            GateSpan {
                on: at,
                off: whole.off,
                anchor: at,
                vel: whole.vel,
                link,
            },
        );
    }
    Outcome::Applied
}

fn join_at(t: &mut TrackParts, at: TimelinePos, tol: u64) -> Outcome {
    let spans = &mut t.spans;
    // The distance from `at` to each junction [off of the first, on of the
    // second], a single point when the pair abuts. Lower i wins a tie.
    let best = (1..spans.len())
        .filter_map(|i| {
            let (a, b) = (spans[i - 1].off.0, spans[i].on.0);
            let d = if at.0 < a {
                a - at.0
            } else {
                at.0.saturating_sub(b)
            };
            (d <= tol).then_some((d, i))
        })
        .min();
    let Some((_, i)) = best else {
        return Outcome::Dormant(Dormancy::NoTarget);
    };
    // The merged span keeps the first span's on, anchor, velocity and link.
    // Its off is the second's, so the link of the span after it still holds.
    spans[i - 1].off = spans[i].off;
    spans.remove(i);
    Outcome::Applied
}

/// The hit an anchor selects: the first in canonical order at the nearest
/// anchor within tolerance.
fn target(hits: &[HitEvent], anchor: HitAnchor) -> Option<usize> {
    let anchors: Vec<TimelinePos> = hits.iter().map(|h| h.anchor).collect();
    anchor.resolve(&anchors)
}

/// The index of `role`, adding it to the sorted role table and remapping
/// every hit when it is new. `None` when the table is full.
fn role_index(t: &mut TrackParts, role: &RoleName) -> Option<u8> {
    match t.roles.binary_search(role) {
        Ok(i) => u8::try_from(i).ok(),
        Err(i) => {
            if t.roles.len() >= MAX_ROLES {
                return None;
            }
            let i = u8::try_from(i).ok()?;
            t.roles.insert(usize::from(i), role.clone());
            for h in &mut t.hits {
                if h.role >= i {
                    h.role += 1;
                }
            }
            Some(i)
        }
    }
}

/// Gives hit `i` the role `role`: `changed` when it did not have it, `NoOp`
/// when it did.
fn set_role(t: &mut TrackParts, i: usize, role: &RoleName, changed: Outcome) -> Outcome {
    if t.roles[usize::from(t.hits[i].role)] == *role {
        return Outcome::NoOp;
    }
    let Some(r) = role_index(t, role) else {
        return Outcome::Dormant(Dormancy::OutOfRange);
    };
    t.hits[i].role = r;
    sort_hits(t);
    changed
}

fn add_hit(
    t: &mut TrackParts,
    hit: HitAnchor,
    dur: u64,
    role: &RoleName,
    vel: Velocity,
) -> Outcome {
    if let Some(i) = target(&t.hits, hit) {
        return set_role(t, i, role, Outcome::Merged);
    }
    let at = hit.at;
    if at.0 >= t.len {
        return Outcome::Dormant(Dormancy::OutOfRange);
    }
    let Some(role) = role_index(t, role) else {
        return Outcome::Dormant(Dormancy::OutOfRange);
    };
    let max_dur = t.rate.samples_for_ms(MAX_HIT_MS).min(t.len - at.0);
    t.hits.push(HitEvent {
        at,
        anchor: at,
        dur: dur.clamp(1, max_dur),
        role,
        vel,
        bright: ADDED_BRIGHT,
        breath: ADDED_BREATH,
        mouth: ADDED_MOUTH,
    });
    sort_hits(t);
    Outcome::Applied
}

fn mute_span(t: &mut TrackParts, span: TimelineSpan) -> Outcome {
    if !touches_track(t, span) {
        return Outcome::Dormant(Dormancy::OutOfRange);
    }
    let (s, e) = (span.start(), span.end());
    let mut spans = Vec::with_capacity(t.spans.len() + 1);
    for g in &t.spans {
        if g.off <= s || g.on >= e {
            spans.push(*g);
            continue;
        }
        if g.on < s {
            spans.push(GateSpan { off: s, ..*g });
        }
        if g.off > e {
            // A span clipped at its start keeps its identity; the second
            // piece of a straddling span is new and anchors where it starts.
            let anchor = if g.on < s { e } else { g.anchor };
            spans.push(GateSpan {
                on: e,
                anchor,
                link: Link::Detached,
                ..*g
            });
        }
    }
    relink(&mut spans);
    t.spans = spans;
    t.hits
        .retain(|h| !span.contains(h.anchor) && !span.contains(h.at));
    for h in &mut t.hits {
        if h.at < s {
            h.dur = h.dur.min(s.0 - h.at.0);
        }
    }
    sort_hits(t);
    Outcome::Applied
}

/// Restores the link invariant: the first span and every span after a gap
/// are Detached, and an abutting span that was Detached becomes Retrigger.
/// Links that already hold are left alone.
fn relink(spans: &mut [GateSpan]) {
    for i in 0..spans.len() {
        let gap = i == 0 || spans[i].on > spans[i - 1].off;
        if gap {
            spans[i].link = Link::Detached;
        } else if spans[i].link == Link::Detached {
            spans[i].link = Link::Retrigger;
        }
    }
}

fn sort_hits(t: &mut TrackParts) {
    t.hits.sort_unstable_by_key(HitEvent::canonical_key);
}
