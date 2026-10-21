//! Takes (core-types §2.4): immutable audio plus capture metadata. Identical
//! PCM is one take.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{Hash32, TakeId, context};
use crate::time::SampleRate;

/// Hashes under [`crate::ids::context::TAKE_PCM`]: rate (u32), channels (u16),
/// "f32le", every sample's bits (LE), then the frame count at finish. The
/// disk-writer thread feeds it during capture, and chunking cannot change the
/// id. NaN or infinity means a capture bug and is rejected: the hasher then
/// refuses every later call, so a take with a bad sample never gets an id.
#[derive(Clone, Debug)]
pub struct TakeHasher {
    hasher: blake3::Hasher,
    channels: u16,
    samples: u64,
    /// The first error; once set, `update` and `finish` return it.
    error: Option<TakeError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeError {
    /// A NaN or infinite sample, at this interleaved sample index.
    NonFinite { sample: u64 },
    /// The sample count is not a whole number of frames.
    PartialFrame,
    /// Zero channels.
    Channels,
}

impl fmt::Display for TakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { sample } => write!(f, "non-finite sample at index {sample}"),
            Self::PartialFrame => f.write_str("sample count is not a whole number of frames"),
            Self::Channels => f.write_str("a take needs at least one channel"),
        }
    }
}

impl std::error::Error for TakeError {}

/// Samples converted to bytes per `blake3::Hasher::update` call.
const BATCH: usize = 1024;

impl TakeHasher {
    pub fn new(rate: SampleRate, channels: u16) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(context::TAKE_PCM);
        hasher.update(&rate.hz().to_le_bytes());
        hasher.update(&channels.to_le_bytes());
        hasher.update(b"f32le");
        Self {
            hasher,
            channels,
            samples: 0,
            error: None,
        }
    }

    /// Feeds interleaved samples. A chunk need not hold whole frames; only the
    /// total is checked, at [`finish`](Self::finish). A chunk holding a NaN or
    /// an infinity is rejected whole, and the hasher stays failed.
    pub fn update(&mut self, interleaved: &[f32]) -> Result<(), TakeError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if self.channels == 0 {
            self.error = Some(TakeError::Channels);
            return Err(TakeError::Channels);
        }
        if let Some(i) = interleaved.iter().position(|x| !x.is_finite()) {
            let e = TakeError::NonFinite {
                sample: self.samples + i as u64,
            };
            self.error = Some(e);
            return Err(e);
        }
        let mut buf = [0u8; 4 * BATCH];
        for chunk in interleaved.chunks(BATCH) {
            for (out, x) in buf.as_chunks_mut::<4>().0.iter_mut().zip(chunk) {
                *out = x.to_bits().to_le_bytes();
            }
            self.hasher.update(&buf[..4 * chunk.len()]);
        }
        self.samples += interleaved.len() as u64;
        Ok(())
    }

    /// The id and the frame count.
    pub fn finish(self) -> Result<(TakeId, u64), TakeError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if self.channels == 0 {
            return Err(TakeError::Channels);
        }
        let channels = u64::from(self.channels);
        if !self.samples.is_multiple_of(channels) {
            return Err(TakeError::PartialFrame);
        }
        let frames = self.samples / channels;
        let mut hasher = self.hasher;
        hasher.update(&frames.to_le_bytes());
        Ok((
            TakeId::from_hash(Hash32(*hasher.finalize().as_bytes())),
            frames,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInfo {
    pub rate: SampleRate,
    pub channels: u16,
    pub frames: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureSource {
    Recorded {
        device: String,
        latency_compensated: bool,
    },
    Dropped {
        original_name: String,
        source_format: String,
    },
}

/// What the host reported at capture. None in v0.1. Not keyed, so f64 is safe
/// here (float_roundtrip).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostAnchor {
    pub host_rate: SampleRate,
    pub host_sample_at_start: i64,
    pub ppq_at_start: f64,
    pub tempo_bpm: f64,
    pub time_sig: (u8, u8),
}

impl HostAnchor {
    /// Whether both f64 fields are finite, so the anchor survives JSON.
    pub fn is_finite(&self) -> bool {
        self.ppq_at_start.is_finite() && self.tempo_bpm.is_finite()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Take {
    /// Content identity; stored as `audio/<id>.wav` (f32 WAV at `audio.rate`), immutable.
    pub id: TakeId,
    /// `audio.rate` is the timeline rate of everything anchored to this take.
    pub audio: AudioInfo,
    pub source: CaptureSource,
    pub anchor: Option<HostAnchor>,
    /// Display only.
    pub name: String,
    /// RFC 3339, display only, supplied by app.
    pub captured_at: String,
}
