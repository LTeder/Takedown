//! Test support for the render contract (core-types §2.15 and §3.6), behind
//! the `testkit` cargo feature:
//! - [`ToyBackend`], which uses every hook: a pitch-driven phase accumulator,
//!   one-pole amp smoothing per sample, cell-rate brightness smoothing, hit
//!   envelopes that play slices, trigger-relative and absolute noise, a
//!   latency delay line, and a release whose tail can be declared honestly or
//!   not;
//! - the shared proptest strategies of `core/tests/common`, re-exported, plus
//!   [`toy_config`], [`blocks`] and [`render_case`];
//! - [`render_mono`], [`render_blocks`] and [`render_from_segment`];
//! - [`first_bit_difference`];
//! - [`tail_overrun`], which renders the quiet parts through the backend and
//!   returns their peak;
//! - [`slice_purity_violation`], which audits a `SliceSource` for position
//!   purity;
//! - [`CountingAlloc`], which counts alloc, dealloc and realloc on the current
//!   thread. Each test binary that needs it declares it as its
//!   `#[global_allocator]`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::Arc;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::{select, subsequence};
use takedown_core::control::{
    ControlFrame, ControlTrack, Curves, GateSpan, HitEvent, Link, TrackParts, U7, Velocity,
};
use takedown_core::hit::RoleName;
use takedown_core::ids::{Hash32, TakeId};
use takedown_core::seed::{NoiseStream, Seed, SeedTree, VoiceSeeds};
use takedown_core::time::{
    RenderPos, SampleRate, TimelinePos, TimelineSpan, frames_covering, rescale_ceil,
};

use super::{
    BackendInfo, BindSpec, BoundKind, BoundTrack, BoundTrigger, Driver, DriverEvent, EventCursor,
    Prepared, RenderBackend, SliceId, SliceSource, VoiceRoute,
};

/// The generators shared with core's tests: one file, included rather than
/// copied.
#[path = "../../../core/tests/common/mod.rs"]
pub mod common;

pub use common::*;

// ---------------------------------------------------------------------------
// The toy backend

/// Hit slots: more than the dense roll's 12-plus overlapping hits.
const SLOTS: usize = 32;
/// Gate attack, per sample.
const ATTACK_STEP: f32 = 1.0 / 16.0;
/// One-pole amp smoothing coefficient, per sample.
const AMP_K: f32 = 0.02;
/// Brightness smoothing coefficient, per cell.
const BRIGHT_K: f32 = 0.25;
/// Level of the voice-level (absolute) noise.
const VOICE_NOISE_LEVEL: f32 = 0.05;
/// Per-sample pitch drop of a hit.
const HIT_PITCH_DROP: f32 = 0.999_95;
/// Noise streams, one per use.
const STREAM_CLICK: u64 = 1;
const STREAM_BREATH: u64 = 2;
const STREAM_VOICE: u64 = 3;
const STREAM_SCALAR: u64 = 4;
const STREAM_TAKE: u64 = 5;

/// A per-trigger random scalar in [0, 1).
///
/// Drawn far from index 0: core's `NoiseStream::u64_at(n)` is
/// `mix(mix(n + a) ^ mix(a ^ stream))`, which is 0 wherever
/// `n + a == a ^ stream`. That index lies within `stream` of 0, and for
/// stream 0 it is 0 itself, so `seed.noise(0).unit_at(0)` is 0.0 for every
/// seed.
#[inline]
fn scalar(seed: Seed, i: u64) -> f32 {
    seed.noise(STREAM_SCALAR).unit_at((1 << 40) + i)
}

/// How a [`ToyBackend`] is set up and routed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToyConfig {
    /// Declared and implemented latency, in output samples.
    pub latency: u32,
    /// The cell grid in samples (0 or 1: none).
    pub cell: u32,
    /// The declared tail, in control frames.
    pub tail: u32,
    /// The release actually played, in control frames. Honest when it is at
    /// most `tail`.
    pub release: u32,
    /// Voice-level noise, indexed by the absolute sample.
    pub voice_noise: bool,
    /// Whether the voice takes the phrase lane.
    pub phrase: bool,
    /// The hit roles the voice takes.
    pub roles: BTreeSet<RoleName>,
    pub slice_max_ms: u32,
}

