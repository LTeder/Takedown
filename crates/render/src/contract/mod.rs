//! The render contract (core-types §2.15): the backend trait, the bound
//! track with its segment plan, the driver that owns event order, and the
//! offline worker loop.
//!
//! The renderer never sees notes: a backend receives a [`BoundTrack`] built
//! from a `ControlTrack` and nothing else. The driver guarantees block-size
//! invariance, reset placement and latency compensation once, so a backend
//! only has to obey the laws listed on [`RenderBackend`].
//!
//! This module imports nothing from the rest of `render`, so it can become its
//! own crate when `infer` or `plugin` needs the trait without Plaits.

mod bound;
mod driver;
#[cfg(feature = "testkit")]
pub mod testkit;

use std::collections::BTreeSet;

use takedown_core::hit::RoleName;
use takedown_core::ids::TakeId;
use takedown_core::layer::BackendStamp;
use takedown_core::seed::{SeedTree, VoiceSeeds};
use takedown_core::time::{RenderPos, SampleRate, TimelineSpan};

pub use bound::{
    BoundKind, BoundTrack, BoundTrigger, ControlCursor, ROUNDING_SLACK_FRAMES, Segment, SliceBank,
    SliceId, activity_frames, segment_plan,
};
pub use driver::{Cancelled, Driver, DriverEvent, EventCursor, render_offline};

/// Covers core interpolation and rounding, seed derivation and noise, the
/// segment plan, the event order and the slice resampler. Bump on any change
/// to their output.
pub const RENDER_CONTRACT_VERSION: u32 = 1;

/// What a backend declares about itself. Constant for a backend version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BackendInfo {
    pub id: &'static str,
    /// Bump on any change that alters output.
    pub version: u32,
    /// tract plans are not, yet.
    pub realtime_capable: bool,
    /// Control frames output may continue after activity ends; audited by
    /// `testkit::tail_overrun`.
    pub tail_frames: u32,
    /// Block-rate grid in samples at the prepared rate (0 or 1: none).
    pub cell: u32,
    /// Take audio read per event: 0 none; 30 for exciters; `u32::MAX` for
    /// VoiceMaterial.
    pub slice_max_ms: u32,
}

impl BackendInfo {
    /// The identity a render key records.
    pub fn stamp(&self) -> BackendStamp {
        BackendStamp {
            id: self.id.to_owned(),
            version: self.version,
        }
    }
}

/// Output lags control time by `latency` samples (resamplers, lookahead).
/// Constant after prepare.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Prepared {
    pub latency: u32,
}

/// A voice is a backend instance that includes its own mangle chain. There is
/// no global bus.
///
/// Laws a backend must obey (the driver guarantees everything else):
/// 1. Only `prepare` allocates, locks, blocks or does I/O.
/// 2. `reset` leaves the state a pure function of `(track, seeds, at)`.
/// 3. **Split law.** `run(s, out[..a+b])` writes the same samples as
///    `run(s, out[..a])` followed by `run(s+a, out[a..a+b])`.
/// 4. Block-rate state (the 32-sample mod matrix) advances only in `cell`,
///    which the driver calls at every absolute multiple of `info.cell`.
/// 5. Randomness comes only from the seeds handed in, through `NoiseStream`.
///    Per-trigger draws use the trigger's seeds and are indexed by
///    `n - trig.at`, so a moved trigger produces a shifted copy of itself.
///    Voice-level draws use the voice seeds and are indexed by the absolute `n`.
/// 6. Output is silent once activity plus `tail_frames` has passed.
/// 7. A hit-only voice (`route.phrase == false`) reads no curves. For such a
///    voice [`BoundTrack::cursor`] returns `None`.
pub trait RenderBackend: Send {
    fn info(&self) -> BackendInfo;
    /// The only place allowed to allocate.
    fn prepare(&mut self, rate: SampleRate, max_block: usize) -> Prepared;
    fn reset(&mut self, track: &BoundTrack, seeds: VoiceSeeds, at: RenderPos);
    fn cell(&mut self, track: &BoundTrack, at: RenderPos) {
        let _ = (track, at);
    }
    fn trigger(&mut self, track: &BoundTrack, trig: &BoundTrigger);
    /// `out.len() <= max_block`, and no event falls strictly inside the run.
    fn run(&mut self, track: &BoundTrack, start: RenderPos, out: &mut [f32]);
}

/// Which events a voice takes. Hit voices route by role, so MIDI (which
/// carries roles as keys) recovers the routing; the track holds no voice slots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VoiceRoute {
    pub phrase: bool,
    pub roles: BTreeSet<RoleName>,
}

/// Everything `bind` needs besides the track and the slice source.
#[derive(Clone, Copy, Debug)]
pub struct BindSpec<'a> {
    /// The voice's stable name from the palette; its seeds derive from it.
    pub voice: &'a str,
    pub route: &'a VoiceRoute,
    pub out_rate: SampleRate,
    pub info: &'a BackendInfo,
    pub prepared: Prepared,
    pub seeds: &'a SeedTree,
}

/// Reads take audio resampled to the output rate. Implemented by render; its
/// algorithm is covered by [`RENDER_CONTRACT_VERSION`].
///
/// For `span` at the take's rate, the result holds the output-rate samples
/// from `rescale_ceil(span.start)` to `rescale_ceil(span.end)`: the samples
/// whose time lies in the span. `None` means the take is unavailable.
///
/// **Position purity.** Each output sample must depend only on `(take, its
/// absolute output index)`, never on the span it was read with. [`SliceBank`]
/// reads each union of overlapping or touching slice spans once, so the span a
/// slice is read with depends on its neighbours: a windowed resampler that
/// tapers or zero-pads at the edges of the span it is given would let an
/// inserted hit change an existing hit's slice. A resampler meets the rule by
/// reading the take audio its kernel needs beyond the span, and treating only
/// audio outside the take as zero. `testkit::slice_purity_violation` audits an
/// implementation.
pub trait SliceSource {
    fn read(&mut self, take: TakeId, span: TimelineSpan, out_rate: SampleRate) -> Option<Vec<f32>>;
}
