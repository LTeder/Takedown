//! Identities (core-types §2.2): blake3 digests behind typed newtypes that
//! cannot be swapped for one another, and local ids from the project counter.

use std::fmt;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::key::{KeyError, KeyHash, KeyWriter};

/// The frozen blake3 `derive_key` contexts. Changing one re-keys every stored
/// object of that kind, so they never change; a new meaning gets a new context.
pub mod context {
    /// [`super::TakeId`]: `TakeHasher` over framed PCM.
    pub const TAKE_PCM: &str = "takedown 2026-10 take-pcm v1";
    /// [`crate::key::KeyKind::Evidence`].
    pub const EVIDENCE: &str = "takedown 2026-10 evidence v1";
    /// [`crate::key::KeyKind::HitSet`].
    pub const HIT_SET: &str = "takedown 2026-10 hit-set v1";
    /// [`crate::key::KeyKind::TeachSet`].
    pub const TEACH_SET: &str = "takedown 2026-10 teach-set v1";
    /// [`crate::key::KeyKind::Interp`].
    pub const INTERP: &str = "takedown 2026-10 interp v1";
    /// [`crate::key::KeyKind::Render`].
    pub const RENDER: &str = "takedown 2026-10 render v1";
    /// [`super::ControlId`]: raw framing of a ControlTrack.
    pub const CONTROL: &str = "takedown 2026-10 control v1";
    /// `Evidence::content_hash`: raw framing of what the analyser produced.
    pub const EVIDENCE_CONTENT: &str = "takedown 2026-10 evidence-content v1";
    /// The trailing frame hash of a `.tkev` container.
    pub const EVIDENCE_CONTAINER: &str = "takedown 2026-10 tkev-frame v1";
    /// `HitSet::content_hash`: raw framing of a hit set.
    pub const HIT_SET_CONTENT: &str = "takedown 2026-10 hit-set-content v1";
    /// [`super::PaletteId`]: bytes of `palettes/<id>.json`.
    pub const PALETTE: &str = "takedown 2026-10 palette v1";
    /// [`super::WavId`]: bytes of `renders/<id>.wav`.
    pub const WAV: &str = "takedown 2026-10 wav v1";
    /// [`super::MidiFileId`]: bytes of `midi/<id>.mid`.
    pub const MIDI_FILE: &str = "takedown 2026-10 midi-file v1";
    /// `Seed::derive` (core-types §2.3, frozen as written there).
    pub const SEED_DERIVE: &str = "takedown 2026 seed-derive v1";
}

const HEX: &[u8; 16] = b"0123456789abcdef";

pub(crate) fn encode_hex(bytes: &[u8], out: &mut [u8]) {
    for (i, b) in bytes.iter().enumerate() {
        out[2 * i] = HEX[usize::from(b >> 4)];
        out[2 * i + 1] = HEX[usize::from(b & 0x0F)];
    }
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Lowercase hex only, exactly `2 * out.len()` digits.
pub(crate) fn decode_hex(s: &str, out: &mut [u8]) -> bool {
    let s = s.as_bytes();
    if s.len() != 2 * out.len() {
        return false;
    }
    for (i, o) in out.iter_mut().enumerate() {
        match (nibble(s[2 * i]), nibble(s[2 * i + 1])) {
            (Some(hi), Some(lo)) => *o = (hi << 4) | lo,
            _ => return false,
        }
    }
    true
}

/// Deserialises exactly `2 * N` lowercase hex digits.
pub(crate) struct HexVisitor<const N: usize>;

impl<const N: usize> Visitor<'_> for HexVisitor<N> {
    type Value = [u8; N];

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} lowercase hex digits", 2 * N)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<[u8; N], E> {
        let mut out = [0u8; N];
        if decode_hex(v, &mut out) {
            Ok(out)
        } else {
            Err(E::invalid_value(Unexpected::Str(v), &self))
        }
    }
}

/// A blake3 digest, serialised as 64 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    pub fn to_hex(&self) -> String {
        self.to_string()
    }

    /// Exactly 64 lowercase hex digits.
    pub fn from_hex(s: &str) -> Option<Self> {
        let mut out = [0u8; 32];
        decode_hex(s, &mut out).then_some(Self(out))
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 64];
        encode_hex(&self.0, &mut buf);
        // Every byte is an ASCII hex digit.
        f.write_str(std::str::from_utf8(&buf).map_err(|_| fmt::Error)?)
    }
}

impl fmt::Debug for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash32({self})")
    }
}

impl Serialize for Hash32 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut buf = [0u8; 64];
        encode_hex(&self.0, &mut buf);
        let text = std::str::from_utf8(&buf).map_err(serde::ser::Error::custom)?;
        s.serialize_str(text)
    }
}

impl<'de> Deserialize<'de> for Hash32 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(HexVisitor::<32>).map(Self)
    }
}

impl KeyHash for Hash32 {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.bytes(&self.0);
        Ok(())
    }
}

macro_rules! hash_id {
    ($($(#[$m:meta])* $name:ident;)*) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Hash32);

        impl $name {
            pub const fn from_hash(h: Hash32) -> Self {
                Self(h)
            }

            pub const fn hash32(self) -> Hash32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl KeyHash for $name {
            fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
                self.0.write_key(w)
            }
        }
    )*};
}

hash_id! {
    /// Content identity of a take: `TakeHasher` over framed PCM (§2.4).
    TakeId;
    /// Key of (take, Provenance): what was asked for.
    EvidenceKey;
    /// Key of (evidence key, classifier stamp, teach set).
    HitSetKey;
    /// Key of the teach labels.
    TeachSetId;
    /// Key of an Interpretation's render-relevant content (§2.12).
    InterpKey;
    /// Key of RenderInputs (§2.13).
    RenderKey;
    /// Content of an imported ControlTrack (raw framing).
    ControlId;
    /// Bytes of `palettes/<id>.json` as snapshotted.
    PaletteId;
    /// Bytes of `renders/<id>.wav`.
    WavId;
    /// Bytes of `midi/<id>.mid`.
    MidiFileId;
    /// Supplied by app at creation.
    ProjectId;
}

impl PaletteId {
    /// The id of a palette snapshot's bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(Hash32(blake3::derive_key(context::PALETTE, bytes)))
    }
}

impl WavId {
    /// The id of a rendered WAV's bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(Hash32(blake3::derive_key(context::WAV, bytes)))
    }
}

impl MidiFileId {
    /// The id of a `.mid` file's bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(Hash32(blake3::derive_key(context::MIDI_FILE, bytes)))
    }
}

macro_rules! local_id {
    ($($(#[$m:meta])* $name:ident;)*) => {$(
        $(#[$m])*
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);
    )*};
}

local_id! {
    /// A local id from `Project::alloc_id`; never reused, outside every key.
    InterpId;
    /// A local id from `Project::alloc_id`; never reused, outside every key.
    LayerId;
    /// A local id from `Project::alloc_id`; never reused, outside every key.
    EditId;
}