impl ToyConfig {
    /// An honest hit-only voice for `roles`, with no latency, cell or tail.
    pub fn hits(roles: &[&str]) -> Self {
        Self {
            latency: 0,
            cell: 0,
            tail: 0,
            release: 0,
            voice_noise: true,
            phrase: false,
            roles: roles
                .iter()
                .map(|r| RoleName::new(r).expect("a valid role name"))
                .collect(),
            slice_max_ms: 0,
        }
    }

    /// Whether the release fits inside the declared tail.
    pub fn honest(&self) -> bool {
        self.release <= self.tail
    }

    /// The same voice with a release well past its declared tail.
    #[must_use]
    pub fn dishonest(mut self) -> Self {
        self.release = self.tail * 2 + 12;
        self
    }

    pub fn route(&self) -> VoiceRoute {
        VoiceRoute {
            phrase: self.phrase,
            roles: self.roles.clone(),
        }
    }

    pub fn info(&self) -> BackendInfo {
        BackendInfo {
            id: "toy",
            version: 1,
            realtime_capable: true,
            tail_frames: self.tail,
            cell: self.cell,
            slice_max_ms: self.slice_max_ms,
        }
    }
}

/// One sounding hit.
#[derive(Clone, Copy, Debug)]
struct Slot {
    active: bool,
    at: u64,
    dur: u64,
    /// `dur` plus the release, in samples: silent from here on.
    end: u64,
    vel: f32,
    breath: f32,
    mouth: f32,
    drive: f32,
    phase: f32,
    inc: f32,
    noise: NoiseStream,
    slice: Option<SliceId>,
}

impl Slot {
    fn idle() -> Self {
        Self {
            active: false,
            at: 0,
            dur: 1,
            end: 1,
            vel: 0.0,
            breath: 0.0,
            mouth: 0.0,
            drive: 1.0,
            phase: 0.0,
            inc: 0.0,
            noise: Seed(0).noise(0),
            slice: None,
        }
    }

    /// Output and envelope at absolute sample `n`. Everything here is indexed
    /// from the trigger, so a moved hit is a shifted copy of itself.
    #[inline]
    fn sample(&mut self, n: u64, bright: f32, slice: &[f32]) -> (f32, f32) {
        let k = n - self.at;
        if k >= self.end {
            self.active = false;
            return (0.0, 0.0);
        }
        let env = if k < self.dur {
            1.0 - 0.5 * (k as f32 / self.dur as f32)
        } else {
            0.5 * (1.0 - (k - self.dur + 1) as f32 / (self.end - self.dur) as f32)
        };
        let osc = shape(self.phase, bright);
        self.phase += self.inc;
        self.phase -= self.phase.floor();
        self.inc *= HIT_PITCH_DROP;
        let mut x = osc * (1.0 - self.mouth) + self.noise.bipolar_at(k) * self.breath * 0.5;
        if let Some(s) = slice.get(k as usize) {
            x += s * self.mouth;
        }
        (soft(x * self.drive) * env * self.vel, env)
    }
}

/// The phrase voice's state.
#[derive(Clone, Copy, Debug)]
struct Phrase {
    gate: f32,
    gate_on: bool,
    rel_from: f32,
    rel_at: u64,
    vel: f32,
    amp: f32,
    phase: f32,
    click: bool,
    click_at: u64,
    click_noise: NoiseStream,
    click_drive: f32,
    click_slice: Option<SliceId>,
}

impl Phrase {
    fn idle(at: u64) -> Self {
        Self {
            gate: 0.0,
            gate_on: false,
            rel_from: 0.0,
            rel_at: at,
            vel: 0.0,
            amp: 0.0,
            phase: 0.0,
            click: false,
            click_at: at,
            click_noise: Seed(0).noise(0),
            click_drive: 1.0,
            click_slice: None,
        }
    }
}

/// A triangle morphing towards a square with `bright`.
#[inline]
fn shape(phase: f32, bright: f32) -> f32 {
    let tri = 4.0 * (phase - 0.5).abs() - 1.0;
    let sq = if phase < 0.5 { 1.0 } else { -1.0 };
    tri + (sq - tri) * bright * 0.5
}

