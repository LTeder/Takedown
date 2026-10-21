//! The MIDI projection's types (core-types §2.9): RoleMap, MpeProfile,
//! MidiBinding and QuantiseReport. `quantise`, `pitch_to_wire` and the lattice
//! functions arrive with the midi step.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::control::Lane;
use crate::hit::RoleName;
use crate::musical::TimeMap;

/// Role <-> MIDI key, injective. `fallback` is one of the keys; roles missing
/// from the map export as it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleMap {
    #[serde(deserialize_with = "crate::strict::map")]
    pub keys: BTreeMap<RoleName, u8>,
    pub fallback: RoleName,
}

impl RoleMap {
    /// GM drums: kick 36, snare 38, hat_closed 42, hat_open 46, clap 39, rim 37,
    /// tom_low 45, tom_high 50, crash 49, perc 56 (the fallback).
    pub fn gm_default() -> Self {
        let role = |s: &str| RoleName::new(s).expect("GM role names are valid");
        let keys = [
            ("kick", 36),
            ("snare", 38),
            ("hat_closed", 42),
            ("hat_open", 46),
            ("clap", 39),
            ("rim", 37),
            ("tom_low", 45),
            ("tom_high", 50),
            ("crash", 49),
            ("perc", 56),
        ]
        .into_iter()
        .map(|(name, key)| (role(name), key))
        .collect();
        Self {
            keys,
            fallback: role("perc"),
        }
    }
}

/// One MPE lower zone. Defaults ([`MpeProfile::default`]): phrase lane on
/// channels 2-4, hit lane on 5-16, bend range ±48, CC74 brightness, CC2
/// breath, CC16/17 vowel; unassigned channels are read as hits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MpeProfile {
    pub version: u16,
    pub bend_range_st: u8,
    /// Zero-based inclusive range, at least 3 channels; default (1, 3).
    pub phrase_channels: (u8, u8),
    /// Zero-based inclusive range; default (4, 15).
    pub hit_channels: (u8, u8),
    pub cc_bright: u8,
    pub cc_breath: u8,
    pub cc_vowel: (u8, u8),
    pub roles: RoleMap,
    pub unassigned: Lane,
}

/// The current [`MpeProfile::version`].
pub const MPE_PROFILE_VERSION: u16 = 1;

/// The fewest channels the phrase lane may have.
pub const MIN_PHRASE_CHANNELS: u8 = 3;
/// The widest pitch-bend range: RPN 0's coarse value is one 7-bit data byte.
pub const MAX_BEND_RANGE_ST: u8 = 127;
/// The member channels of a lower zone, zero-based: channel 0 is the zone's
/// master channel and carries no notes.
pub const MEMBER_CHANNELS: (u8, u8) = (1, 15);
/// Controllers an expression CC may not use: data entry (6, 38), data
/// increment and decrement (96, 97) and the NRPN and RPN selectors (98 to
/// 101), which the RPN 0 setup at tick 0 uses, plus the channel mode
/// messages (120 to 127), which are not controllers.
pub const RESERVED_CCS: [u8; 16] = [
    6, 38, 96, 97, 98, 99, 100, 101, 120, 121, 122, 123, 124, 125, 126, 127,
];

/// An [`MpeProfile`] that no MPE file could carry, or that would not read
/// back as itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpeProfileError {
    /// A role mapped to a key above 127.
    KeyRange(RoleName),
    /// Two roles mapped to one key, so the map is not injective.
    SharedKey(u8),
    /// The fallback role is not one of the keys.
    Fallback(RoleName),
    /// A channel range that is reversed or leaves the member channels 1..=15.
    Channels { lane: Lane, range: (u8, u8) },
    /// The phrase and hit lanes share a channel.
    LanesOverlap,
    /// The phrase lane has fewer than [`MIN_PHRASE_CHANNELS`] channels.
    PhraseTooNarrow,
    /// An expression CC above 127 or in [`RESERVED_CCS`].
    Cc(u8),
    /// Two expressions on one CC.
    SharedCc(u8),
    /// A bend range outside 1..=[`MAX_BEND_RANGE_ST`] semitones.
    BendRange(u8),
}

