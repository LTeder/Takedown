//! ControlTrack, the only renderer input (core-types §2.7).
//!
//! Six curves on the 200 Hz grid, a monophonic phrase lane of gate spans, and a
//! polyphonic hit lane. The gate is exactly the union of the spans: there is no
//! gate curve, so the gate and NoteOn/NoteOff cannot disagree. Gate events carry
//! no pitch; a backend reads pitch from the curve, so the renderer never sees
//! notes.
//!
//! `reattach_anchors` arrives with the midi step.

use std::fmt;
use std::iter::FusedIterator;

use serde::{Deserialize, Serialize};

use crate::hit::RoleName;
use crate::ids::{ControlId, TakeId, context};
use crate::key::{KeyError, KeyHash, KeyWriter, RawHasher};
use crate::migrate::{self, DocumentError, Step};
use crate::time::{
    ControlClock, SampleRate, TimelinePos, TimelineSpan, control_index, frames_covering,
};

/// The amplitude floor of `amp_db`.
pub const AMP_FLOOR_DB: f32 = -120.0;
/// The longest hit.
pub const MAX_HIT_MS: u32 = 60_000;
/// The most roles a track can name.
pub const MAX_ROLES: usize = 128;
/// The longest track, in samples: 2^40.
pub const MAX_LEN: u64 = 1 << 40;
/// The schema version of `controls/*.json`.
pub const CONTROL_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Curve {
    PitchSt,
    AmpDb,
    Bright,
    Breath,
    VowelX,
    VowelY,
}

impl Curve {
    pub const ALL: [Curve; 6] = [
        Curve::PitchSt,
        Curve::AmpDb,
        Curve::Bright,
        Curve::Breath,
        Curve::VowelX,
        Curve::VowelY,
    ];

    /// The inclusive range a stored value must lie in.
    pub const fn range(self) -> (f32, f32) {
        match self {
            Self::PitchSt => (-64.0, 192.0),
            Self::AmpDb => (AMP_FLOOR_DB, 12.0),
            Self::Bright | Self::Breath => (0.0, 1.0),
            Self::VowelX | Self::VowelY => (-1.0, 1.0),
        }
    }
}

/// Six curves on the 200 Hz grid, each `frames_covering(len, rate)` long (at
/// least 1). Ranges: `pitch_st` is MIDI semitones (69 = A4) in [-64, 192];
/// `amp_db` in [`AMP_FLOOR_DB`, 12]; `bright` and `breath` in [0, 1];
/// `vowel_x` and `vowel_y` in [-1, 1]. Values outside spans are whatever the
/// lens wrote; MIDI fills them canonically.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curves {
    pub pitch_st: Vec<f32>,
    pub amp_db: Vec<f32>,
    pub bright: Vec<f32>,
    pub breath: Vec<f32>,
    pub vowel_x: Vec<f32>,
    pub vowel_y: Vec<f32>,
}

impl Curves {
    pub fn get(&self, c: Curve) -> &[f32] {
        match c {
            Curve::PitchSt => &self.pitch_st,
            Curve::AmpDb => &self.amp_db,
            Curve::Bright => &self.bright,
            Curve::Breath => &self.breath,
            Curve::VowelX => &self.vowel_x,
            Curve::VowelY => &self.vowel_y,
        }
    }

    pub fn get_mut(&mut self, c: Curve) -> &mut Vec<f32> {
        match c {
            Curve::PitchSt => &mut self.pitch_st,
            Curve::AmpDb => &mut self.amp_db,
            Curve::Bright => &mut self.bright,
            Curve::Breath => &mut self.breath,
            Curve::VowelX => &mut self.vowel_x,
            Curve::VowelY => &mut self.vowel_y,
        }
    }
}

/// The curves' values at one instant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlFrame {
    pub pitch_st: f32,
    pub amp_db: f32,
    pub bright: f32,
    pub breath: f32,
    pub vowel: [f32; 2],
}

/// A 7-bit value, 0..=127.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct U7(u8);