/// A cheap soft clipper: the toy's mangle chain.
#[inline]
fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

#[inline]
fn db_to_lin(db: f32) -> f32 {
    (db * (std::f32::consts::LN_10 / 20.0)).exp()
}

/// A backend that uses every hook of the contract, cheaply. Mono, allocation
/// free outside `prepare`.
#[derive(Clone, Debug)]
pub struct ToyBackend {
    cfg: ToyConfig,
    hz: f32,
    /// The release in output samples.
    release: u64,
    click_len: u64,
    phrase: Phrase,
    bright: f32,
    last_hit_bright: f32,
    voice_noise: NoiseStream,
    slots: Box<[Slot]>,
    /// `slots[live..]` are idle.
    live: usize,
    delay: Box<[f32]>,
}

impl ToyBackend {
    /// Unprepared; call `prepare` before use.
    pub fn new(cfg: ToyConfig) -> Self {
        Self {
            cfg,
            hz: 48_000.0,
            release: 0,
            click_len: 1,
            phrase: Phrase::idle(0),
            bright: 0.5,
            last_hit_bright: 0.5,
            voice_noise: Seed(0).noise(1),
            slots: Box::default(),
            live: 0,
            delay: Box::default(),
        }
    }

    pub fn config(&self) -> &ToyConfig {
        &self.cfg
    }

    /// A free slot, or the oldest one when all are sounding.
    fn take_slot(&mut self, at: u64) -> usize {
        if let Some(i) = self
            .slots
            .iter()
            .position(|s| !s.active || at - s.at >= s.end)
        {
            return i;
        }
        self.slots
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| s.at)
            .map_or(0, |(i, _)| i)
    }

    #[inline]
    fn phrase_sample(&mut self, t: &BoundTrack, n: u64, f: &ControlFrame) -> (f32, f32) {
        let ph = &mut self.phrase;
        if ph.gate_on {
            ph.gate = (ph.gate + ATTACK_STEP).min(1.0);
        } else {
            let k = n - ph.rel_at;
            ph.gate = if k + 1 >= self.release {
                0.0
            } else {
                ph.rel_from * (1.0 - (k + 1) as f32 / self.release as f32)
            };
        }
        let amp_target = db_to_lin(f.amp_db) * ph.vel;
        ph.amp += (amp_target - ph.amp) * AMP_K;
        let freq = 440.0 * ((f.pitch_st - 69.0) / 12.0).exp2();
        ph.phase += freq / self.hz;
        ph.phase -= ph.phase.floor();
        let mut x = shape(ph.phase, self.bright) * ph.amp;
        if ph.click {
            let k = n - ph.click_at;
            let slice = ph.click_slice.map_or(&[][..], |id| t.slice(id));
            if k < self.click_len {
                let fade = 1.0 - k as f32 / self.click_len as f32;
                x += ph.click_noise.bipolar_at(k) * fade * 0.3 * ph.vel;
            }
            if let Some(s) = slice.get(k as usize) {
                x += s * 0.5;
            }
            if k >= self.click_len && k >= slice.len() as u64 {
                ph.click = false;
            }
        }
        (soft(x * ph.click_drive) * ph.gate, ph.gate)
    }
}

impl RenderBackend for ToyBackend {
    fn info(&self) -> BackendInfo {
        self.cfg.info()
    }

    fn prepare(&mut self, rate: SampleRate, max_block: usize) -> Prepared {
        let _ = max_block;
        self.hz = rate.hz() as f32;
        self.release = u64::from(self.cfg.release) * u64::from(rate.hz()) / 200;
        self.click_len = (u64::from(rate.hz()) / 500).max(1);
        self.slots = vec![Slot::idle(); SLOTS].into_boxed_slice();
        self.live = 0;
        self.delay = vec![0.0; self.cfg.latency as usize].into_boxed_slice();
        Prepared {
            latency: self.cfg.latency,
        }
    }

