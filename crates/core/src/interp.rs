//! Interpretations (core-types §2.12): what to render from a take, and how.
//!
//! | InterpKey | Includes | Excludes |
//! |---|---|---|
//! | Lens | take, evidence key, hits key, edit ops in order, palette id, all six sliders, seed root, non-zero rolls, region rolls | id, name, slider locks, aspect locks, edit ids, palette name, MidiBinding |
//! | Imported | control id, take, palette id, Mangle, seed root, non-zero rolls, region rolls | the above, plus the track-shaping sliders and the file id |
//!
//! Edit order stays in the key: edits apply in order, so order is meaning.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::canonical::MidiBinding;
use crate::control::ControlTrack;
use crate::edit::EditEntry;
use crate::evidence::EvidenceHeader;
use crate::ids::{ControlId, EvidenceKey, HitSetKey, InterpId, InterpKey, MidiFileId, TakeId};
use crate::key::{KeyError, KeyHash, KeyKind, KeyWriter, key_of};
use crate::lens::{LensError, LensSettings, Sliders};
use crate::seed::{SeedError, SeedTree};
use crate::take::Take;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interpretation {
    /// Outside keys.
    pub id: InterpId,
    /// Outside keys.
    pub name: String,
    /// Required for Lens; optional for Imported (anchors and slices).
    pub take: Option<TakeId>,
    pub source: ControlSource,
    /// Kept for both sources: palette and Mangle still apply to Imported.
    pub lens: LensSettings,
    /// The dice live here, so an Imported source keeps its rolls.
    pub seeds: SeedTree,
    /// Export only; outside keys.
    pub midi: MidiBinding,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlSource {
    /// Re-derived in milliseconds; the track is never stored.
    Lens {
        evidence: EvidenceKey,
        hits: Option<HitSetKey>,
        edits: Vec<EditEntry>,
    },
    /// An edited .mid, decoded once, with anchors re-attached, stored at
    /// `controls/<control>.json`. The original file stays at `midi/<file>.mid`
    /// as provenance.
    Imported {
        control: ControlId,
        file: Option<MidiFileId>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum InterpError {
    /// A Lens source without a take.
    MissingTake,
    /// A take id the project does not hold.
    UnknownTake(TakeId),
    /// The evidence key was not computed for this interpretation's take.
    EvidenceTake,
    /// A track's timeline rate differs from its take's rate.
    RateMismatch,
    Lens(LensError),
    Seeds(SeedError),
    Key(KeyError),
}

impl fmt::Display for InterpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTake => f.write_str("a lens interpretation needs a take"),
            Self::UnknownTake(t) => write!(f, "unknown take {t}"),
            Self::EvidenceTake => f.write_str("evidence belongs to a different take"),
            Self::RateMismatch => f.write_str("track rate differs from its take's rate"),
            Self::Lens(e) => write!(f, "lens settings: {e}"),
            Self::Seeds(e) => write!(f, "seed tree: {e}"),
            Self::Key(e) => write!(f, "key: {e}"),
        }
    }
}

impl std::error::Error for InterpError {}

/// The lens settings an Imported source keeps in its key: the palette id and
/// Mangle. The track-shaping sliders are inert for a stored track.
struct ImportedLens<'a>(&'a LensSettings);

impl KeyHash for ImportedLens<'_> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let LensSettings {
            palette,
            sliders,
            locks: _,
        } = self.0;
        let Sliders {
            mouth_amount: _,
            fidelity: _,
            mangle,
            timing_feel: _,
            register_st: _,
            glide_retrigger: _,
        } = sliders;
        w.record(|r| {
            r.field("palette", palette)?;
            r.field("mangle", mangle)
        })
    }
}

/// The InterpKey table: a variant per source kind, by its serde name.
impl KeyHash for Interpretation {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            id: _,
            name: _,
            take,
            source,
            lens,
            seeds,
            midi: _,
        } = self;
        match source {
            ControlSource::Lens {
                evidence,
                hits,
                edits,
            } => w.variant("lens", |r| {
                r.opt("take", take.as_ref())?;
                r.field("evidence", evidence)?;
                r.opt("hits", hits.as_ref())?;
                // EditEntry keys its op only; the list keeps order.
                r.field("edits", edits)?;
                r.field("lens", lens)?;
                r.field("seeds", seeds)
            }),
            ControlSource::Imported { control, file: _ } => w.variant("imported", |r| {
                r.field("control", control)?;
                r.opt("take", take.as_ref())?;
                r.field("lens", &ImportedLens(lens))?;
                r.field("seeds", seeds)
            }),
        }
    }
}

impl Interpretation {
    pub fn key(&self) -> Result<InterpKey, KeyError> {
        key_of(KeyKind::Interp, self).map(InterpKey::from_hash)
    }

    /// A Lens source needs a take; every take named must be in `takes`; lens
    /// settings and seeds must be valid; the key must be computable (every
    /// keyed float finite).
    ///
    /// The evidence take and an imported track's rate live outside the
    /// project document, so their checks are [`check_evidence`] and
    /// [`check_track`], run by whoever loads the evidence or the control file.
    ///
    /// [`check_evidence`]: Self::check_evidence
    /// [`check_track`]: Interpretation::check_track
    pub fn validate(&self, takes: &BTreeMap<TakeId, Take>) -> Result<(), InterpError> {
        if let Some(t) = self.take
            && !takes.contains_key(&t)
        {
            return Err(InterpError::UnknownTake(t));
        }
        if matches!(self.source, ControlSource::Lens { .. }) && self.take.is_none() {
            return Err(InterpError::MissingTake);
        }
        self.lens.validate().map_err(InterpError::Lens)?;
        self.seeds.validate().map_err(InterpError::Seeds)?;
        self.key().map_err(InterpError::Key)?;
        Ok(())
    }

    /// The evidence key's take must equal this interpretation's take: `header`
    /// must be the evidence a Lens source names, computed for its take.
    pub fn check_evidence(&self, header: &EvidenceHeader) -> Result<(), InterpError> {
        match &self.source {
            ControlSource::Lens { evidence, .. }
                if *evidence == header.key && self.take == Some(header.take) =>
            {
                Ok(())
            }
            _ => Err(InterpError::EvidenceTake),
        }
    }

    /// A track's timeline rate must equal its take's rate, and `takes` must
    /// hold that take. A track with no take has nothing to check.
    pub fn check_track(
        track: &ControlTrack,
        takes: &BTreeMap<TakeId, Take>,
    ) -> Result<(), InterpError> {
        let Some(t) = track.take() else {
            return Ok(());
        };
        let take = takes.get(&t).ok_or(InterpError::UnknownTake(t))?;
        if take.audio.rate == track.rate() {
            Ok(())
        } else {
            Err(InterpError::RateMismatch)
        }
    }
}
