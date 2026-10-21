//! Musical time (core-types §2.9): a lens over integer SMF time. No f64 is
//! stored; samples and ticks convert with exact u128 rationals, piecewise
//! across tempo changes.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::time::{CONTROL_HZ, SampleRate, TimelinePos};

/// The slowest tempo SMF can carry, in microseconds per quarter.
pub const MAX_US_PER_QUARTER: u32 = 0xFF_FFFF;
/// The finest PPQ SMF can carry.
pub const MAX_PPQ: u16 = 32_767;

/// A tempo from `tick` on, in SMF's own unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoSegment {
    pub tick: u64,
    pub us_per_quarter: u32,
}

/// Non-empty, first tick 0, strictly increasing ticks, tempi in 1..=0xFF_FFFF.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<TempoSegment>", into = "Vec<TempoSegment>")]
pub struct TempoMap(Vec<TempoSegment>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeMapError {
    /// No tempo segment.
    Empty,
    /// The first segment does not start at tick 0.
    FirstTick,
    /// Segment `i` does not start after segment `i - 1`.
    TickOrder(usize),
    /// Segment `i`'s tempo is outside 1..=0xFF_FFFF.
    Tempo(usize),
    /// A BPM that is not finite and positive, or maps outside the tempo range.
    Bpm(f64),
    /// PPQ outside 1..=32767.
    Ppq(u16),
    /// A tick longer than 5 ms or not longer than one sample at the rate.
    NotExportable,
}

impl fmt::Display for TimeMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("tempo map is empty"),
            Self::FirstTick => f.write_str("tempo map does not start at tick 0"),
            Self::TickOrder(i) => write!(f, "tempo segment {i} does not follow the one before"),
            Self::Tempo(i) => write!(
                f,
                "tempo segment {i} is outside 1..={MAX_US_PER_QUARTER} us"
            ),
            Self::Bpm(b) => write!(f, "{b} BPM is not a usable tempo"),
            Self::Ppq(p) => write!(f, "PPQ {p} is outside 1..={MAX_PPQ}"),
            Self::NotExportable => f.write_str("a tick must be at most 5 ms and over one sample"),
        }
    }
}

impl std::error::Error for TimeMapError {}

impl TryFrom<Vec<TempoSegment>> for TempoMap {
    type Error = TimeMapError;

    fn try_from(segments: Vec<TempoSegment>) -> Result<Self, TimeMapError> {
        let first = segments.first().ok_or(TimeMapError::Empty)?;
        if first.tick != 0 {
            return Err(TimeMapError::FirstTick);
        }
        for (i, s) in segments.iter().enumerate() {
            if !(1..=MAX_US_PER_QUARTER).contains(&s.us_per_quarter) {
                return Err(TimeMapError::Tempo(i));
            }
            if i > 0 && segments[i - 1].tick >= s.tick {
                return Err(TimeMapError::TickOrder(i));
            }
        }
        Ok(Self(segments))
    }
}

impl From<TempoMap> for Vec<TempoSegment> {
    fn from(m: TempoMap) -> Vec<TempoSegment> {
        m.0
    }
}

impl TempoMap {
    pub fn constant(us_per_quarter: u32) -> Result<Self, TimeMapError> {
        Self::try_from(vec![TempoSegment {
            tick: 0,
            us_per_quarter,
        }])
    }

    /// The one lossy step, done once: the nearest whole microsecond per quarter.
    pub fn from_bpm(bpm: f64) -> Result<Self, TimeMapError> {
        if !(bpm.is_finite() && bpm > 0.0) {
            return Err(TimeMapError::Bpm(bpm));
        }
        let us = (60_000_000.0 / bpm).round();
        if !(1.0..=f64::from(MAX_US_PER_QUARTER)).contains(&us) {
            return Err(TimeMapError::Bpm(bpm));
        }
        Self::constant(us as u32)
    }

    pub fn segments(&self) -> &[TempoSegment] {
        &self.0
    }
}

/// Timeline samples <-> ticks with exact u128 rationals, piecewise across
/// tempo changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TimeMapRepr")]
pub struct TimeMap {
    ppq: u16,
    tempo: TempoMap,
    /// The tick of timeline sample 0.
    origin_tick: u64,
}

/// The serialised form of [`TimeMap`], validated through [`TimeMap::new`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeMapRepr {
    ppq: u16,
    tempo: TempoMap,
    origin_tick: u64,
}

impl TryFrom<TimeMapRepr> for TimeMap {
    type Error = TimeMapError;

    fn try_from(r: TimeMapRepr) -> Result<Self, TimeMapError> {
        Self::new(r.ppq, r.tempo, r.origin_tick)
    }
}

impl TimeMap {
    /// PPQ 1..=32767; any SMF map can be read.
    pub fn new(ppq: u16, tempo: TempoMap, origin_tick: u64) -> Result<Self, TimeMapError> {
        if !(1..=MAX_PPQ).contains(&ppq) {
            return Err(TimeMapError::Ppq(ppq));
        }
        Ok(Self {
            ppq,
            tempo,
            origin_tick,
        })
    }

    /// 960 PPQ, 120 BPM, origin 0.
    pub fn default_export() -> Self {
        Self {
            ppq: 960,
            tempo: TempoMap(vec![TempoSegment {
                tick: 0,
                us_per_quarter: 500_000,
            }]),
            origin_tick: 0,
        }
    }

    pub fn ppq(&self) -> u16 {
        self.ppq
    }

    pub fn tempo(&self) -> &TempoMap {
        &self.tempo
    }