/// A note velocity, 1..=127.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Velocity(u8);

/// A byte outside the range of [`U7`] or [`Velocity`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRangeError(pub u8);

impl fmt::Display for ByteRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is out of range", self.0)
    }
}

impl std::error::Error for ByteRangeError {}

impl U7 {
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 127 { Some(Self(v)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Velocity {
    pub const fn new(v: u8) -> Option<Self> {
        if matches!(v, 1..=127) {
            Some(Self(v))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for U7 {
    type Error = ByteRangeError;

    fn try_from(v: u8) -> Result<Self, ByteRangeError> {
        Self::new(v).ok_or(ByteRangeError(v))
    }
}

impl TryFrom<u8> for Velocity {
    type Error = ByteRangeError;

    fn try_from(v: u8) -> Result<Self, ByteRangeError> {
        Self::new(v).ok_or(ByteRangeError(v))
    }
}

impl From<U7> for u8 {
    fn from(v: U7) -> u8 {
        v.0
    }
}

impl From<Velocity> for u8 {
    fn from(v: Velocity) -> u8 {
        v.0
    }
}

impl KeyHash for U7 {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.int(i128::from(self.0));
        Ok(())
    }
}

impl KeyHash for Velocity {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.int(i128::from(self.0));
        Ok(())
    }
}

/// How a span joins the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    /// A gap comes before this span, or it is the first span.
    Detached,
    /// Abuts the previous span: envelopes restart, the gate never falls.
    Retrigger,
    /// Abuts: nothing restarts; a notational boundary for NoteView and MIDI only.
    Legato,
}

/// The two lanes of a track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Phrase,
    Hits,
}

impl Lane {
    /// The frozen seed-derivation label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Phrase => "lane/phrase",
            Self::Hits => "lane/hits",
        }
    }
}

/// One span of the monophonic phrase lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateSpan {
    pub on: TimelinePos,
    pub off: TimelinePos,
    /// Identity: the evidence onset before anything moved it.
    pub anchor: TimelinePos,
    pub vel: Velocity,
    pub link: Link,
}

/// One event of the polyphonic hit lane. Every field is something MIDI
/// carries, so the lane is self-contained: hit voices read these fields, a
/// slice and seeds, never curves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitEvent {
    /// When it fires, after timing feel and perceptual-centre pre-roll.
    pub at: TimelinePos,
    /// Identity: the evidence onset before anything moved it; seeds and the
    /// take slice key on it.
    pub anchor: TimelinePos,
    /// Samples; drives decay; the MIDI note length.
    pub dur: u64,
    /// Index into the track's roles.
    pub role: u8,
    /// Accent, from loudness.
    pub vel: Velocity,
    /// Tone.
    pub bright: U7,
    /// Noise amount.
    pub breath: U7,
    /// How much of the take's own transient strikes the body.
    pub mouth: U7,
}

impl HitEvent {
    /// The canonical order: (at, role, anchor, vel, bright, breath, mouth, dur).
    pub fn canonical_key(&self) -> (TimelinePos, u8, TimelinePos, Velocity, U7, U7, U7, u64) {
        (
            self.at,
            self.role,
            self.anchor,
            self.vel,
            self.bright,
            self.breath,
            self.mouth,
            self.dur,
        )
    }
}

/// The unvalidated fields of a [`ControlTrack`], and its serialised form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackParts {
    /// Timeline rate.
    pub rate: SampleRate,
    /// Samples, 1..=2^40.
    pub len: u64,
    /// Where slices come from; when `Some`, `rate` equals that take's rate.
    pub take: Option<TakeId>,
    /// Sorted, unique, at most [`MAX_ROLES`].
    pub roles: Vec<RoleName>,
    pub curves: Curves,
    pub spans: Vec<GateSpan>,
    pub hits: Vec<HitEvent>,
}