    fn reset(&mut self, track: &BoundTrack, seeds: VoiceSeeds, at: RenderPos) {
        let _ = track;
        self.phrase = Phrase::idle(at.0);
        self.bright = 0.5;
        self.last_hit_bright = 0.5;
        self.voice_noise = seeds.mangle.noise(STREAM_VOICE);
        self.slots.fill(Slot::idle());
        self.live = 0;
        self.delay.fill(0.0);
    }

    fn cell(&mut self, track: &BoundTrack, at: RenderPos) {
        let target = track
            .cursor(at)
            .map_or(self.last_hit_bright, |c| c.get().bright);
        self.bright += (target - self.bright) * BRIGHT_K;
    }

    fn trigger(&mut self, track: &BoundTrack, trig: &BoundTrigger) {
        let _ = track;
        let at = trig.at.0;
        match trig.kind {
            BoundKind::NoteOn { vel, slice } | BoundKind::Retrigger { vel, slice } => {
                let ph = &mut self.phrase;
                ph.gate_on = true;
                ph.vel = f32::from(vel.get()) / 127.0;
                ph.click = true;
                ph.click_at = at;
                ph.click_noise = trig.seeds.body.noise(STREAM_CLICK);
                ph.click_drive = 1.0 + 3.0 * scalar(trig.seeds.mangle, 0);
                ph.click_slice = slice;
            }
            BoundKind::NoteOff => {
                let ph = &mut self.phrase;
                ph.gate_on = false;
                ph.rel_at = at;
                ph.rel_from = ph.gate;
            }
            BoundKind::Hit {
                role,
                vel,
                bright,
                breath,
                mouth,
                dur,
                slice,
            } => {
                let i = self.take_slot(at);
                let body = trig.seeds.body;
                let bright = f32::from(bright.get()) / 127.0;
                let f0 = 40.0
                    * (1.0 + f32::from(role % 6))
                    * (1.0 + 0.5 * scalar(body, 0))
                    * (1.0 + bright);
                let inc = f0 / self.hz;
                // The body seed also sets the start phase, so every sample of
                // the hit depends on it.
                let phase = scalar(body, 1) + trig.lead * inc;
                self.slots[i] = Slot {
                    active: true,
                    at,
                    dur,
                    end: dur.saturating_add(self.release),
                    vel: f32::from(vel.get()) / 127.0,
                    breath: f32::from(breath.get()) / 127.0,
                    mouth: f32::from(mouth.get()) / 127.0,
                    drive: 1.0 + 3.0 * scalar(trig.seeds.mangle, 0),
                    phase: phase - phase.floor(),
                    inc,
                    noise: trig.seeds.body.noise(STREAM_BREATH),
                    slice,
                };
                self.live = self.live.max(i + 1);
                self.last_hit_bright = bright;
            }
        }
    }

    fn run(&mut self, track: &BoundTrack, start: RenderPos, out: &mut [f32]) {
        let mut cursor = track.cursor(start);
        let bright = self.bright;
        for (n, o) in (start.0..).zip(out.iter_mut()) {
            let mut y = 0.0f32;
            let mut env = 0.0f32;
            if let Some(c) = cursor.as_mut() {
                let f = c.get();
                c.advance();
                let (x, e) = self.phrase_sample(track, n, &f);
                y += x;
                env += e;
            }
            for slot in &mut self.slots[..self.live] {
                if slot.active {
                    let slice = slot.slice.map_or(&[][..], |id| track.slice(id));
                    let (x, e) = slot.sample(n, bright, slice);
                    y += x;
                    env += e;
                }
            }
            if self.cfg.voice_noise {
                y += self.voice_noise.bipolar_at(n) * VOICE_NOISE_LEVEL * env;
            }
            if !self.delay.is_empty() {
                let i = (n % self.delay.len() as u64) as usize;
                std::mem::swap(&mut self.delay[i], &mut y);
            }
            *o = y;
        }
    }
}

/// Take audio for tests: a pure function of the absolute output sample, so a
/// slice does not depend on which union it was read with.
#[derive(Clone, Debug)]
pub struct ToySlices {
    /// The take's rate (the track's timeline rate).
    pub timeline: SampleRate,
    /// How many reads were made.
    pub reads: usize,
}