impl fmt::Display for MpeProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyRange(r) => write!(f, "role {r} maps to a key above 127"),
            Self::SharedKey(k) => write!(f, "two roles map to key {k}"),
            Self::Fallback(r) => write!(f, "fallback role {r} has no key"),
            Self::Channels { lane, range } => write!(
                f,
                "{lane:?} channels {}..={} are reversed or outside 1..=15",
                range.0, range.1
            ),
            Self::LanesOverlap => f.write_str("the phrase and hit lanes share a channel"),
            Self::PhraseTooNarrow => write!(
                f,
                "the phrase lane needs at least {MIN_PHRASE_CHANNELS} channels"
            ),
            Self::Cc(c) => write!(f, "CC {c} cannot carry an expression"),
            Self::SharedCc(c) => write!(f, "two expressions share CC {c}"),
            Self::BendRange(b) => write!(
                f,
                "bend range {b} is outside 1..={MAX_BEND_RANGE_ST} semitones"
            ),
        }
    }
}

impl std::error::Error for MpeProfileError {}

impl RoleMap {
    /// Keys at most 127 and injective; the fallback is one of the roles.
    pub fn validate(&self) -> Result<(), MpeProfileError> {
        let mut used = BTreeSet::new();
        for (role, &key) in &self.keys {
            if key > 127 {
                return Err(MpeProfileError::KeyRange(role.clone()));
            }
            if !used.insert(key) {
                return Err(MpeProfileError::SharedKey(key));
            }
        }
        if self.keys.contains_key(&self.fallback) {
            Ok(())
        } else {
            Err(MpeProfileError::Fallback(self.fallback.clone()))
        }
    }
}

impl MpeProfile {
    /// Everything an MPE file under this profile needs to carry and read
    /// back: the role map is valid; both lanes are ordered ranges of member
    /// channels (1..=15) that do not overlap, the phrase lane at least
    /// [`MIN_PHRASE_CHANNELS`] wide; the four expression CCs are distinct
    /// controllers outside [`RESERVED_CCS`]; the bend range is 1..=127.
    pub fn validate(&self) -> Result<(), MpeProfileError> {
        let Self {
            version: _,
            bend_range_st,
            phrase_channels,
            hit_channels,
            cc_bright,
            cc_breath,
            cc_vowel,
            roles,
            unassigned: _,
        } = self;
        roles.validate()?;
        for (lane, range) in [
            (Lane::Phrase, *phrase_channels),
            (Lane::Hits, *hit_channels),
        ] {
            let (lo, hi) = range;
            if lo > hi || lo < MEMBER_CHANNELS.0 || hi > MEMBER_CHANNELS.1 {
                return Err(MpeProfileError::Channels { lane, range });
            }
        }
        if phrase_channels.0 <= hit_channels.1 && hit_channels.0 <= phrase_channels.1 {
            return Err(MpeProfileError::LanesOverlap);
        }
        if phrase_channels.1 - phrase_channels.0 + 1 < MIN_PHRASE_CHANNELS {
            return Err(MpeProfileError::PhraseTooNarrow);
        }
        let mut ccs = BTreeSet::new();
        for cc in [*cc_bright, *cc_breath, cc_vowel.0, cc_vowel.1] {
            if cc > 127 || RESERVED_CCS.contains(&cc) {
                return Err(MpeProfileError::Cc(cc));
            }
            if !ccs.insert(cc) {
                return Err(MpeProfileError::SharedCc(cc));
            }
        }
        if (1..=MAX_BEND_RANGE_ST).contains(bend_range_st) {
            Ok(())
        } else {
            Err(MpeProfileError::BendRange(*bend_range_st))
        }
    }
}

impl Default for MpeProfile {
    fn default() -> Self {
        Self {
            version: MPE_PROFILE_VERSION,
            bend_range_st: 48,
            phrase_channels: (1, 3),
            hit_channels: (4, 15),
            cc_bright: 74,
            cc_breath: 2,
            cc_vowel: (16, 17),
            roles: RoleMap::gm_default(),
            unassigned: Lane::Hits,
        }
    }
}

/// How an interpretation serialises to MIDI. It affects export only and stays
/// outside InterpKey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiBinding {
    pub profile: MpeProfile,
    pub time: TimeMap,
}

impl MidiBinding {
    /// The profile's checks. A `TimeMap` is valid by construction: its
    /// constructor, which deserialisation goes through, checks PPQ and the
    /// tempo map.
    pub fn validate(&self) -> Result<(), MpeProfileError> {
        self.profile.validate()
    }

    /// The default profile under [`TimeMap::default_export`].
    pub fn default_export() -> Self {
        Self {
            profile: MpeProfile::default(),
            time: TimeMap::default_export(),
        }
    }
}

/// What `quantise` changed beyond the lattice, for the export report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuantiseReport {
    pub choked_hits: u32,
    pub dropped_spans: u32,
    pub clamped_frames: u32,
    pub remapped_roles: u32,
}
