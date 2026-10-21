//! The MIDI projection's types (core-types §2.9): RoleMap, MpeProfile,
//! MidiBinding and QuantiseReport. `quantise`, `pitch_to_wire` and the lattice
//! functions arrive with the midi step.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::control::Lane;
use crate::hit::RoleName;
use crate::musical::TimeMap;

/// Role <-> MIDI key, injective. `fallback` is one of the keys; roles missing
/// from the map export as it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleMap {
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