impl ToySlices {
    pub fn new(timeline: SampleRate) -> Self {
        Self { timeline, reads: 0 }
    }
}

impl SliceSource for ToySlices {
    fn read(&mut self, take: TakeId, span: TimelineSpan, out_rate: SampleRate) -> Option<Vec<f32>> {
        self.reads += 1;
        let a = rescale_ceil(span.start().0, self.timeline, out_rate).0;
        let b = rescale_ceil(span.end().0, self.timeline, out_rate).0;
        let mut first = [0u8; 8];
        first.copy_from_slice(&take.hash32().0[..8]);
        let noise = Seed(u64::from_le_bytes(first)).noise(STREAM_TAKE);
        Some((a..b).map(|q| noise.bipolar_at(q) * 0.5).collect())
    }
}

/// The first absolute output sample at which `source` is not position pure
/// over `span` (see [`SliceSource`]): `span` is read whole, then as two
/// pieces split at each of several points and as several inner windows, and
/// every piece must equal the whole read at the same output samples, bit for
/// bit and in length. `None` when every read agrees, or when `source` has no
/// audio for `take`.
pub fn slice_purity_violation(
    source: &mut dyn SliceSource,
    take: TakeId,
    timeline: SampleRate,
    out_rate: SampleRate,
    span: TimelineSpan,
) -> Option<u64> {
    let (a, b) = (span.start().0, span.end().0);
    let whole = source.read(take, span, out_rate)?;
    let origin = rescale_ceil(a, timeline, out_rate).0;
    let len = span.len();
    let mut pieces = Vec::new();
    for k in 1..4u64 {
        let cut = a + len * k / 4;
        pieces.push((a, cut));
        pieces.push((cut, b));
        let inset = len * k / 9;
        pieces.push((a + inset, b - inset));
    }
    for (p, q) in pieces {
        // Every piece lies inside `span`, so it is ordered and its samples
        // are a range of the whole read.
        let piece = TimelineSpan::new(TimelinePos(p), TimelinePos(q)).expect("p <= q");
        let first = rescale_ceil(p, timeline, out_rate).0;
        let last = rescale_ceil(q, timeline, out_rate).0;
        let at = |n: u64| ((n - origin) as usize).min(whole.len());
        let want = &whole[at(first)..at(last)];
        let got = source.read(take, piece, out_rate).unwrap_or_default();
        if let Some(i) = first_bit_difference(want, &got) {
            return Some(first + i as u64);
        }
    }
    None
}

/// A toy voice bound to a track: the backend's settings, its `Prepared`, the
/// bound track and the block size.
#[derive(Debug)]
pub struct ToyVoice {
    pub cfg: ToyConfig,
    pub prepared: Prepared,
    pub bound: BoundTrack,
    pub max_block: usize,
}

impl ToyVoice {
    pub fn bind(
        cfg: &ToyConfig,
        track: &Arc<ControlTrack>,
        out_rate: SampleRate,
        tree: &SeedTree,
        voice: &str,
        max_block: usize,
    ) -> Self {
        let prepared = ToyBackend::new(cfg.clone()).prepare(out_rate, max_block);
        let info = cfg.info();
        let route = cfg.route();
        let spec = BindSpec {
            voice,
            route: &route,
            out_rate,
            info: &info,
            prepared,
            seeds: tree,
        };
        let mut slices = ToySlices::new(track.rate());
        let bound = BoundTrack::bind(Arc::clone(track), &spec, &mut slices);
        Self {
            cfg: cfg.clone(),
            prepared,
            bound,
            max_block,
        }
    }

    /// A fresh, prepared backend.
    pub fn backend(&self) -> ToyBackend {
        let mut b = ToyBackend::new(self.cfg.clone());
        b.prepare(self.bound.rate(), self.max_block);
        b
    }

    /// The bound track's end plus a margin of trailing quiet.
    pub fn render_len(&self) -> u64 {
        self.bound.end().0 + 64
    }

    pub fn mono(&self, len: u64) -> Vec<f32> {
        render_mono(
            &mut self.backend(),
            self.prepared,
            &self.bound,
            self.max_block,
            len,
        )
    }

