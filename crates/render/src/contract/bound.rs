//! `BoundTrack`: one voice's view of a `ControlTrack` at one output rate,
//! with its triggers, slices and segment plan precomputed off the audio
//! thread.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use takedown_core::control::{ControlFrame, ControlTrack, EventKind, Lane, U7, Velocity};
use takedown_core::hit::RoleName;
use takedown_core::ids::TakeId;
use takedown_core::seed::VoiceSeeds;
use takedown_core::time::{
    ControlClock, MIN_GAP_FRAMES, RenderPos, SampleRate, TimelinePos, TimelineSpan, control_index,
    frame_start, frames_covering, rescale_ceil, rescale_round,
};

use super::{BindSpec, SliceSource, VoiceRoute};

/// Extra activity at each end of every event. Output-rate rounding moves a
/// trigger's end by under 1.5 output samples, which is under 0.3 frames at
/// any accepted rate.
pub const ROUNDING_SLACK_FRAMES: u64 = 1;

/// A slice of take audio held by a [`SliceBank`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SliceId(pub u32);

/// The union of all slice spans, resampled once into one buffer. Each slice is
/// a range into it: no copy per event, and at most one resampled take in
/// memory.
#[derive(Clone, Default, PartialEq)]
pub struct SliceBank {
    data: Box<[f32]>,
    ranges: Box<[(u32, u32)]>,
}

impl fmt::Debug for SliceBank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SliceBank")
            .field("slices", &self.ranges.len())
            .field("samples", &self.data.len())
            .finish()
    }
}

/// A union interval of slice spans and where its samples sit in the bank.
struct Piece {
    start: u64,
    base: u64,
    len: u64,
}

impl SliceBank {
    /// The slice's samples at the output rate; empty for an unknown id.
    pub fn get(&self, id: SliceId) -> &[f32] {
        self.ranges
            .get(id.0 as usize)
            .and_then(|&(a, b)| self.data.get(a as usize..b as usize))
            .unwrap_or(&[])
    }

    /// The number of slices.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// The number of resampled samples held.
    pub fn samples(&self) -> usize {
        self.data.len()
    }

    /// Reads the union of `spans` once, then gives each span a range into it.
    /// A span gets `None` when it is empty, unreadable or too far into the
    /// bank for a `u32` index.
    fn build(
        take: TakeId,
        spans: &[TimelineSpan],
        timeline: SampleRate,
        out: SampleRate,
        source: &mut dyn SliceSource,
    ) -> (SliceBank, Vec<Option<SliceId>>) {
        let mut sorted: Vec<TimelineSpan> =
            spans.iter().copied().filter(|s| !s.is_empty()).collect();
        sorted.sort_unstable();
        let mut unions: Vec<(u64, u64)> = Vec::new();
        for s in sorted {
            match unions.last_mut() {
                Some(u) if s.start().0 <= u.1 => u.1 = u.1.max(s.end().0),
                _ => unions.push((s.start().0, s.end().0)),
            }
        }
        let mut data: Vec<f32> = Vec::new();
        let mut pieces: Vec<Piece> = Vec::with_capacity(unions.len());
        for (a, b) in unions {
            let read = TimelineSpan::new(TimelinePos(a), TimelinePos(b))
                .and_then(|span| source.read(take, span, out))
                .unwrap_or_default();
            pieces.push(Piece {
                start: a,
                base: data.len() as u64,
                len: read.len() as u64,
            });
            data.extend_from_slice(&read);
        }
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        let ids = spans
            .iter()
            .map(|s| {
                if s.is_empty() {
                    return None;
                }
                let i = pieces.partition_point(|p| p.start <= s.start().0);
                let p = pieces.get(i.checked_sub(1)?)?;
                let origin = rescale_ceil(p.start, timeline, out).0;
                let offset = |pos: TimelinePos| {
                    rescale_ceil(pos.0, timeline, out)
                        .0
                        .saturating_sub(origin)
                        .min(p.len)
                };
                let (lo, hi) = (offset(s.start()), offset(s.end()));
                if hi <= lo {
                    return None;
                }
                let a = u32::try_from(p.base + lo).ok()?;
                let b = u32::try_from(p.base + hi).ok()?;
                let id = SliceId(u32::try_from(ranges.len()).ok()?);
                ranges.push((a, b));
                Some(id)
            })
            .collect();
        let bank = SliceBank {
            data: data.into_boxed_slice(),
            ranges: ranges.into_boxed_slice(),
        };
        (bank, ids)
    }
}

