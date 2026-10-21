//! The seed tree and counter-based noise (core-types §2.3, principle 6).
//!
//! Rolling one aspect changes only that aspect's seeds; a lock just means
//! "this counter does not move". Region rolls fold only into per-event seeds
//! whose anchor lies inside the region, never into voice seeds.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::control::Lane;
use crate::ids::{HexVisitor, context, encode_hex};
use crate::key::{KeyError, KeyHash, KeyWriter};
use crate::time::{TimelinePos, TimelineSpan};

/// A 64-bit seed, serialised as 16 lowercase hex digits because JSON numbers
/// lose precision above 2^53.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// blake3 `derive_key` "takedown 2026 seed-derive v1" over the parent (LE),
    /// the label (u64 length LE, then bytes) and `idx` (LE); the first 8 bytes,
    /// LE. Called at bind or lens time, never on the audio thread.
    #[must_use]
    pub fn derive(self, label: &str, idx: u64) -> Seed {
        let mut h = blake3::Hasher::new_derive_key(context::SEED_DERIVE);
        h.update(&self.0.to_le_bytes());
        h.update(&(label.len() as u64).to_le_bytes());
        h.update(label.as_bytes());
        h.update(&idx.to_le_bytes());
        let mut first = [0u8; 8];
        first.copy_from_slice(&h.finalize().as_bytes()[..8]);
        Seed(u64::from_le_bytes(first))
    }

    /// Noise stream `stream` of this seed: two integer mixes, fine on the
    /// audio thread. Distinct (seed, stream) pairs give distinct streams.
    pub fn noise(self, stream: u64) -> NoiseStream {
        let a = mix(self.0 ^ stream.wrapping_mul(GOLDEN_GAMMA));
        // value(n) is 0 exactly where mix(n + a) == b, i.e. at n = mix⁻¹(b) - a.
        // Deriving b from mix(a) and a salt puts that one index at a
        // seed-dependent place near 2^63; `mix(a ^ stream)` put it at n = 0 of
        // stream 0 for every seed.
        let b = mix(mix(a) ^ stream ^ B_SALT);
        NoiseStream { a, b }
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Seed({:016x})", self.0)
    }
}

impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut buf = [0u8; 16];
        encode_hex(&self.0.to_be_bytes(), &mut buf);
        let text = std::str::from_utf8(&buf).map_err(serde::ser::Error::custom)?;
        s.serialize_str(text)
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(HexVisitor::<8>)
            .map(|b| Seed(u64::from_be_bytes(b)))
    }
}

impl KeyHash for Seed {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.int(i128::from(self.0));
        Ok(())
    }
}

/// The SplitMix64 increment.
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
/// Fractional bits of sqrt(2).
const B_SALT: u64 = 0x6A09_E667_F3BC_C909;

/// The SplitMix64 finaliser: a bijection on u64.
#[inline]
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Counter-based noise: `value(n) = mix(mix(n + a) ^ b)`, integer only. Random
/// access equals sequential reading under any chunking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseStream {
    a: u64,
    b: u64,
}

impl NoiseStream {
    #[inline]
    pub fn u64_at(self, n: u64) -> u64 {
        mix(mix(n.wrapping_add(self.a)) ^ self.b)
    }

    /// [0, 1) on a 2^-24 grid; exact.
    #[inline]
    pub fn unit_at(self, n: u64) -> f32 {
        (self.u64_at(n) >> 40) as f32 * (1.0 / 16_777_216.0)
    }

    /// [-1, 1) on a 2^-23 grid; exact.
    #[inline]
    pub fn bipolar_at(self, n: u64) -> f32 {
        ((self.u64_at(n) >> 40) as i32 - (1 << 23)) as f32 * (1.0 / 8_388_608.0)
    }
}

/// Dice aspects. Their labels feed every seed and are frozen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    Body,
    Mangle,
    TimingFeel,
    MouthAmount,
    PitchReading,
}

impl Aspect {
    pub const ALL: [Aspect; 5] = [
        Aspect::Body,
        Aspect::Mangle,
        Aspect::TimingFeel,
        Aspect::MouthAmount,
        Aspect::PitchReading,
    ];

    /// The frozen derivation label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Body => "aspect/body",
            Self::Mangle => "aspect/mangle",
            Self::TimingFeel => "aspect/timing_feel",
            Self::MouthAmount => "aspect/mouth_amount",
            Self::PitchReading => "aspect/pitch_reading",
        }
    }

    /// The frozen name used in keys and on disk.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Mangle => "mangle",
            Self::TimingFeel => "timing_feel",
            Self::MouthAmount => "mouth_amount",
            Self::PitchReading => "pitch_reading",
        }
    }
}

impl KeyHash for Aspect {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.variant(self.name(), |_| Ok(()))
    }
}

/// A time-anchored reroll of one aspect. The UI arrives in v0.3; the type and
/// its semantics exist now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionRoll {
    pub aspect: Aspect,
    pub span: TimelineSpan,
    pub roll: u32,
}

impl KeyHash for RegionRoll {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { aspect, span, roll } = self;
        w.record(|r| {
            r.field("aspect", aspect)?;
            r.field("span", span)?;
            r.field("roll", roll)
        })
    }
}

/// The per-interpretation dice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedTree {
    /// Supplied by app.
    pub root: Seed,
    /// A missing entry means 0; zero entries are removed.
    pub rolls: BTreeMap<Aspect, u32>,
    /// The dice skip these. Outside keys.
    pub locked: BTreeSet<Aspect>,
    /// Sorted by (aspect, span), at most one entry per (aspect, span), no zero rolls.
    pub regions: Vec<RegionRoll>,
}

/// What a backend receives at reset and with each trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoiceSeeds {
    pub body: Seed,
    pub mangle: Seed,
}