    pub fn blocks(&self, len: u64, blocks: &[usize]) -> Vec<f32> {
        render_blocks(
            &mut self.backend(),
            self.prepared,
            &self.bound,
            self.max_block,
            len,
            blocks,
        )
    }

    pub fn from_segment(&self, segment: usize, len: u64, blocks: &[usize]) -> Vec<f32> {
        render_from_segment(
            &mut self.backend(),
            self.prepared,
            &self.bound,
            self.max_block,
            segment,
            len,
            blocks,
        )
    }

    /// The peak of the quiet parts rendered through a fresh backend.
    pub fn tail_overrun(&self) -> f32 {
        tail_overrun(&mut self.backend(), &self.bound, self.max_block)
    }
}

// ---------------------------------------------------------------------------
// Render loops

/// Output `[0, len)` from one `process` call.
pub fn render_mono<B: RenderBackend + ?Sized>(
    b: &mut B,
    prepared: Prepared,
    t: &BoundTrack,
    max_block: usize,
    len: u64,
) -> Vec<f32> {
    let mut d = Driver::new(&b.info(), prepared, max_block);
    d.start(b, t, 0);
    let mut out = vec![0.0; len as usize];
    d.process(b, t, &mut out);
    out
}

/// Output `[0, len)` fed in blocks of the given lengths, cycled.
pub fn render_blocks<B: RenderBackend + ?Sized>(
    b: &mut B,
    prepared: Prepared,
    t: &BoundTrack,
    max_block: usize,
    len: u64,
    blocks: &[usize],
) -> Vec<f32> {
    render_from_segment(b, prepared, t, max_block, 0, len, blocks)
}

/// Output `[segments[segment].start, len)`, started at that segment and fed in
/// blocks of the given lengths, cycled.
pub fn render_from_segment<B: RenderBackend + ?Sized>(
    b: &mut B,
    prepared: Prepared,
    t: &BoundTrack,
    max_block: usize,
    segment: usize,
    len: u64,
    blocks: &[usize],
) -> Vec<f32> {
    let mut d = Driver::new(&b.info(), prepared, max_block);
    d.start(b, t, segment);
    let start = d.position().map_or(0, |p| p.0);
    let n = len.saturating_sub(start) as usize;
    let mut out = vec![0.0; n];
    let mut o = 0;
    for &k in blocks.iter().cycle() {
        if o >= n {
            break;
        }
        let k = k.clamp(1, n - o);
        d.process(b, t, &mut out[o..o + k]);
        o += k;
    }
    if o < n {
        // No block lengths were given.
        d.process(b, t, &mut out[o..]);
    }
    out
}

/// The first index at which the two buffers differ bit for bit, or their
/// shorter length when one is a prefix of the other.
pub fn first_bit_difference(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
        .or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
}

/// Renders the quiet parts through the backend instead of writing zeros, from
/// 0 to one second past `t.end()`, and returns their peak magnitude in
/// backend (control) time. An honest backend stays below 1e-3. The backend
/// must be prepared at `t.rate()` with at least `max_block`.
pub fn tail_overrun<B: RenderBackend + ?Sized>(b: &mut B, t: &BoundTrack, max_block: usize) -> f32 {
    let info = b.info();
    let max_block = max_block.max(1);
    let mut cursor = EventCursor::new(t, RenderPos(0), info.cell, max_block);
    let mut buf = vec![0.0f32; max_block];
    let end = RenderPos(t.end().0 + u64::from(t.rate().hz()));
    let mut peak = 0.0f32;
    while let Some(ev) = cursor.next_event(t, end) {
        match ev {
            DriverEvent::Reset { at } => b.reset(t, t.voice_seeds(), at),
            DriverEvent::Cell { at } => b.cell(t, at),
            DriverEvent::Trigger { index } => b.trigger(t, &t.triggers()[index]),
            DriverEvent::Run { start, len } => b.run(t, start, &mut buf[..len]),
            DriverEvent::Silence { start, len } => {
                b.run(t, start, &mut buf[..len]);
                peak = buf[..len].iter().fold(peak, |p, x| p.max(x.abs()));
            }
        }
    }
    peak
}

// ---------------------------------------------------------------------------
// Strategies