/// One event of a voice, at the output rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundTrigger {
    /// The first output sample at or after the exact time.
    pub at: RenderPos,
    /// The exact time is `at - lead`, with `lead` in [0, 1): for band-limited
    /// onsets.
    pub lead: f32,
    pub anchor: TimelinePos,
    /// `SeedTree::trigger_seeds(voice, lane, anchor, ordinal)`.
    pub seeds: VoiceSeeds,
    pub kind: BoundKind,
}

/// What a trigger does. Gate events carry no pitch: a phrase voice reads pitch
/// from the curves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundKind {
    NoteOn {
        vel: Velocity,
        slice: Option<SliceId>,
    },
    Retrigger {
        vel: Velocity,
        slice: Option<SliceId>,
    },
    NoteOff,
    Hit {
        role: u8,
        vel: Velocity,
        bright: U7,
        breath: U7,
        mouth: U7,
        /// Output samples, at least 1.
        dur: u64,
        slice: Option<SliceId>,
    },
}

impl BoundKind {
    /// The take slice this trigger plays, if any.
    pub fn slice(&self) -> Option<SliceId> {
        match *self {
            Self::NoteOn { slice, .. }
            | Self::Retrigger { slice, .. }
            | Self::Hit { slice, .. } => slice,
            Self::NoteOff => None,
        }
    }

    fn set_slice(&mut self, id: Option<SliceId>) {
        match self {
            Self::NoteOn { slice, .. }
            | Self::Retrigger { slice, .. }
            | Self::Hit { slice, .. } => {
                *slice = id;
            }
            Self::NoteOff => {}
        }
    }
}

/// Active from `start` to `quiet_from`; quiet (driver-written zeros) from
/// there to the next start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub start: RenderPos,
    /// The control frame whose first output sample is `start`.
    pub frame: u64,
    pub quiet_from: RenderPos,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Immutable; built off the audio thread and shared by `Arc`.
pub struct BoundTrack {
    id: u64,
    track: Arc<ControlTrack>,
    rate: SampleRate,
    phrase: bool,
    role_mask: u128,
    voice_seeds: VoiceSeeds,
    triggers: Box<[BoundTrigger]>,
    segments: Box<[Segment]>,
    end: RenderPos,
    slices: SliceBank,
}

impl fmt::Debug for BoundTrack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundTrack")
            .field("id", &self.id)
            .field("rate", &self.rate)
            .field("phrase", &self.phrase)
            .field("role_mask", &format_args!("{:#x}", self.role_mask))
            .field("voice_seeds", &self.voice_seeds)
            .field("triggers", &self.triggers.len())
            .field("segments", &self.segments)
            .field("end", &self.end)
            .field("slices", &self.slices)
            .finish_non_exhaustive()
    }
}

/// The bit of `role` in a role mask.
fn role_bit(role: u8) -> u128 {
    1u128.checked_shl(u32::from(role)).unwrap_or(0)
}

/// The track's roles that the route takes, as a mask over role indices.
fn role_mask(roles: &[RoleName], route: &VoiceRoute) -> u128 {
    roles
        .iter()
        .enumerate()
        .filter(|(_, r)| route.roles.contains(*r))
        .fold(0, |m, (i, _)| m | u8::try_from(i).map_or(0, role_bit))
}