    pub fn origin_tick(&self) -> u64 {
        self.origin_tick
    }

    /// A tick is at most 5 ms and longer than one sample, in every tempo
    /// segment. Then every control frame gets its own tick and every tick its
    /// own nearest sample.
    pub fn check_exportable(&self, rate: SampleRate) -> Result<(), TimeMapError> {
        let ppq = u128::from(self.ppq);
        let hz = u128::from(rate.hz());
        let ok = self.tempo.segments().iter().all(|s| {
            // One tick is us_per_quarter / ppq microseconds; one sample 10^6 / hz.
            let us = u128::from(s.us_per_quarter);
            us <= MAX_TICK_US * ppq && us * hz > US_PER_S * ppq
        });
        if ok {
            Ok(())
        } else {
            Err(TimeMapError::NotExportable)
        }
    }

    /// The coarsest integer PPQ refinement that passes `check_exportable`:
    /// PPQ, every tempo tick and the origin times the smallest factor that
    /// brings every tick to 5 ms or less, so every instant keeps its time.
    /// `None` when that tick is a sample or shorter (refining only shortens
    /// it), or the refinement leaves the SMF ranges.
    pub fn exportable_for(&self, rate: SampleRate) -> Option<TimeMap> {
        let ppq = u64::from(self.ppq);
        let segments = self.tempo.segments();
        let factor = segments
            .iter()
            .map(|s| u64::from(s.us_per_quarter).div_ceil(MAX_TICK_US as u64 * ppq))
            .max()?
            .max(1);
        let ppq = u16::try_from(ppq * factor).ok()?;
        let tempo = segments
            .iter()
            .map(|s| {
                Some(TempoSegment {
                    tick: s.tick.checked_mul(factor)?,
                    us_per_quarter: s.us_per_quarter,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let tempo = TempoMap::try_from(tempo).ok()?;
        let refined = TimeMap::new(ppq, tempo, self.origin_tick.checked_mul(factor)?).ok()?;
        refined.check_exportable(rate).is_ok().then_some(refined)
    }

    /// The nearest tick to timeline sample `s`; a tie goes to the later tick.
    pub fn tick_of_sample(&self, s: TimelinePos, rate: SampleRate) -> u64 {
        self.tick_at(s.0, rate.hz())
    }

    /// The nearest sample to tick `t`; a tie goes to the later sample. A tick
    /// before the origin maps to sample 0.
    pub fn sample_of_tick(&self, t: u64, rate: SampleRate) -> TimelinePos {
        let (at, origin) = (self.quarter_us(t), self.quarter_us(self.origin_tick));
        if at <= origin {
            return TimelinePos(0);
        }
        // (at - origin) / ppq microseconds, times hz / 10^6.
        let n = (at - origin) * u128::from(rate.hz());
        TimelinePos(saturate(div_round(n, u128::from(self.ppq) * US_PER_S)))
    }

    /// The nearest tick to control frame `k`, at `k / 200` s on the timeline.
    pub fn tick_of_frame(&self, k: u64) -> u64 {
        self.tick_at(k, CONTROL_HZ)
    }

    /// The time of `tick` from tick 0, exactly, in units of `1 / ppq` µs: the
    /// sum over tempo segments of ticks times microseconds per quarter. At most
    /// 2^64 · 2^24 = 2^88.
    fn quarter_us(&self, tick: u64) -> u128 {
        let segments = self.tempo.segments();
        let mut acc = 0u128;
        for pair in segments.windows(2) {
            let (s, next) = (pair[0], pair[1]);
            if tick < next.tick {
                return acc + u128::from(tick - s.tick) * u128::from(s.us_per_quarter);
            }
            acc += u128::from(next.tick - s.tick) * u128::from(s.us_per_quarter);
        }
        // The map is non-empty, and `tick` is at or after its last segment.
        let last = segments[segments.len() - 1];
        acc + u128::from(tick - last.tick) * u128::from(last.us_per_quarter)
    }

    /// The nearest tick to the timeline time `num / den` seconds, piecewise
    /// across tempo segments. Everything is scaled to units of
    /// `1 / (ppq · den)` µs, where a tick of segment `s` is
    /// `us_per_quarter · den` units: under 2^110 throughout.
    fn tick_at(&self, num: u64, den: u32) -> u64 {
        let den = u128::from(den);
        let target = self.quarter_us(self.origin_tick) * den
            + u128::from(num) * US_PER_S * u128::from(self.ppq);
        let segments = self.tempo.segments();
        let mut start = 0u128;
        for pair in segments.windows(2) {
            let (s, next) = (pair[0], pair[1]);
            let step = u128::from(s.us_per_quarter) * den;
            let end = start + u128::from(next.tick - s.tick) * step;
            if target < end {
                // The segment's linear piece runs up to and including next.tick.
                return saturate(u128::from(s.tick) + div_round(target - start, step));
            }
            start = end;
        }
        let last = segments[segments.len() - 1];
        let step = u128::from(last.us_per_quarter) * den;
        saturate(u128::from(last.tick) + div_round(target - start, step))
    }
}

/// Microseconds per second.
const US_PER_S: u128 = 1_000_000;
/// The longest exportable tick: 5 ms, one control frame.
const MAX_TICK_US: u128 = 5_000;

fn saturate(v: u128) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

/// `n / d` rounded half up; `d > 0`.
fn div_round(n: u128, d: u128) -> u128 {
    n / d + u128::from(2 * (n % d) >= d)
}