/// The only renderer input. It can exist only validated: serde goes through
/// [`TrackParts`]. A deserialised track must already be canonical (sorted
/// hits); [`ControlTrack::new`] sorts them for callers that build tracks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TrackParts", into = "TrackParts")]
pub struct ControlTrack {
    rate: SampleRate,
    len: u64,
    take: Option<TakeId>,
    roles: Vec<RoleName>,
    curves: Curves,
    spans: Vec<GateSpan>,
    hits: Vec<HitEvent>,
}

/// One way a track breaks its invariants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackError {
    /// `len` outside 1..=2^40.
    Length,
    /// A curve whose length is not `frames_covering(len, rate)`.
    CurveLength { curve: Curve },
    /// A non-finite or out-of-range value.
    OutOfRange { curve: Curve, frame: u64 },
    /// Span `i` has `off <= on`.
    EmptySpan(usize),
    /// Span `i` starts before span `i - 1` ends.
    SpanOrder(usize),
    /// Span `i` ends after `len`.
    SpanBeyondEnd(usize),
    /// Span `i`'s link disagrees with the gap before it.
    LinkMismatch(usize),
    /// Hit `i` has `dur` of 0 or over [`MAX_HIT_MS`].
    HitDuration(usize),
    /// Hit `i` ends after `len`, or `at + dur` overflows.
    HitBeyondEnd(usize),
    /// Hit `i` is before hit `i - 1` in canonical order.
    HitOrder(usize),
    /// Hit `i` names a role the track does not have.
    RoleIndex(usize),
    /// Roles not sorted and unique, or more than [`MAX_ROLES`].
    Roles,
    /// An anchor after `len`.
    AnchorBeyondEnd,
}

impl fmt::Display for TrackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => write!(f, "track length outside 1..={MAX_LEN} samples"),
            Self::CurveLength { curve } => write!(f, "curve {curve:?} has the wrong length"),
            Self::OutOfRange { curve, frame } => {
                write!(
                    f,
                    "curve {curve:?} is non-finite or out of range at frame {frame}"
                )
            }
            Self::EmptySpan(i) => write!(f, "span {i} is empty"),
            Self::SpanOrder(i) => write!(f, "span {i} overlaps or precedes span {}", i - 1),
            Self::SpanBeyondEnd(i) => write!(f, "span {i} ends after the track"),
            Self::LinkMismatch(i) => write!(f, "span {i} has a link that disagrees with its gap"),
            Self::HitDuration(i) => {
                write!(f, "hit {i} has a duration of 0 or over {MAX_HIT_MS} ms")
            }
            Self::HitBeyondEnd(i) => write!(f, "hit {i} ends after the track"),
            Self::HitOrder(i) => write!(f, "hit {i} is out of canonical order"),
            Self::RoleIndex(i) => write!(f, "hit {i} names a role the track does not have"),
            Self::Roles => write!(
                f,
                "roles are not sorted and unique, or more than {MAX_ROLES}"
            ),
            Self::AnchorBeyondEnd => write!(f, "an anchor lies after the track"),
        }
    }
}

impl std::error::Error for TrackError {}

