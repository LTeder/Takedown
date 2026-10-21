//! Time base (core-types §2.1): integer samples at the timeline rate, a fixed
//! 200 Hz control grid, and exact conversions between them.
//!
//! All arithmetic is u128 and saturates at `u64::MAX`; nothing here panics.
//! Interpolation between frames is `a + (b - a) * frac` in f32, linear in
//! semitones, in dB and on the unit range. f64 appears only in [`seconds`],
//! which is for display and logs and is never fed back.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::key::{KeyError, KeyHash, KeyWriter};

/// Frame `k` sits at `k / 200` s, independent of any sample rate. Never changes.
pub const CONTROL_HZ: u32 = 200;
/// 50 ms: the reset gap of principle 8.
pub const MIN_GAP_FRAMES: u64 = 10;
/// At least 5 samples per frame, so the clock carries at most once per sample.
pub const MIN_RATE_HZ: u32 = 1_000;
/// Below 2^24, so `rem` and `rate` are exact in f32.
pub const MAX_RATE_HZ: u32 = 1_536_000;

/// A sample rate in [`MIN_RATE_HZ`, `MAX_RATE_HZ`], validated on construction
/// and on load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct SampleRate(u32);

impl SampleRate {
    pub const fn new(hz: u32) -> Option<Self> {
        if matches!(hz, MIN_RATE_HZ..=MAX_RATE_HZ) {
            Some(Self(hz))
        } else {
            None
        }
    }

    pub const fn hz(self) -> u32 {
        self.0
    }

    /// `ms` milliseconds in samples, rounded half up.
    pub fn samples_for_ms(self, ms: u32) -> u64 {
        (u64::from(ms) * u64::from(self.0) + 500) / 1000
    }
}

/// A sample rate outside the accepted range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateError(pub u32);

impl fmt::Display for RateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sample rate {} Hz is outside {MIN_RATE_HZ}..={MAX_RATE_HZ} Hz",
            self.0
        )
    }
}

impl std::error::Error for RateError {}

impl TryFrom<u32> for SampleRate {
    type Error = RateError;

    fn try_from(hz: u32) -> Result<Self, RateError> {
        Self::new(hz).ok_or(RateError(hz))
    }
}

impl From<SampleRate> for u32 {
    fn from(rate: SampleRate) -> u32 {
        rate.0
    }
}

/// A position at the timeline rate: the take's capture rate, or the declared
/// rate of a track with no take. Used by evidence, hits, edits, ControlTrack
/// and seeds.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct TimelinePos(pub u64);

/// A position at a backend's output rate. Used only by `render::contract` and
/// backends.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct RenderPos(pub u64);

/// Half-open `[start, end)` with `start <= end`, at the timeline rate.
/// Serialised as `[start, end]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "[u64; 2]", into = "[u64; 2]")]
pub struct TimelineSpan {
    start: TimelinePos,
    end: TimelinePos,
}

impl TimelineSpan {
    /// `None` when `start > end`.
    pub const fn new(start: TimelinePos, end: TimelinePos) -> Option<Self> {
        if start.0 <= end.0 {
            Some(Self { start, end })
        } else {
            None
        }
    }

    pub const fn start(self) -> TimelinePos {
        self.start
    }

    pub const fn end(self) -> TimelinePos {
        self.end
    }

    pub const fn len(self) -> u64 {
        self.end.0 - self.start.0
    }

    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }

    pub const fn contains(self, pos: TimelinePos) -> bool {
        self.start.0 <= pos.0 && pos.0 < self.end.0
    }
}

/// A serialised span whose start is after its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanError {
    pub start: u64,
    pub end: u64,
}

impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "span [{}, {}) starts after it ends",
            self.start, self.end
        )
    }
}

impl std::error::Error for SpanError {}

impl TryFrom<[u64; 2]> for TimelineSpan {
    type Error = SpanError;

    fn try_from([start, end]: [u64; 2]) -> Result<Self, SpanError> {
        Self::new(TimelinePos(start), TimelinePos(end)).ok_or(SpanError { start, end })
    }
}