/// A SeedTree that breaks its invariants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedError {
    /// A zero entry in `rolls`.
    ZeroRoll(Aspect),
    /// Region `i` has roll 0.
    ZeroRegionRoll(usize),
    /// Region `i` is not after region `i - 1` in (aspect, span) order.
    RegionOrder(usize),
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRoll(a) => write!(f, "zero roll stored for aspect {}", a.name()),
            Self::ZeroRegionRoll(i) => write!(f, "region roll {i} is zero"),
            Self::RegionOrder(i) => write!(f, "region roll {i} is out of order or duplicated"),
        }
    }
}

impl std::error::Error for SeedError {}

fn region_order(r: &RegionRoll) -> (Aspect, TimelineSpan) {
    (r.aspect, r.span)
}

impl SeedTree {
    /// No rolls, no locks, no regions.
    pub fn new(root: Seed) -> Self {
        Self {
            root,
            rolls: BTreeMap::new(),
            locked: BTreeSet::new(),
            regions: Vec::new(),
        }
    }

    /// Checks the invariants that deserialisation cannot.
    pub fn validate(&self) -> Result<(), SeedError> {
        if let Some((a, _)) = self.rolls.iter().find(|(_, n)| **n == 0) {
            return Err(SeedError::ZeroRoll(*a));
        }
        for (i, r) in self.regions.iter().enumerate() {
            if r.roll == 0 {
                return Err(SeedError::ZeroRegionRoll(i));
            }
            if i > 0 && region_order(&self.regions[i - 1]) >= region_order(r) {
                return Err(SeedError::RegionOrder(i));
            }
        }
        Ok(())
    }

    fn roll_of(&self, a: Aspect) -> u32 {
        self.rolls.get(&a).copied().unwrap_or(0)
    }

    /// `root.derive(label(a), roll(a))`.
    pub fn aspect(&self, a: Aspect) -> Seed {
        self.root.derive(a.label(), u64::from(self.roll_of(a)))
    }

    /// `aspect(a)`, folded in (aspect, span) order with each non-zero region
    /// roll of `a` whose span contains `at`.
    pub fn aspect_at(&self, a: Aspect, at: TimelinePos) -> Seed {
        self.regions
            .iter()
            .filter(|r| r.aspect == a && r.roll != 0 && r.span.contains(at))
            .fold(self.aspect(a), |s, r| {
                s.derive("region/start", r.span.start().0)
                    .derive("region/end", r.span.end().0)
                    .derive("region/roll", u64::from(r.roll))
            })
    }

    /// Returns false and changes nothing when `a` is locked.
    pub fn roll(&mut self, a: Aspect) -> bool {
        if self.locked.contains(&a) {
            return false;
        }
        let next = self.roll_of(a).wrapping_add(1);
        if next == 0 {
            self.rolls.remove(&a);
        } else {
            self.rolls.insert(a, next);
        }
        true
    }

    /// Rolls every unlocked aspect.
    pub fn roll_unlocked(&mut self) {
        for a in Aspect::ALL {
            self.roll(a);
        }
    }

    /// Rolls `a` inside `span` only. Like [`roll`](Self::roll), does nothing
    /// when `a` is locked.
    pub fn roll_region(&mut self, a: Aspect, span: TimelineSpan) {
        if self.locked.contains(&a) {
            return;
        }
        match self.regions.binary_search_by_key(&(a, span), region_order) {
            Ok(i) => {
                let next = self.regions[i].roll.wrapping_add(1);
                if next == 0 {
                    self.regions.remove(i);
                } else {
                    self.regions[i].roll = next;
                }
            }
            Err(i) => self.regions.insert(
                i,
                RegionRoll {
                    aspect: a,
                    span,
                    roll: 1,
                },
            ),
        }
    }

    /// `aspect(Body | Mangle).derive(voice, 0)`; never folds region rolls.
    pub fn voice_seeds(&self, voice: &str) -> VoiceSeeds {
        VoiceSeeds {
            body: self.aspect(Aspect::Body).derive(voice, 0),
            mangle: self.aspect(Aspect::Mangle).derive(voice, 0),
        }
    }

    /// `aspect_at(Body | Mangle, anchor).derive(voice, 0).derive(lane label,
    /// anchor).derive("ordinal", ordinal)`. `ordinal` counts the earlier events
    /// with the same (lane, anchor) in canonical order.
    pub fn trigger_seeds(
        &self,
        voice: &str,
        lane: Lane,
        anchor: TimelinePos,
        ordinal: u32,
    ) -> VoiceSeeds {
        let per_event = |a: Aspect| {
            self.aspect_at(a, anchor)
                .derive(voice, 0)
                .derive(lane.label(), anchor.0)
                .derive("ordinal", u64::from(ordinal))
        };
        VoiceSeeds {
            body: per_event(Aspect::Body),
            mangle: per_event(Aspect::Mangle),
        }
    }
}

/// Keys the non-zero entries of `rolls`.
struct NonZeroRolls<'a>(&'a BTreeMap<Aspect, u32>);

impl KeyHash for NonZeroRolls<'_> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.map(self.0.iter().filter(|(_, n)| **n != 0))
    }
}

/// Keys the non-zero region rolls, as a set.
struct NonZeroRegions<'a>(&'a [RegionRoll]);

impl KeyHash for NonZeroRegions<'_> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.set(self.0.iter().filter(|r| r.roll != 0))
    }
}

/// Root, non-zero rolls and region rolls; locks are outside keys.
impl KeyHash for SeedTree {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            root,
            rolls,
            locked: _,
            regions,
        } = self;
        w.record(|r| {
            r.field("root", root)?;
            r.field("rolls", &NonZeroRolls(rolls))?;
            r.field("regions", &NonZeroRegions(regions))
        })
    }
}