fn validate(p: &TrackParts) -> Result<(), TrackError> {
    let TrackParts {
        rate,
        len,
        take: _,
        roles,
        curves,
        spans,
        hits,
    } = p;
    let (rate, len) = (*rate, *len);
    if !(1..=MAX_LEN).contains(&len) {
        return Err(TrackError::Length);
    }
    if roles.len() > MAX_ROLES || roles.windows(2).any(|w| w[0] >= w[1]) {
        return Err(TrackError::Roles);
    }
    let frames = frames_covering(len, rate);
    for curve in Curve::ALL {
        let values = curves.get(curve);
        if values.len() as u64 != frames {
            return Err(TrackError::CurveLength { curve });
        }
        let (lo, hi) = curve.range();
        // NaN is in no range; infinities fail the bounds.
        if let Some(frame) = values.iter().position(|v| !(lo..=hi).contains(v)) {
            return Err(TrackError::OutOfRange {
                curve,
                frame: frame as u64,
            });
        }
    }
    for (i, s) in spans.iter().enumerate() {
        if s.off <= s.on {
            return Err(TrackError::EmptySpan(i));
        }
        let prev = i.checked_sub(1).map(|j| spans[j]);
        if prev.is_some_and(|p| s.on < p.off) {
            return Err(TrackError::SpanOrder(i));
        }
        if s.off.0 > len {
            return Err(TrackError::SpanBeyondEnd(i));
        }
        let detached = prev.is_none_or(|p| s.on > p.off);
        if detached != (s.link == Link::Detached) {
            return Err(TrackError::LinkMismatch(i));
        }
        if s.anchor.0 > len {
            return Err(TrackError::AnchorBeyondEnd);
        }
    }
    let max_dur = rate.samples_for_ms(MAX_HIT_MS);
    for (i, h) in hits.iter().enumerate() {
        if h.dur == 0 || h.dur > max_dur {
            return Err(TrackError::HitDuration(i));
        }
        if h.at.0.checked_add(h.dur).is_none_or(|end| end > len) {
            return Err(TrackError::HitBeyondEnd(i));
        }
        if usize::from(h.role) >= roles.len() {
            return Err(TrackError::RoleIndex(i));
        }
        if i > 0 && hits[i - 1].canonical_key() > h.canonical_key() {
            return Err(TrackError::HitOrder(i));
        }
        if h.anchor.0 > len {
            return Err(TrackError::AnchorBeyondEnd);
        }
    }
    Ok(())
}

impl TryFrom<TrackParts> for ControlTrack {
    type Error = TrackError;

    /// The load path: validates without sorting, so a file must be canonical.
    fn try_from(parts: TrackParts) -> Result<Self, TrackError> {
        validate(&parts)?;
        let TrackParts {
            rate,
            len,
            take,
            roles,
            curves,
            spans,
            hits,
        } = parts;
        Ok(Self {
            rate,
            len,
            take,
            roles,
            curves,
            spans,
            hits,
        })
    }
}

impl From<ControlTrack> for TrackParts {
    fn from(t: ControlTrack) -> TrackParts {
        t.into_parts()
    }
}

impl ControlTrack {
    /// Sorts hits canonically, then validates.
    pub fn new(mut parts: TrackParts) -> Result<Self, TrackError> {
        parts.hits.sort_unstable_by_key(HitEvent::canonical_key);
        Self::try_from(parts)
    }

    pub fn into_parts(self) -> TrackParts {
        let Self {
            rate,
            len,
            take,
            roles,
            curves,
            spans,
            hits,
        } = self;
        TrackParts {
            rate,
            len,
            take,
            roles,
            curves,
            spans,
            hits,
        }
    }

    pub fn rate(&self) -> SampleRate {
        self.rate
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    /// Never true: a track is at least one sample long.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn take(&self) -> Option<TakeId> {
        self.take
    }

    pub fn roles(&self) -> &[RoleName] {
        &self.roles
    }

    pub fn curves(&self) -> &Curves {
        &self.curves
    }

    pub fn spans(&self) -> &[GateSpan] {
        &self.spans
    }

    pub fn hits(&self) -> &[HitEvent] {
        &self.hits
    }

    /// The length of every curve.
    pub fn frames(&self) -> u64 {
        self.curves.pitch_st.len() as u64
    }

    /// The curves at absolute sample `pos` at `rate` (any rate): a pure
    /// function of the position. Samples on the grid return stored values bit
    /// for bit; past the last frame the last value holds.
    pub fn value_at(&self, rate: SampleRate, pos: u64) -> ControlFrame {
        let (frame, rem) = control_index(pos, rate);
        self.sample(frame, rem as f32 / rate.hz() as f32)
    }

    /// The same value as [`value_at`](Self::value_at) through an incremental clock.
    pub fn at_clock(&self, clock: &ControlClock) -> ControlFrame {
        self.sample(clock.frame(), clock.frac())
    }