impl From<TimelineSpan> for [u64; 2] {
    fn from(s: TimelineSpan) -> [u64; 2] {
        [s.start.0, s.end.0]
    }
}

fn saturate(v: u128) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

/// `(frame, rem)` with `frame * rate + rem == pos * 200` and `rem < rate`:
/// the frame containing sample `pos`, and how far into it the sample lies.
pub fn control_index(pos: u64, rate: SampleRate) -> (u64, u32) {
    let t = u128::from(pos) * u128::from(CONTROL_HZ);
    let r = u128::from(rate.0);
    // rem < rate <= MAX_RATE_HZ, so the cast is exact.
    (saturate(t / r), (t % r) as u32)
}

/// `ceil(frame * rate / 200)`: the first sample of the frame.
pub fn frame_start(frame: u64, rate: SampleRate) -> u64 {
    saturate((u128::from(frame) * u128::from(rate.0)).div_ceil(u128::from(CONTROL_HZ)))
}

/// `ceil(samples * 200 / rate)`: the number of frames whose time lies before
/// `samples / rate` seconds.
pub fn frames_covering(samples: u64, rate: SampleRate) -> u64 {
    saturate((u128::from(samples) * u128::from(CONTROL_HZ)).div_ceil(u128::from(rate.0)))
}

/// `pos` rescaled from `from` to `to`, rounded half up. With `to >= from`,
/// rescaling up and then back down is the identity.
pub fn rescale_round(pos: u64, from: SampleRate, to: SampleRate) -> u64 {
    let n = u128::from(pos) * u128::from(to.0);
    let d = u128::from(from.0);
    saturate((2 * n + d) / (2 * d))
}

/// The first sample at `to` at or after the exact time of `pos` at `from`,
/// plus `lead`, the distance from that time to the sample in [0, 1) output
/// samples. `lead` is one correctly rounded f32 division of exact integers.
pub fn rescale_ceil(pos: u64, from: SampleRate, to: SampleRate) -> (u64, f32) {
    let n = u128::from(pos) * u128::from(to.0);
    let d = u128::from(from.0);
    let q = n.div_ceil(d);
    match u64::try_from(q) {
        // q * d - n < d <= MAX_RATE_HZ < 2^24: exact in f32, and the quotient
        // rounds below 1.
        Ok(q64) => (q64, (q * d - n) as f32 / from.0 as f32),
        Err(_) => (u64::MAX, 0.0),
    }
}

/// Seconds, for display and logs only. Never fed back into positions.
pub fn seconds(pos: u64, rate: SampleRate) -> f64 {
    pos as f64 / f64::from(rate.0)
}

/// Exact incremental clock over the control grid: the state after `n` calls
/// to [`advance`](Self::advance) from `at(rate, p)` equals `at(rate, p + n)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlClock {
    rate: u32,
    frame: u64,
    rem: u32,
}

impl ControlClock {
    pub fn at(rate: SampleRate, pos: u64) -> Self {
        let (frame, rem) = control_index(pos, rate);
        Self {
            rate: rate.0,
            frame,
            rem,
        }
    }

    /// One sample on. Allocation-free and branch-light; carries at most once
    /// because the rate is at least five times the control rate.
    #[inline]
    pub fn advance(&mut self) {
        self.rem += CONTROL_HZ;
        if self.rem >= self.rate {
            self.rem -= self.rate;
            self.frame = self.frame.saturating_add(1);
        }
    }

    #[inline]
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// `rem / rate`, one correctly rounded f32 division; exactly 0.0 on the grid.
    #[inline]
    pub fn frac(&self) -> f32 {
        self.rem as f32 / self.rate as f32
    }
}

impl KeyHash for SampleRate {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.int(i128::from(self.0));
        Ok(())
    }
}

impl KeyHash for TimelinePos {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.int(i128::from(self.0));
        Ok(())
    }
}

impl KeyHash for TimelineSpan {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { start, end } = self;
        w.record(|r| {
            r.field("start", start)?;
            r.field("end", end)
        })
    }
}