/// Sorts, drops empty intervals and merges overlapping or touching ones.
fn merge(mut iv: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    iv.retain(|(a, b)| a < b);
    iv.sort_unstable();
    let mut out: Vec<(u64, u64)> = Vec::with_capacity(iv.len());
    for (a, b) in iv {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// A voice's activity in control frames, sorted and merged, so it does not
/// depend on the output rate. A phrase span covers
/// `[frame(on), frames_covering(off))` and a hit `[frame(at),
/// frames_covering(at + dur))`, each with [`ROUNDING_SLACK_FRAMES`] extra at
/// both ends.
pub fn activity_frames(track: &ControlTrack, phrase: bool, role_mask: u128) -> Vec<(u64, u64)> {
    let rate = track.rate();
    let slack = ROUNDING_SLACK_FRAMES;
    let interval = |on: u64, off: u64| {
        (
            control_index(on, rate).0.saturating_sub(slack),
            frames_covering(off, rate).saturating_add(slack),
        )
    };
    let mut iv = Vec::new();
    if phrase {
        iv.extend(track.spans().iter().map(|s| interval(s.on.0, s.off.0)));
    }
    iv.extend(
        track
            .hits()
            .iter()
            .filter(|h| role_mask & role_bit(h.role) != 0)
            .map(|h| interval(h.at.0, h.at.0.saturating_add(h.dur))),
    );
    merge(iv)
}

/// `(reset frame, quiet-from frame)` pairs. There is a reset at frame 0 and at
/// the start of every interval that follows at least `MIN_GAP_FRAMES` of
/// quiet. Shorter gaps stay inside one segment, where the backend runs
/// through them and keeps its state. The first pair is `(0, 0)` when the first
/// interval itself follows such a gap.
pub fn segment_plan(activity_with_tail: &[(u64, u64)]) -> Vec<(u64, u64)> {
    let mut plan = Vec::new();
    let (mut start, mut quiet) = (0u64, 0u64);
    for (a, b) in merge(activity_with_tail.to_vec()) {
        if a.saturating_sub(quiet) >= MIN_GAP_FRAMES {
            plan.push((start, quiet));
            (start, quiet) = (a, b);
        } else {
            quiet = quiet.max(b);
        }
    }
    plan.push((start, quiet));
    plan
}

impl BoundTrack {
    /// Binds `track` to one voice at one output rate. Allocates; worker thread
    /// only. Works in four steps: triggers, slices, activity, plan.
    pub fn bind(
        track: Arc<ControlTrack>,
        spec: &BindSpec<'_>,
        slices: &mut dyn SliceSource,
    ) -> BoundTrack {
        let out = spec.out_rate;
        let phrase = spec.route.phrase;
        let role_mask = role_mask(track.roles(), spec.route);

        let (mut triggers, timeline) = bind_triggers(&track, spec, role_mask);
        let slices = bind_slices(&track, spec, &mut triggers, &timeline, slices);

        let ext = u64::from(spec.info.tail_frames)
            .saturating_add(frames_covering(u64::from(spec.prepared.latency), out));
        let with_tail: Vec<(u64, u64)> = activity_frames(&track, phrase, role_mask)
            .into_iter()
            .map(|(a, b)| (a, b.saturating_add(ext)))
            .collect();
        let segments: Box<[Segment]> = segment_plan(&with_tail)
            .into_iter()
            .map(|(reset, quiet)| Segment {
                start: RenderPos(frame_start(reset, out)),
                frame: reset,
                quiet_from: RenderPos(frame_start(quiet, out)),
            })
            .collect();
        let end = segments.last().map_or(RenderPos(0), |s| s.quiet_from);

        BoundTrack {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            track,
            rate: out,
            phrase,
            role_mask,
            voice_seeds: spec.seeds.voice_seeds(spec.voice),
            triggers: triggers.into_boxed_slice(),
            segments,
            end,
            slices,
        }
    }

    /// Unique within the process.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The output rate.
    pub fn rate(&self) -> SampleRate {
        self.rate
    }

    /// Whether the voice takes the phrase lane, and so may read curves.
    pub fn phrase(&self) -> bool {
        self.phrase
    }

    /// The track's role indices the voice takes.
    pub fn role_mask(&self) -> u128 {
        self.role_mask
    }

    /// What `reset` receives. Never folds region rolls.
    pub fn voice_seeds(&self) -> VoiceSeeds {
        self.voice_seeds
    }

    /// Sorted by `at`; ties keep canonical order.
    pub fn triggers(&self) -> &[BoundTrigger] {
        &self.triggers
    }

    /// Strictly increasing starts; `segments()[0].start == 0`.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The last `quiet_from`; an offline render needs `[0, end)`.
    pub fn end(&self) -> RenderPos {
        self.end
    }

    /// Empty for an unknown id; never panics.
    pub fn slice(&self, id: SliceId) -> &[f32] {
        self.slices.get(id)
    }

    pub fn slices(&self) -> &SliceBank {
        &self.slices
    }

    /// The curves from `pos` on, at the output rate. `None` for a hit-only
    /// voice (law 7).
    pub fn cursor(&self, pos: RenderPos) -> Option<ControlCursor<'_>> {
        self.phrase.then(|| ControlCursor {
            track: &self.track,
            clock: ControlClock::at(self.rate, pos.0),
        })
    }

    /// The segment containing `pos`: the last one starting at or before it.
    pub fn segment_at(&self, pos: RenderPos) -> usize {
        self.segments
            .partition_point(|s| s.start <= pos)
            .saturating_sub(1)
    }

    /// Where a partial re-render starting at `pos` begins.
    pub fn reset_point_before(&self, pos: RenderPos) -> RenderPos {
        self.segments
            .get(self.segment_at(pos))
            .map_or(RenderPos(0), |s| s.start)
    }
}

/// Step 1: the voice's events, with `at` from `rescale_ceil`, hit `dur` from
/// `rescale_round` (at least 1) and seeds from `trigger_seeds`. Also returns
/// each trigger's timeline `at` and timeline `dur` (0 for gate events).
fn bind_triggers(
    track: &ControlTrack,
    spec: &BindSpec<'_>,
    role_mask: u128,
) -> (Vec<BoundTrigger>, Vec<(u64, u64)>) {
    let (rate, out, phrase) = (track.rate(), spec.out_rate, spec.route.phrase);
    // Ordinals count every earlier event of the track with the same (lane,
    // anchor), routed to this voice or not, so routing never moves a seed.
    let mut ordinals: BTreeMap<(Lane, TimelinePos), u32> = BTreeMap::new();
    let mut triggers = Vec::new();
    let mut timeline = Vec::new();
    for e in track.events() {
        let lane = e.kind.lane();
        let count = ordinals.entry((lane, e.anchor)).or_insert(0);
        let ordinal = *count;
        *count = count.saturating_add(1);
        let (kind, dur) = match e.kind {
            EventKind::NoteOn { vel, .. } if phrase => (BoundKind::NoteOn { vel, slice: None }, 0),
            EventKind::Retrigger { vel, .. } if phrase => {
                (BoundKind::Retrigger { vel, slice: None }, 0)
            }
            EventKind::NoteOff { .. } if phrase => (BoundKind::NoteOff, 0),
            EventKind::Hit { hit } => {
                let Some(h) = track.hits().get(hit as usize) else {
                    continue;
                };
                if role_mask & role_bit(h.role) == 0 {
                    continue;
                }
                let kind = BoundKind::Hit {
                    role: h.role,
                    vel: h.vel,
                    bright: h.bright,
                    breath: h.breath,
                    mouth: h.mouth,
                    dur: rescale_round(h.dur, rate, out).max(1),
                    slice: None,
                };
                (kind, h.dur)
            }
            EventKind::NoteOn { .. } | EventKind::Retrigger { .. } | EventKind::NoteOff { .. } => {
                continue;
            }
        };
        let (at, lead) = rescale_ceil(e.at.0, rate, out);
        triggers.push(BoundTrigger {
            at: RenderPos(at),
            lead,
            anchor: e.anchor,
            seeds: spec
                .seeds
                .trigger_seeds(spec.voice, lane, e.anchor, ordinal),
            kind,
        });
        timeline.push((e.at.0, dur));
    }
    (triggers, timeline)
}

/// Step 2: when the backend reads slices and the track has a take, a hit reads
/// `slice_span(anchor, min(dur, slice_max))` and a NoteOn or Retrigger reads
/// `slice_span(anchor, min(time to the voice's next phrase event,
/// slice_max))`. A slice never outlives its event.
fn bind_slices(
    track: &ControlTrack,
    spec: &BindSpec<'_>,
    triggers: &mut [BoundTrigger],
    timeline: &[(u64, u64)],
    source: &mut dyn SliceSource,
) -> SliceBank {
    let Some(take) = track.take() else {
        return SliceBank::default();
    };
    if spec.info.slice_max_ms == 0 {
        return SliceBank::default();
    }
    let max = track.rate().samples_for_ms(spec.info.slice_max_ms);
    let mut wanted: Vec<(usize, TimelineSpan)> = Vec::new();
    let mut next_phrase: Option<u64> = None;
    for (i, (trig, &(at, dur))) in triggers.iter().zip(timeline).enumerate().rev() {
        let len = match trig.kind {
            BoundKind::Hit { .. } => Some(dur),
            BoundKind::NoteOn { .. } | BoundKind::Retrigger { .. } => {
                Some(next_phrase.map_or(max, |n| n.saturating_sub(at)))
            }
            BoundKind::NoteOff => None,
        };
        if !matches!(trig.kind, BoundKind::Hit { .. }) {
            next_phrase = Some(at);
        }
        if let Some(span) = len.and_then(|len| track.slice_span(trig.anchor, len.min(max))) {
            wanted.push((i, span));
        }
    }
    let spans: Vec<TimelineSpan> = wanted.iter().map(|w| w.1).collect();
    let (bank, ids) = SliceBank::build(take, &spans, track.rate(), spec.out_rate, source);
    for ((i, _), id) in wanted.into_iter().zip(ids) {
        triggers[i].kind.set_slice(id);
    }
    bank
}

/// The curves read incrementally at the output rate: equal, bit for bit, to
/// `ControlTrack::value_at` at every absolute sample.
#[derive(Clone, Copy)]
pub struct ControlCursor<'a> {
    track: &'a ControlTrack,
    clock: ControlClock,
}

impl fmt::Debug for ControlCursor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlCursor")
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}

impl ControlCursor<'_> {
    #[inline]
    pub fn get(&self) -> ControlFrame {
        self.track.at_clock(&self.clock)
    }

    /// One output sample on.
    #[inline]
    pub fn advance(&mut self) {
        self.clock.advance();
    }
}