    #[inline]
    fn sample(&self, frame: u64, frac: f32) -> ControlFrame {
        let c = &self.curves;
        let last = c.pitch_st.len() - 1;
        // frac is +0.0 exactly on the grid; returning the stored value there
        // keeps -0.0 and avoids any rounding.
        if frame >= last as u64 || frac.to_bits() == 0 {
            let i = frame.min(last as u64) as usize;
            return ControlFrame {
                pitch_st: c.pitch_st[i],
                amp_db: c.amp_db[i],
                bright: c.bright[i],
                breath: c.breath[i],
                vowel: [c.vowel_x[i], c.vowel_y[i]],
            };
        }
        let i = frame as usize;
        let lerp = |v: &[f32]| v[i] + (v[i + 1] - v[i]) * frac;
        ControlFrame {
            pitch_st: lerp(&c.pitch_st),
            amp_db: lerp(&c.amp_db),
            bright: lerp(&c.bright),
            breath: lerp(&c.breath),
            vowel: [lerp(&c.vowel_x), lerp(&c.vowel_y)],
        }
    }

    /// The index of the span containing `pos`.
    pub fn gate_at(&self, pos: TimelinePos) -> Option<usize> {
        let i = self.spans.partition_point(|s| s.on <= pos);
        let i = i.checked_sub(1)?;
        (pos < self.spans[i].off).then_some(i)
    }

    /// Every event in canonical order; allocates nothing.
    pub fn events(&self) -> EventIter<'_> {
        let mut it = EventIter {
            spans: &self.spans,
            hits: &self.hits,
            span: 0,
            at_off: false,
            hit: 0,
        };
        it.settle();
        it
    }

    /// `[anchor, anchor + len)` clipped to the track; `None` without a take.
    pub fn slice_span(&self, anchor: TimelinePos, len: u64) -> Option<TimelineSpan> {
        self.take?;
        let start = anchor.0.min(self.len);
        let end = anchor.0.saturating_add(len).min(self.len);
        TimelineSpan::new(TimelinePos(start), TimelinePos(end))
    }

    /// Raw framing of every field.
    pub fn content_id(&self) -> ControlId {
        let Self {
            rate,
            len,
            take,
            roles,
            curves,
            spans,
            hits,
        } = self;
        let mut h = RawHasher::new(context::CONTROL);
        h.u32(rate.hz());
        h.u64(*len);
        match take {
            Some(t) => {
                h.u8(1);
                h.raw(&t.hash32().0);
            }
            None => h.u8(0),
        }
        h.count(roles.len());
        for r in roles {
            h.str(r.as_str());
        }
        let Curves {
            pitch_st,
            amp_db,
            bright,
            breath,
            vowel_x,
            vowel_y,
        } = curves;
        for c in [pitch_st, amp_db, bright, breath, vowel_x, vowel_y] {
            h.f32s(c);
        }
        h.count(spans.len());
        for s in spans {
            let GateSpan {
                on,
                off,
                anchor,
                vel,
                link,
            } = s;
            h.u64(on.0);
            h.u64(off.0);
            h.u64(anchor.0);
            h.u8(vel.get());
            h.u8(*link as u8);
        }
        h.count(hits.len());
        for e in hits {
            let HitEvent {
                at,
                anchor,
                dur,
                role,
                vel,
                bright,
                breath,
                mouth,
            } = e;
            h.u64(at.0);
            h.u64(anchor.0);
            h.u64(*dur);
            h.u8(*role);
            h.u8(vel.get());
            h.u8(bright.get());
            h.u8(breath.get());
            h.u8(mouth.get());
        }
        ControlId::from_hash(h.finish())
    }
}

/// One event of a track, with its firing time and its identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub at: TimelinePos,
    pub anchor: TimelinePos,
    pub kind: EventKind,
}

/// Gate events carry no pitch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    NoteOn { span: u32, vel: Velocity },
    Retrigger { span: u32, vel: Velocity },
    NoteOff { span: u32 },
    Hit { hit: u32 },
}

