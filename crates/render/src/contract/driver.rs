//! The driver owns event order. At each absolute sample `p`, in backend
//! (control) time:
//! 1. `Reset`, if `p` is a segment start.
//! 2. `Cell`, if `p` is a multiple of `info.cell` and not quiet.
//! 3. Every `Trigger` at `p`, in order.
//! 4. A `Run` up to the next event, the block end, `max_block` or
//!    `quiet_from`. In a quiet part it is a `Silence` instead: the driver
//!    writes +0.0 and does not call the backend.
//!
//! Zero-length events at `p` are emitted only by the call whose block
//! contains `p`, so the event sequence is the same for every segmentation once
//! adjacent runs are merged.
//!
//! **Latency.** Output sample `n` comes from control time `n + L`; `start`
//! pre-rolls `L` samples into a scratch buffer. The segment plan already
//! includes `L`, so a reset never cuts a delay line that still holds sound.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use takedown_core::time::RenderPos;

use super::{BackendInfo, BoundTrack, Prepared, RenderBackend};

/// One step of the driver, in backend (control) time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriverEvent {
    Reset { at: RenderPos },
    Cell { at: RenderPos },
    Trigger { index: usize },
    Run { start: RenderPos, len: usize },
    Silence { start: RenderPos, len: usize },
}

/// Which zero-length events at the current position are still to come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Reset,
    Cell,
    Triggers,
    Run,
}

/// Walks a bound track's events in driver order. Allocates nothing; starting
/// one costs two binary searches.
#[derive(Clone, Debug)]
pub struct EventCursor {
    pos: u64,
    seg: usize,
    trig: usize,
    stage: Stage,
    /// 0 when the backend has no cell grid.
    cell: u64,
    max_run: u64,
}

impl EventCursor {
    /// A cursor whose first events are those at `at`. A `Reset` comes first
    /// only when `at` is a segment start.
    pub fn new(track: &BoundTrack, at: RenderPos, cell: u32, max_run: usize) -> Self {
        Self {
            pos: at.0,
            seg: track.segment_at(at),
            trig: track.triggers().partition_point(|t| t.at < at),
            stage: Stage::Reset,
            cell: if cell > 1 { u64::from(cell) } else { 0 },
            max_run: (max_run as u64).max(1),
        }
    }

    /// The position of the next event, in backend time.
    pub fn position(&self) -> RenderPos {
        RenderPos(self.pos)
    }

    /// The next event before `end`, or `None` once the position reaches it.
    pub fn next_event(&mut self, track: &BoundTrack, end: RenderPos) -> Option<DriverEvent> {
        let segments = track.segments();
        let triggers = track.triggers();
        loop {
            if self.pos >= end.0 {
                return None;
            }
            let at = RenderPos(self.pos);
            match self.stage {
                Stage::Reset => {
                    self.stage = Stage::Cell;
                    while segments.get(self.seg + 1).is_some_and(|s| s.start <= at) {
                        self.seg += 1;
                    }
                    if segments.get(self.seg).is_some_and(|s| s.start == at) {
                        return Some(DriverEvent::Reset { at });
                    }
                }
                Stage::Cell => {
                    self.stage = Stage::Triggers;
                    if self.cell != 0 && self.pos.is_multiple_of(self.cell) && !self.quiet(track) {
                        return Some(DriverEvent::Cell { at });
                    }
                }
                Stage::Triggers => {
                    // Triggers before `at` cannot be pending; skip any defensively.
                    while triggers.get(self.trig).is_some_and(|t| t.at < at) {
                        self.trig += 1;
                    }
                    if triggers.get(self.trig).is_some_and(|t| t.at == at) {
                        self.trig += 1;
                        return Some(DriverEvent::Trigger {
                            index: self.trig - 1,
                        });
                    }
                    self.stage = Stage::Run;
                }
                Stage::Run => {
                    let quiet = self.quiet(track);
                    let mut stop = end.0.min(self.pos.saturating_add(self.max_run));
                    if let Some(next) = segments.get(self.seg + 1) {
                        stop = stop.min(next.start.0);
                    }
                    if let Some(t) = triggers.get(self.trig) {
                        stop = stop.min(t.at.0);
                    }
                    if !quiet {
                        if let Some(s) = segments.get(self.seg) {
                            stop = stop.min(s.quiet_from.0);
                        }
                        if let Some(q) = self.pos.checked_div(self.cell) {
                            stop = stop.min((q + 1).saturating_mul(self.cell));
                        }
                    }
                    // Every bound above lies strictly after `pos`.
                    let len = (stop - self.pos) as usize;
                    self.pos = stop;
                    self.stage = Stage::Reset;
                    return Some(if quiet {
                        DriverEvent::Silence { start: at, len }
                    } else {
                        DriverEvent::Run { start: at, len }
                    });
                }
            }
        }
    }

