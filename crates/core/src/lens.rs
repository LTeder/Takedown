//! Lens settings (core-types §2.11): the palette, the sliders and their locks.
//! Versioned by the project schema, not on their own. What a lock prevents is
//! lens and app behaviour; core only stores it.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::PaletteId;
use crate::key::{KeyError, KeyHash, KeyWriter};

/// Names are frozen: they appear in project.json and in keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slider {
    MouthAmount,
    Fidelity,
    Mangle,
    TimingFeel,
    Register,
    GlideRetrigger,
}

impl Slider {
    /// True for all but Mangle; those sliders are inert for an Imported source.
    pub fn shapes_track(self) -> bool {
        self != Slider::Mangle
    }
}

/// The largest register shift, in semitones either way.
pub const MAX_REGISTER_ST: i8 = 36;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sliders {
    /// 0..=1
    pub mouth_amount: f32,
    /// 0..=1
    pub fidelity: f32,
    /// 0..=1
    pub mangle: f32,
    /// 0..=1
    pub timing_feel: f32,
    /// -36..=36
    pub register_st: i8,
    /// -1 (always retrigger) ..= 1 (always glide)
    pub glide_retrigger: f32,
}

/// All six sliders, by their frozen names.
impl KeyHash for Sliders {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            mouth_amount,
            fidelity,
            mangle,
            timing_feel,
            register_st,
            glide_retrigger,
        } = self;
        w.record(|r| {
            r.field("mouth_amount", mouth_amount)?;
            r.field("fidelity", fidelity)?;
            r.field("mangle", mangle)?;
            r.field("timing_feel", timing_feel)?;
            r.field("register_st", register_st)?;
            r.field("glide_retrigger", glide_retrigger)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteRef {
    /// Display; outside keys.
    pub name: String,
    pub id: PaletteId,
}

/// The palette id only; the name is display.
impl KeyHash for PaletteRef {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { name: _, id } = self;
        id.write_key(w)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LensSettings {
    pub palette: PaletteRef,
    pub sliders: Sliders,
    /// Outside keys.
    pub locks: BTreeSet<Slider>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LensError {
    /// A slider value that is non-finite or outside its range.
    OutOfRange(Slider),
}

impl fmt::Display for LensError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(s) => write!(f, "slider {s:?} is outside its range"),
        }
    }
}

impl std::error::Error for LensError {}

impl LensSettings {
    /// Every slider finite and in its range (NaN is out of every range).
    pub fn validate(&self) -> Result<(), LensError> {
        let Sliders {
            mouth_amount,
            fidelity,
            mangle,
            timing_feel,
            register_st,
            glide_retrigger,
        } = self.sliders;
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        let checks = [
            (Slider::MouthAmount, unit(mouth_amount)),
            (Slider::Fidelity, unit(fidelity)),
            (Slider::Mangle, unit(mangle)),
            (Slider::TimingFeel, unit(timing_feel)),
            (
                Slider::Register,
                (-MAX_REGISTER_ST..=MAX_REGISTER_ST).contains(&register_st),
            ),
            (
                Slider::GlideRetrigger,
                (-1.0..=1.0).contains(&glide_retrigger),
            ),
        ];
        match checks.iter().find(|(_, ok)| !ok) {
            Some((s, _)) => Err(LensError::OutOfRange(*s)),
            None => Ok(()),
        }
    }
}

/// Palette id and all six sliders; the palette name and locks are outside keys.
impl KeyHash for LensSettings {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            palette,
            sliders,
            locks: _,
        } = self;
        w.record(|r| {
            r.field("palette", palette)?;
            r.field("sliders", sliders)
        })
    }
}