/// Cases per render property: 128 (core-types §3), or `PROPTEST_CASES` when
/// set, as for the 5,000-case soak before a contract change merges.
pub fn render_cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128)
}

/// The fixed block cycle every invariance test includes.
pub const FIXED_BLOCKS: [usize; 5] = [1, 7, 220, 221, 4096];

/// Rate pairs (timeline, output) that every render property includes: the
/// non-integer samples-per-frame rates against each other and against 48 kHz.
pub const FIXED_RATE_PAIRS: [(u32, u32); 6] = [
    (44_100, 88_200),
    (88_200, 44_100),
    (44_100, 48_000),
    (48_000, 44_100),
    (8_000, 192_000),
    (192_000, 11_025),
];

/// The names of a render case's two voices.
pub const VOICE_NAMES: [&str; 2] = ["toy_a", "toy_b"];

/// Block lengths to cycle through: random in 1..=2·max, or the fixed cycle.
pub fn blocks(max: usize) -> impl Strategy<Value = Vec<usize>> {
    let max = max.max(1);
    prop_oneof![
        1 => Just(FIXED_BLOCKS.to_vec()),
        3 => vec(1..=2 * max, 1..=12),
    ]
}

/// A subset of the role pool.
pub fn route_roles() -> impl Strategy<Value = BTreeSet<RoleName>> {
    subsequence(ROLE_POOL.to_vec(), 0..=ROLE_POOL.len()).prop_map(|names| {
        names
            .into_iter()
            .map(|n| RoleName::new(n).expect("pool names are valid"))
            .collect()
    })
}

/// Honest toy settings: latency {0, 1, 7, 64}; cell {0, 7, 32}; tail {0, 4,
/// 30}; route phrase on and off; role subsets; slice_max {0, 30 ms,
/// unbounded}.
pub fn toy_config() -> impl Strategy<Value = ToyConfig> {
    (
        select(vec![0u32, 1, 7, 64]),
        select(vec![0u32, 7, 32]),
        select(vec![0u32, 4, 30]),
        any::<bool>(),
        route_roles(),
        select(vec![0u32, 30, u32::MAX]),
    )
        .prop_map(
            |(latency, cell, tail, phrase, roles, slice_max_ms)| ToyConfig {
                latency,
                cell,
                tail,
                release: tail,
                voice_noise: true,
                phrase,
                roles,
                slice_max_ms,
            },
        )
}

/// (timeline, output) rates: independent `std_rate`s (so mostly unequal), or
/// one of [`FIXED_RATE_PAIRS`].
pub fn rate_pair() -> impl Strategy<Value = (SampleRate, SampleRate)> {
    prop_oneof![
        3 => (std_rate(), std_rate()),
        1 => select(FIXED_RATE_PAIRS.to_vec()).prop_map(|(a, b)| (rate(a), rate(b))),
    ]
}

/// The shared `track` plan, reshaped so render properties see resets: no
/// ten-minute draws (one fixed sparse case covers those), a quarter as many
/// tiny tracks, and only one hit in four of the plan's long durations (which run to
/// the end of the track and merge every segment). Every hard case remains.
pub fn render_track(rate: SampleRate) -> impl Strategy<Value = ControlTrack> {
    let max = 4 * u64::from(rate.hz());
    track_plan(rate)
        .prop_filter("render properties draw tracks of at most 4 s", move |p| {
            p.len <= max && (p.len > 400 || p.salt % 4 == 0)
        })
        .prop_map(|mut p| {
            for h in &mut p.hits {
                if h.dur_kind >= 2 && h.raw % 4 != 0 {
                    h.dur_kind = 1;
                }
            }
            ControlTrack::new(p.build()).expect("the generator builds valid tracks")
        })
}

/// Everything one render property draws.
#[derive(Clone, Debug)]
pub struct RenderCase {
    pub track: Arc<ControlTrack>,
    pub out_rate: SampleRate,
    /// Two voices with different routes.
    pub voices: [ToyConfig; 2],
    pub tree: SeedTree,
    pub max_block: usize,
}

