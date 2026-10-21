//! Evidence and its `.tkev` container (core-types §2.5): parameter-free
//! per-frame observations at 200 Hz. Nothing a slider changes lives here.
//!
//! Reuse rule, the whole of "never silently recompute under a new model":
//! pinned key in the cache, load it; not cached but the running analyser's
//! provenance hashes to the pinned key, recompute without asking; otherwise
//! `NeedsUpgrade`, the UI asks, and layers keep their WAVs.
//!
//! **NaN means "undefined" in four columns** (a deviation from core-types
//! §2.5, decided by the lead). Spectral centroid and cepstral peak prominence
//! have no value on a silent frame, and brightness in harmonic units or
//! spectral tilt has none without a pitch candidate. Those four columns
//! ([`Column::allows_nan`]: `CentroidHz`, `BrightnessHarm`, `TiltDbOct`,
//! `CppDb`) may hold NaN; every other column, and every onset field, must be
//! finite. Infinities are always refused. [`Evidence::new`] rewrites every NaN
//! to [`CANONICAL_NAN_BITS`], the bit pattern of `f32::NAN`, so the content
//! hash, the container bytes and equality (which is bitwise) are
//! deterministic. The pattern is written out because Rust does not promise
//! `f32::NAN`'s bits across versions and targets, and content hashes are
//! frozen.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{EvidenceKey, Hash32, TakeId, context};
use crate::key::{KeyError, KeyHash, KeyKind, KeyWriter, RawHasher, key_of};
use crate::params::Params;
use crate::time::{SampleRate, TimelinePos, frames_covering};

/// The one NaN evidence holds: a positive quiet NaN with no payload, which is
/// `f32::NAN` on every current target. Frozen, since content hashes see it.
pub const CANONICAL_NAN_BITS: u32 = 0x7FC0_0000;

/// The computation that produced the evidence. `params` holds fixed analyser
/// constants only (window sizes, the candidate search's hard bounds); a user's
/// range calibration is lens work and never appears here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub analyzer: String,
    pub version: u32,
    pub models: BTreeMap<String, Hash32>,
    pub params: Params,
}

impl Provenance {
    /// The key of (take, this provenance).
    pub fn key(&self, take: TakeId) -> Result<EvidenceKey, KeyError> {
        struct Input<'a> {
            take: TakeId,
            provenance: &'a Provenance,
        }
        impl KeyHash for Input<'_> {
            fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
                let Self { take, provenance } = self;
                w.record(|r| {
                    r.field("take", take)?;
                    r.field("provenance", *provenance)
                })
            }
        }
        key_of(
            KeyKind::Evidence,
            &Input {
                take,
                provenance: self,
            },
        )
        .map(EvidenceKey::from_hash)
    }
}

/// Every field: analyser name and version, model hashes by name, params by name.
impl KeyHash for Provenance {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            analyzer,
            version,
            models,
            params,
        } = self;
        w.record(|r| {
            r.field("analyzer", analyzer)?;
            r.field("version", version)?;
            r.field("models", models)?;
            r.field("params", params)
        })
    }
}

/// Evidence columns. Names are frozen; the container header lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    /// Top-3 pitch candidates; a missing candidate is 0 Hz at salience 0.
    F0Hz0,
    F0Hz1,
    F0Hz2,
    F0Sal0,
    F0Sal1,
    F0Sal2,
    VoicingP,
    LoudnessDbA,
    /// NaN on a frame with no spectral energy.
    CentroidHz,
    /// NaN on a frame with no pitch candidate.
    BrightnessHarm,
    /// NaN on a frame with no pitch candidate or no spectral energy.
    TiltDbOct,
    /// NaN on a frame with no spectral energy.
    CppDb,
    Vowel0,
    Vowel1,
    ClassPitched,
    ClassNoise,
    ClassTransient,
    ClassSilence,
    Odf,
}

impl Column {
    /// Every column, in Column order.
    pub const ALL: [Column; 19] = [
        Column::F0Hz0,
        Column::F0Hz1,
        Column::F0Hz2,
        Column::F0Sal0,
        Column::F0Sal1,
        Column::F0Sal2,
        Column::VoicingP,
        Column::LoudnessDbA,
        Column::CentroidHz,
        Column::BrightnessHarm,
        Column::TiltDbOct,
        Column::CppDb,
        Column::Vowel0,
        Column::Vowel1,
        Column::ClassPitched,
        Column::ClassNoise,
        Column::ClassTransient,
        Column::ClassSilence,
        Column::Odf,
    ];

