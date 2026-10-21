//! NoteView and HitView (core-types §2.8): read-only views of a ControlTrack
//! for the MIDI exporter and the display. Derived, never stored; the MIDI
//! importer produces a ControlTrack, never views.
//!
//! A note's boundaries, velocity and link come only from its span, so no curve
//! change can move, split or merge a note. Its key is the rounded lower median
//! of `pitch_st` over the frames it covers, which ignores scoops, vibrato and
//! short octave blips and changes only when the median crosses a half-semitone.

use std::ops::Range;

use crate::canonical::RoleMap;
use crate::control::{ControlTrack, Link, U7, Velocity};
use crate::hit::RoleName;
use crate::time::{SampleRate, TimelinePos, control_index};

/// Exactly one per span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteView {
    pub span: usize,
    pub on: TimelinePos,
    pub off: TimelinePos,
    pub key: u8,
    pub vel: Velocity,
    pub link: Link,
    /// The frames the note covers: those whose `frame_start` lies in `[on, off)`.
    pub frames: Range<u64>,
}

/// One per hit, in canonical order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitView {
    pub at: TimelinePos,
    pub dur: u64,
    pub role: RoleName,
    /// The role's key in the map; `None` when the role is unmapped and would
    /// export as the map's fallback.
    pub key: Option<u8>,
    pub vel: Velocity,
    pub bright: U7,
    pub breath: U7,
    pub mouth: U7,
}

/// The number of frames whose `frame_start` lies before sample `n`, which is
/// also the first frame whose `frame_start` is at or after `n`.
fn first_frame_at_or_after(n: u64, rate: SampleRate) -> u64 {
    match n.checked_sub(1) {
        Some(prev) => control_index(prev, rate).0 + 1,
        None => 0,
    }
}

/// The frames a span `[on, off)` covers: those whose `frame_start` lies in it.
pub fn covered_frames(on: TimelinePos, off: TimelinePos, rate: SampleRate) -> Range<u64> {
    first_frame_at_or_after(on.0, rate)..first_frame_at_or_after(off.0, rate)
}

pub fn note_view(t: &ControlTrack) -> Vec<NoteView> {
    let rate = t.rate();
    let pitch = &t.curves().pitch_st;
    let mut scratch = Vec::new();
    t.spans()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let frames = covered_frames(s.on, s.off, rate);
            let rep = control_index(s.on.0, rate).0;
            NoteView {
                span: i,
                on: s.on,
                off: s.off,
                key: key_with(pitch, frames.clone(), rep, &mut scratch),
                vel: s.vel,
                link: s.link,
                frames,
            }
        })
        .collect()
}

pub fn hit_view(t: &ControlTrack, roles: &RoleMap) -> Vec<HitView> {
    t.hits()
        .iter()
        .map(|h| {
            let role = t.roles()[usize::from(h.role)].clone();
            HitView {
                at: h.at,
                dur: h.dur,
                key: roles.keys.get(&role).copied(),
                role,
                vel: h.vel,
                bright: h.bright,
                breath: h.breath,
                mouth: h.mouth,
            }
        })
        .collect()
}

/// The rounded lower median of `pitch` over `frames`, clamped to 0..=127.
/// With no covered frame (a span under 5 ms), the pitch at frame `rep` (the
/// frame containing `on`). Ranges beyond the curve are clipped to it.
pub fn derive_key(pitch: &[f32], frames: Range<u64>, rep: usize) -> u8 {
    key_with(pitch, frames, rep as u64, &mut Vec::new())
}

fn key_of_pitch(p: f32) -> u8 {
    p.round().clamp(0.0, 127.0) as u8
}

fn key_with(pitch: &[f32], frames: Range<u64>, rep: u64, scratch: &mut Vec<f32>) -> u8 {
    let n = pitch.len() as u64;
    let (start, end) = (frames.start.min(n), frames.end.min(n));
    if start >= end {
        let Some(last) = n.checked_sub(1) else {
            // No curve at all: middle C, rather than a panic.
            return 60;
        };
        return key_of_pitch(pitch[rep.min(last) as usize]);
    }
    scratch.clear();
    scratch.extend_from_slice(&pitch[start as usize..end as usize]);
    let mid = (scratch.len() - 1) / 2;
    let (_, median, _) = scratch.select_nth_unstable_by(mid, f32::total_cmp);
    key_of_pitch(*median)
}