    fn quiet(&self, track: &BoundTrack) -> bool {
        track
            .segments()
            .get(self.seg)
            .is_none_or(|s| self.pos >= s.quiet_from.0)
    }
}

/// Feeds a backend from an event cursor. Allocates nothing.
fn drive<B: RenderBackend + ?Sized>(
    cursor: &mut EventCursor,
    b: &mut B,
    t: &BoundTrack,
    out: &mut [f32],
) {
    let base = cursor.pos;
    let end = RenderPos(base.saturating_add(out.len() as u64));
    while let Some(ev) = cursor.next_event(t, end) {
        match ev {
            DriverEvent::Reset { at } => b.reset(t, t.voice_seeds(), at),
            DriverEvent::Cell { at } => b.cell(t, at),
            DriverEvent::Trigger { index } => b.trigger(t, &t.triggers()[index]),
            DriverEvent::Run { start, len } => {
                let o = (start.0 - base) as usize;
                b.run(t, start, &mut out[o..o + len]);
            }
            DriverEvent::Silence { start, len } => {
                let o = (start.0 - base) as usize;
                out[o..o + len].fill(0.0);
            }
        }
    }
}

/// Renders one voice: owns the event order, the cell grid and latency
/// compensation.
#[derive(Debug)]
pub struct Driver {
    cursor: Option<EventCursor>,
    cell: u32,
    max_block: usize,
    latency: u64,
    scratch: Box<[f32]>,
    bound_id: u64,
}

impl Driver {
    /// Allocates the pre-roll scratch; worker thread only.
    pub fn new(info: &BackendInfo, prepared: Prepared, max_block: usize) -> Self {
        let max_block = max_block.max(1);
        let scratch_len = (prepared.latency as usize).min(max_block);
        Self {
            cursor: None,
            cell: info.cell,
            max_block,
            latency: u64::from(prepared.latency),
            scratch: vec![0.0; scratch_len].into_boxed_slice(),
            bound_id: 0,
        }
    }

    /// The first output sample is the segment's start. Pre-rolls the latency.
    /// A segment index past the end starts the last segment.
    pub fn start<B: RenderBackend + ?Sized>(&mut self, b: &mut B, t: &BoundTrack, segment: usize) {
        let segment = segment.min(t.segments().len().saturating_sub(1));
        let at = t.segments().get(segment).map_or(RenderPos(0), |s| s.start);
        let mut cursor = EventCursor::new(t, at, self.cell, self.max_block);
        let mut left = self.latency;
        while left > 0 {
            let n = (self.scratch.len() as u64).min(left) as usize;
            drive(&mut cursor, b, t, &mut self.scratch[..n]);
            left -= n as u64;
        }
        self.cursor = Some(cursor);
        self.bound_id = t.id();
    }

    /// Takes any `out.len()`, splits at `max_block`, allocates nothing.
    /// Debug-asserts `t.id()` is the started track. Before `start` it writes
    /// silence.
    pub fn process<B: RenderBackend + ?Sized>(
        &mut self,
        b: &mut B,
        t: &BoundTrack,
        out: &mut [f32],
    ) {
        debug_assert!(
            self.cursor.is_none() || self.bound_id == t.id(),
            "Driver::process called with a track it was not started on"
        );
        match &mut self.cursor {
            Some(cursor) => drive(cursor, b, t, out),
            None => out.fill(0.0),
        }
    }

    /// The output position of the next sample `process` writes; `None` before
    /// `start`.
    pub fn position(&self) -> Option<RenderPos> {
        self.cursor
            .as_ref()
            .map(|c| RenderPos(c.pos.saturating_sub(self.latency)))
    }

    pub fn max_block(&self) -> usize {
        self.max_block
    }
}

/// `render_offline` was cancelled before it finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

/// The worker loop. Renders `[segments[from].start, end)` in `max_block`
/// blocks and hands each block to `sink`. Render mixes voices into the layer
/// or streams them to disk, so a 10-minute take never needs one buffer per
/// voice.
///
/// `progress` holds the output position up to which blocks have been handed
/// to `sink`. `cancel` is checked before every block.
#[allow(clippy::too_many_arguments)] // the signature is the contract's (core-types §2.15)
pub fn render_offline<B: RenderBackend + ?Sized>(
    b: &mut B,
    t: &BoundTrack,
    d: &mut Driver,
    from: usize,
    end: RenderPos,
    sink: &mut dyn FnMut(RenderPos, &[f32]),
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<(), Cancelled> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled);
    }
    d.start(b, t, from);
    let mut pos = d.position().map_or(0, |p| p.0);
    progress.store(pos, Ordering::Relaxed);
    let mut block = vec![0.0f32; d.max_block()];
    while pos < end.0 {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let n = (end.0 - pos).min(block.len() as u64) as usize;
        d.process(b, t, &mut block[..n]);
        sink(RenderPos(pos), &block[..n]);
        pos += n as u64;
        progress.store(pos, Ordering::Relaxed);
    }
    Ok(())
}