    /// The frozen name: the serde name, as the container header and the
    /// content hash write it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::F0Hz0 => "f0_hz0",
            Self::F0Hz1 => "f0_hz1",
            Self::F0Hz2 => "f0_hz2",
            Self::F0Sal0 => "f0_sal0",
            Self::F0Sal1 => "f0_sal1",
            Self::F0Sal2 => "f0_sal2",
            Self::VoicingP => "voicing_p",
            Self::LoudnessDbA => "loudness_db_a",
            Self::CentroidHz => "centroid_hz",
            Self::BrightnessHarm => "brightness_harm",
            Self::TiltDbOct => "tilt_db_oct",
            Self::CppDb => "cpp_db",
            Self::Vowel0 => "vowel0",
            Self::Vowel1 => "vowel1",
            Self::ClassPitched => "class_pitched",
            Self::ClassNoise => "class_noise",
            Self::ClassTransient => "class_transient",
            Self::ClassSilence => "class_silence",
            Self::Odf => "odf",
        }
    }

    /// Whether the column may hold NaN, meaning "undefined on this frame"
    /// (see the module docs). Infinities are refused in every column.
    pub const fn allows_nan(self) -> bool {
        matches!(
            self,
            Self::CentroidHz | Self::BrightnessHarm | Self::TiltDbOct | Self::CppDb
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnsetCandidate {
    pub at: TimelinePos,
    pub strength: f32,
    pub rise_db: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceHeader {
    pub key: EvidenceKey,
    pub take: TakeId,
    pub take_rate: SampleRate,
    pub take_frames: u64,
    pub provenance: Provenance,
}

/// Validated per-frame observations of one take.
///
/// Equality is bitwise on the values: NaN equals NaN and -0.0 differs from
/// 0.0, exactly as the content hash sees them.
#[derive(Clone, Debug)]
pub struct Evidence {
    header: EvidenceHeader,
    n_frames: usize,
    columns: Vec<(Column, Vec<f32>)>,
    onsets: Vec<OnsetCandidate>,
}

impl PartialEq for Evidence {
    fn eq(&self, other: &Self) -> bool {
        let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        let onset_bits = |o: &OnsetCandidate| (o.at, o.strength.to_bits(), o.rise_db.to_bits());
        self.header == other.header
            && self.n_frames == other.n_frames
            && self.columns.len() == other.columns.len()
            && self
                .columns
                .iter()
                .zip(&other.columns)
                .all(|((ca, va), (cb, vb))| ca == cb && bits(va) == bits(vb))
            && self.onsets.len() == other.onsets.len()
            && self
                .onsets
                .iter()
                .zip(&other.onsets)
                .all(|(a, b)| onset_bits(a) == onset_bits(b))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvidenceError {
    /// A column whose length is not `n_frames`.
    ColumnLength(Column),
    /// Columns not unique and in Column order.
    ColumnOrder,
    /// An infinity anywhere, a NaN outside the columns that allow it, or a
    /// NaN in an onset.
    NonFinite,
    /// Onsets not sorted, or outside the take.
    Onsets,
    /// `header.key` is not `provenance.key(take)`, or not the expected key.
    KeyMismatch,
    Key(KeyError),
    /// Not a `.tkev` container, a different container version, or an illegal
    /// length.
    Container(String),
    /// The frame or content hash does not match.
    Corrupt,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ColumnLength(c) => write!(f, "column {c:?} has the wrong length"),
            Self::ColumnOrder => f.write_str("columns are not unique and in Column order"),
            Self::NonFinite => f.write_str("non-finite evidence value"),
            Self::Onsets => f.write_str("onsets are unsorted or outside the take"),
            Self::KeyMismatch => f.write_str("evidence key does not match"),
            Self::Key(e) => write!(f, "evidence key: {e}"),
            Self::Container(m) => write!(f, "evidence container: {m}"),
            Self::Corrupt => f.write_str("evidence container is corrupt"),
        }
    }
}

impl std::error::Error for EvidenceError {}

/// `"TKEV"`.
const MAGIC: [u8; 4] = *b"TKEV";
/// Magic, version and header length.
const PREFIX_BYTES: usize = 12;
/// The trailing blake3 frame hash.
const FRAME_HASH_BYTES: usize = 32;
/// One onset: `at` (u64), strength and rise (f32), little-endian.
const ONSET_BYTES: usize = 16;

/// The container's JSON header, as written.
#[derive(Serialize)]
struct ContainerHeaderOut<'a> {
    header: &'a EvidenceHeader,
    n_frames: u64,
    columns: Vec<Column>,
    n_onsets: u64,
    content_hash: Hash32,
}

/// The container's JSON header, as read.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerHeaderIn {
    header: EvidenceHeader,
    n_frames: u64,
    columns: Vec<Column>,
    n_onsets: u64,
    content_hash: Hash32,
}

fn container_error(msg: impl Into<String>) -> EvidenceError {
    EvidenceError::Container(msg.into())
}

fn le_u32(b: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(b);
    u32::from_le_bytes(a)
}

fn le_u64(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(b);
    u64::from_le_bytes(a)
}

fn le_f32(b: &[u8]) -> f32 {
    f32::from_bits(le_u32(b))
}

impl Evidence {
    /// Invariants: `n_frames == frames_covering(take_frames, take_rate)`; every
    /// column has `n_frames` values; columns are unique and in Column order;
    /// values are finite except NaN in the columns that allow it (rewritten to
    /// [`CANONICAL_NAN_BITS`]); onsets are finite, sorted by `at` and inside the
    /// take; `header.key == provenance.key(take)`.
    pub fn new(
        header: EvidenceHeader,
        mut columns: Vec<(Column, Vec<f32>)>,
        onsets: Vec<OnsetCandidate>,
    ) -> Result<Self, EvidenceError> {
        let key = header
            .provenance
            .key(header.take)
            .map_err(EvidenceError::Key)?;
        if key != header.key {
            return Err(EvidenceError::KeyMismatch);
        }
        let frames = frames_covering(header.take_frames, header.take_rate);
        for (i, (c, values)) in columns.iter().enumerate() {
            if i > 0 && columns[i - 1].0 >= *c {
                return Err(EvidenceError::ColumnOrder);
            }
            if values.len() as u64 != frames {
                return Err(EvidenceError::ColumnLength(*c));
            }
        }
        // Unreachable on 64-bit targets: a u64 sample count gives under 2^62 frames.
        let n_frames = usize::try_from(frames)
            .map_err(|_| container_error("more frames than this platform can address"))?;
        for (c, values) in &mut columns {
            let nan_ok = c.allows_nan();
            for v in values.iter_mut() {
                if v.is_nan() && nan_ok {
                    *v = f32::from_bits(CANONICAL_NAN_BITS);
                } else if !v.is_finite() {
                    return Err(EvidenceError::NonFinite);
                }
            }
        }
        for (i, o) in onsets.iter().enumerate() {
            if !(o.strength.is_finite() && o.rise_db.is_finite()) {
                return Err(EvidenceError::NonFinite);
            }
            if o.at.0 >= header.take_frames || (i > 0 && onsets[i - 1].at > o.at) {
                return Err(EvidenceError::Onsets);
            }
        }
        Ok(Self {
            header,
            n_frames,
            columns,
            onsets,
        })
    }

    pub fn header(&self) -> &EvidenceHeader {
        &self.header
    }

    pub fn n_frames(&self) -> usize {
        self.n_frames
    }

    /// An analyser may omit columns (analyser v1 has no vowel or class hints).
    pub fn column(&self, c: Column) -> Option<&[f32]> {
        self.columns
            .binary_search_by_key(&c, |(col, _)| *col)
            .ok()
            .map(|i| self.columns[i].1.as_slice())
    }

    /// The columns present, in Column order.
    pub fn columns(&self) -> impl Iterator<Item = Column> + '_ {
        self.columns.iter().map(|(c, _)| *c)
    }

    pub fn onsets(&self) -> &[OnsetCandidate] {
        &self.onsets
    }

    /// Raw framing of `n_frames`, column names and bits, and onsets: what was
    /// produced. The header (what was asked for) is outside it.
    pub fn content_hash(&self) -> Hash32 {
        let Self {
            header: _,
            n_frames,
            columns,
            onsets,
        } = self;
        let mut h = RawHasher::new(context::EVIDENCE_CONTENT);
        h.count(*n_frames);
        h.count(columns.len());
        for (c, values) in columns {
            h.str(c.name());
            h.f32s(values);
        }
        h.count(onsets.len());
        for o in onsets {
            let OnsetCandidate {
                at,
                strength,
                rise_db,
            } = o;
            h.u64(at.0);
            h.f32(*strength);
            h.f32(*rise_db);
        }
        h.finish()
    }

    /// `"TKEV"`, version (u32), header length (u32), header JSON, the columns
    /// as f32 LE in Column order, the onsets as (u64, f32, f32) LE, then a
    /// blake3 hash of everything before.
    pub fn to_container(&self) -> Vec<u8> {
        let head = ContainerHeaderOut {
            header: &self.header,
            n_frames: self.n_frames as u64,
            columns: self.columns().collect(),
            n_onsets: self.onsets.len() as u64,
            content_hash: self.content_hash(),
        };
        // The header holds strings, integers, hashes and finite params (the
        // key check in `new` refuses non-finite ones), so it always serialises.
        // The exact-f32 writer keeps every Float param, so the key holds.
        let json = crate::project::json::to_vec(&head).expect("an evidence header serialises");
        let head_len = u32::try_from(json.len()).expect("an evidence header is under 4 GiB");
        let body = 4 * self.n_frames * self.columns.len() + ONSET_BYTES * self.onsets.len();
        let mut out = Vec::with_capacity(PREFIX_BYTES + json.len() + body + FRAME_HASH_BYTES);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
        out.extend_from_slice(&head_len.to_le_bytes());
        out.extend_from_slice(&json);
        for (_, values) in &self.columns {
            for v in values {
                out.extend_from_slice(&v.to_bits().to_le_bytes());
            }
        }
        for o in &self.onsets {
            out.extend_from_slice(&o.at.0.to_le_bytes());
            out.extend_from_slice(&o.strength.to_bits().to_le_bytes());
            out.extend_from_slice(&o.rise_db.to_bits().to_le_bytes());
        }
        let frame = blake3::derive_key(context::EVIDENCE_CONTAINER, &out);
        out.extend_from_slice(&frame);
        out
    }

    /// Checks the frame hash, magic, version, legal length, finiteness, onset
    /// order, content hash, and that the stored key equals `expected`, so a
    /// file is never a silent substitute. Any error is a cache miss.
    pub fn from_container(bytes: &[u8], expected: EvidenceKey) -> Result<Self, EvidenceError> {
        if bytes.len() < PREFIX_BYTES + FRAME_HASH_BYTES {
            return Err(container_error("shorter than an empty container"));
        }
        let (content, frame) = bytes.split_at(bytes.len() - FRAME_HASH_BYTES);
        if blake3::derive_key(context::EVIDENCE_CONTAINER, content) != frame {
            return Err(EvidenceError::Corrupt);
        }
        if content[..4] != MAGIC {
            return Err(container_error("not a .tkev container"));
        }
        let version = le_u32(&content[4..8]);
        if version != CONTAINER_VERSION {
            return Err(container_error(format!(
                "container version {version}, this build reads {CONTAINER_VERSION}"
            )));
        }
        let head_len = le_u32(&content[8..12]) as usize;
        let rest = &content[PREFIX_BYTES..];
        if head_len > rest.len() {
            return Err(container_error("header length beyond the container"));
        }
        let (head, body) = rest.split_at(head_len);
        let head: ContainerHeaderIn =
            serde_json::from_slice(head).map_err(|e| container_error(format!("header: {e}")))?;
        if head.header.key != expected {
            return Err(EvidenceError::KeyMismatch);
        }
        let frames = frames_covering(head.header.take_frames, head.header.take_rate);
        if head.n_frames != frames {
            return Err(container_error("frame count does not match the take"));
        }
        let column_bytes = frames
            .checked_mul(4)
            .and_then(|b| b.checked_mul(head.columns.len() as u64));
        let onset_bytes = head.n_onsets.checked_mul(ONSET_BYTES as u64);
        let total = column_bytes
            .zip(onset_bytes)
            .and_then(|(c, o)| c.checked_add(o));
        if total != Some(body.len() as u64) {
            return Err(container_error("body length does not match the header"));
        }
        // The body length bounds every size below by the container's own size.
        let (column_body, onset_body) = body.split_at(column_bytes.unwrap_or(0) as usize);
        let per_column = if head.columns.is_empty() {
            0
        } else {
            4 * frames as usize
        };
        let columns = head
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let raw = &column_body[i * per_column..(i + 1) * per_column];
                (*c, raw.chunks_exact(4).map(le_f32).collect())
            })
            .collect();
        let onsets = onset_body
            .chunks_exact(ONSET_BYTES)
            .map(|b| OnsetCandidate {
                at: TimelinePos(le_u64(&b[..8])),
                strength: le_f32(&b[8..12]),
                rise_db: le_f32(&b[12..16]),
            })
            .collect();
        let evidence = Self::new(head.header, columns, onsets)?;
        if evidence.content_hash() != head.content_hash {
            return Err(EvidenceError::Corrupt);
        }
        Ok(evidence)
    }
}

pub const CONTAINER_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceResolution {
    UseCached,
    RecomputeSame,
    NeedsUpgrade {
        pinned: EvidenceKey,
        available: EvidenceKey,
    },
}

/// The reuse rule: pinned key in the cache, `UseCached`; not cached but the
/// running analyser's provenance hashes to the pinned key, `RecomputeSame`;
/// otherwise `NeedsUpgrade`, and the UI asks.
pub fn resolve_evidence(
    pinned: EvidenceKey,
    cache_has_pinned: bool,
    take: TakeId,
    running: &Provenance,
) -> Result<EvidenceResolution, KeyError> {
    if cache_has_pinned {
        return Ok(EvidenceResolution::UseCached);
    }
    let available = running.key(take)?;
    Ok(if available == pinned {
        EvidenceResolution::RecomputeSame
    } else {
        EvidenceResolution::NeedsUpgrade { pinned, available }
    })
}
