//! Layers and render keys (core-types §2.13). A render is a mono f32 WAV at
//! `renders/<WavId>.wav`, outside `cache/`. A stale render is still the durable
//! artefact: it stays playable and exportable. One slot per output rate, so a
//! rate switch never discards a render. Gain and mute are outside the key;
//! platform class is provenance only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{Hash32, InterpId, InterpKey, LayerId, RenderKey, WavId};
use crate::key::{KeyError, KeyHash, KeyKind, KeyWriter, SetOf, key_of};
use crate::time::SampleRate;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendStamp {
    pub id: String,
    pub version: u32,
}

impl KeyHash for BackendStamp {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { id, version } = self;
        w.record(|r| {
            r.field("id", id)?;
            r.field("version", version)
        })
    }
}

/// Everything that determines a render's bits, as the running build sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderInputs {
    pub interp: InterpKey,
    /// Content hash of the evidence actually used; `None` for Imported.
    pub evidence: Option<Hash32>,
    /// Content hash of the hit set actually used.
    pub hits: Option<Hash32>,
    /// The running `render::lens::LENS_VERSION`.
    pub lens_version: u32,
    /// Sorted and unique: the backends actually used.
    pub backends: Vec<BackendStamp>,
    /// The running `RENDER_CONTRACT_VERSION`.
    pub contract: u32,
    pub rate: SampleRate,
}

/// Every field. Backends are keyed as a set: their order is irrelevant and a
/// duplicate is `DuplicateKey`.
impl KeyHash for RenderInputs {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            interp,
            evidence,
            hits,
            lens_version,
            backends,
            contract,
            rate,
        } = self;
        w.record(|r| {
            r.field("interp", interp)?;
            r.opt("evidence", evidence.as_ref())?;
            r.opt("hits", hits.as_ref())?;
            r.field("lens_version", lens_version)?;
            r.field("backends", &SetOf(backends))?;
            r.field("contract", contract)?;
            r.field("rate", rate)
        })
    }
}

impl RenderInputs {
    pub fn key(&self) -> Result<RenderKey, KeyError> {
        key_of(KeyKind::Render, self).map(RenderKey::from_hash)
    }
}

/// Provenance; outside the key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformClass {
    pub arch: String,
    pub os: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderRef {
    pub key: RenderKey,
    pub inputs: RenderInputs,
    pub wav: WavId,
    pub frames: u64,
    pub platform: PlatformClass,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: LayerId,
    pub interp: InterpId,
    pub gain_db: f32,
    pub mute: bool,
    #[serde(deserialize_with = "crate::strict::map")]
    pub renders: BTreeMap<SampleRate, RenderRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaleReason {
    InterpretationChanged,
    EvidenceContentChanged,
    HitsChanged,
    LensUpgraded {
        from: u32,
        to: u32,
    },
    BackendsChanged {
        from: Vec<BackendStamp>,
        to: Vec<BackendStamp>,
    },
    ContractUpgraded {
        from: u32,
        to: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerStatus {
    NotRendered,
    Fresh,
    Stale(Vec<StaleReason>),
}

fn sorted(b: &[BackendStamp]) -> Vec<BackendStamp> {
    let mut v = b.to_vec();
    v.sort();
    v
}

impl Layer {
    /// Compares against the slot at `current.rate`. Every input that differs
    /// gives its reason, in field order; backends compare as sets, like the
    /// key. Fresh exactly when the slot's inputs key like `current`.
    pub fn status(&self, current: &RenderInputs) -> LayerStatus {
        let Some(slot) = self.renders.get(&current.rate) else {
            return LayerStatus::NotRendered;
        };
        let RenderInputs {
            interp,
            evidence,
            hits,
            lens_version,
            backends,
            contract,
            rate: _,
        } = &slot.inputs;
        let mut reasons = Vec::new();
        if *interp != current.interp {
            reasons.push(StaleReason::InterpretationChanged);
        }
        if *evidence != current.evidence {
            reasons.push(StaleReason::EvidenceContentChanged);
        }
        if *hits != current.hits {
            reasons.push(StaleReason::HitsChanged);
        }
        if *lens_version != current.lens_version {
            reasons.push(StaleReason::LensUpgraded {
                from: *lens_version,
                to: current.lens_version,
            });
        }
        let (from, to) = (sorted(backends), sorted(&current.backends));
        if from != to {
            reasons.push(StaleReason::BackendsChanged { from, to });
        }
        if *contract != current.contract {
            reasons.push(StaleReason::ContractUpgraded {
                from: *contract,
                to: current.contract,
            });
        }
        if reasons.is_empty() {
            LayerStatus::Fresh
        } else {
            LayerStatus::Stale(reasons)
        }
    }

    /// Fills the slot at the render's rate, replacing what was there.
    pub fn store_render(&mut self, r: RenderRef) {
        self.renders.insert(r.inputs.rate, r);
    }
}