impl RenderCase {
    pub fn bind(&self) -> [ToyVoice; 2] {
        [0, 1].map(|i| {
            ToyVoice::bind(
                &self.voices[i],
                &self.track,
                self.out_rate,
                &self.tree,
                VOICE_NAMES[i],
                self.max_block,
            )
        })
    }
}

pub fn render_case() -> impl Strategy<Value = RenderCase> {
    rate_pair().prop_flat_map(|(timeline, out_rate)| {
        (
            render_track(timeline),
            toy_config(),
            toy_config(),
            seed_tree(),
            select(vec![64usize, 256, 1024, 4096]),
        )
            .prop_map(move |(track, a, mut b, tree, max_block)| {
                if a.route() == b.route() {
                    b.phrase = !b.phrase;
                }
                RenderCase {
                    track: Arc::new(track),
                    out_rate,
                    voices: [a, b],
                    tree,
                    max_block,
                }
            })
    })
}

// ---------------------------------------------------------------------------
// Fixtures for the fixed regression cases

/// Constant curves: A3, -6 dB, half bright, a little breath, neutral vowel.
pub fn flat_curves(frames: u64) -> Curves {
    let n = frames as usize;
    Curves {
        pitch_st: vec![57.0; n],
        amp_db: vec![-6.0; n],
        bright: vec![0.5; n],
        breath: vec![0.2; n],
        vowel_x: vec![0.0; n],
        vowel_y: vec![0.0; n],
    }
}

/// A hit with mid-range tone, breath and mouth.
pub fn hit_event(at: u64, anchor: u64, dur: u64, role: u8, vel: u8) -> HitEvent {
    HitEvent {
        at: TimelinePos(at),
        anchor: TimelinePos(anchor),
        dur,
        role,
        vel: Velocity::new(vel).expect("velocity in 1..=127"),
        bright: U7::new(64).expect("7-bit"),
        breath: U7::new(40).expect("7-bit"),
        mouth: U7::new(80).expect("7-bit"),
    }
}

/// A Detached gate span anchored at its onset.
pub fn gate_span(on: u64, off: u64, link: Link) -> GateSpan {
    GateSpan {
        on: TimelinePos(on),
        off: TimelinePos(off),
        anchor: TimelinePos(on),
        vel: Velocity::new(100).expect("velocity in 1..=127"),
        link,
    }
}

/// A valid track with flat curves. `roles` must be sorted and unique; with
/// `take`, slices come from a fixed take id.
pub fn fixed_track(
    rate: SampleRate,
    len: u64,
    roles: &[&str],
    spans: Vec<GateSpan>,
    hits: Vec<HitEvent>,
    take: bool,
) -> ControlTrack {
    let parts = TrackParts {
        rate,
        len,
        take: take.then(|| TakeId::from_hash(Hash32([0x5a; 32]))),
        roles: roles
            .iter()
            .map(|r| RoleName::new(r).expect("a valid role name"))
            .collect(),
        curves: flat_curves(frames_covering(len, rate)),
        spans,
        hits,
    };
    ControlTrack::new(parts).expect("a valid fixed track")
}

// ---------------------------------------------------------------------------
// Counting allocator

thread_local! {
    // Const-initialised and without Drop, so access never allocates.
    static ALLOC_CALLS: Cell<u64> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOC_CALLS.try_with(|c| c.set(c.get() + 1));
}

/// Alloc, dealloc and realloc calls made on this thread so far, when
/// [`CountingAlloc`] is the global allocator.
pub fn alloc_calls() -> u64 {
    ALLOC_CALLS.with(Cell::get)
}

/// Counts alloc, dealloc and realloc calls on the current thread and forwards
/// them to the system allocator. Declare it in a test binary with
/// `#[global_allocator] static A: CountingAlloc = CountingAlloc;`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CountingAlloc;

// GlobalAlloc is an unsafe trait, and a counting allocator is the only way to
// prove the audio path never allocates. This impl only bumps a thread-local
// counter and forwards every request unchanged to the system allocator, so it
// upholds exactly the contract `System` does.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: forwarded unchanged; the caller upholds alloc's contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: forwarded unchanged; the caller upholds alloc_zeroed's contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        bump();
        // SAFETY: ptr came from System through this allocator with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        // SAFETY: ptr came from System through this allocator with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