impl EventKind {
    pub const fn lane(self) -> Lane {
        match self {
            Self::NoteOn { .. } | Self::Retrigger { .. } | Self::NoteOff { .. } => Lane::Phrase,
            Self::Hit { .. } => Lane::Hits,
        }
    }
}

/// The events of a track in canonical order: by `at`; at equal positions,
/// phrase events before hits, and hits in canonical order. At `on` a span emits
/// `NoteOn` if Detached, `Retrigger` if Retrigger, nothing if Legato; at `off`
/// it emits `NoteOff` unless the next span abuts it.
#[derive(Clone, Debug)]
pub struct EventIter<'a> {
    spans: &'a [GateSpan],
    hits: &'a [HitEvent],
    /// The span of the next phrase event.
    span: usize,
    /// Whether the next phrase event is that span's `off` rather than its `on`.
    at_off: bool,
    hit: usize,
}

impl EventIter<'_> {
    /// Moves the phrase cursor onto the next event that is actually emitted.
    fn settle(&mut self) {
        while let Some(s) = self.spans.get(self.span) {
            if !self.at_off {
                if s.link == Link::Legato {
                    self.at_off = true;
                    continue;
                }
                return;
            }
            match self.spans.get(self.span + 1) {
                Some(next) if next.on == s.off => {
                    self.span += 1;
                    self.at_off = false;
                }
                _ => return,
            }
        }
    }

    fn phrase(&self) -> Option<Event> {
        let s = self.spans.get(self.span)?;
        let span = self.span as u32;
        Some(if self.at_off {
            Event {
                at: s.off,
                anchor: s.anchor,
                kind: EventKind::NoteOff { span },
            }
        } else {
            let kind = match s.link {
                Link::Retrigger => EventKind::Retrigger { span, vel: s.vel },
                Link::Detached | Link::Legato => EventKind::NoteOn { span, vel: s.vel },
            };
            Event {
                at: s.on,
                anchor: s.anchor,
                kind,
            }
        })
    }
}

impl Iterator for EventIter<'_> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let phrase = self.phrase();
        let hit = self.hits.get(self.hit);
        match (phrase, hit) {
            (Some(p), h) if h.is_none_or(|h| p.at <= h.at) => {
                if self.at_off {
                    self.span += 1;
                    self.at_off = false;
                } else {
                    self.at_off = true;
                }
                self.settle();
                Some(p)
            }
            (_, Some(h)) => {
                let index = self.hit as u32;
                self.hit += 1;
                Some(Event {
                    at: h.at,
                    anchor: h.anchor,
                    kind: EventKind::Hit { hit: index },
                })
            }
            _ => None,
        }
    }
}

impl FusedIterator for EventIter<'_> {}

/// `controls/<ControlId>.json`: an imported track as a versioned document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlFile {
    pub schema_version: u32,
    pub track: ControlTrack,
}

impl ControlFile {
    /// `track` as a document of the current schema.
    pub fn new(track: ControlTrack) -> Self {
        Self {
            schema_version: CONTROL_SCHEMA_VERSION,
            track,
        }
    }

    /// Compact and deterministic, written by [`crate::project::json`] so
    /// every f32 reads back exactly.
    pub fn to_json(&self) -> Vec<u8> {
        // A track holds integers, ids, role names and finite f32s.
        crate::project::json::to_vec(self).expect("a control file serialises")
    }

    /// Reads the version (required) -> TooNew? -> typed decode, which
    /// validates the track. No migration step exists yet, so an older
    /// version is a `Gap`.
    pub fn from_json(bytes: &[u8]) -> Result<Self, DocumentError> {
        migrate::load(bytes, CONTROL_STEPS, CONTROL_SCHEMA_VERSION).map(|(file, _)| file)
    }
}

/// The migration chain of `controls/*.json`: empty at schema 1.
pub const CONTROL_STEPS: &[Step] = &[];
