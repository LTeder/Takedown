//! Typed parameters (core-types §2.11): no f32 is ever widened into an untyped
//! JSON value, and integers are never rounded through a float.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::key::{KeyError, KeyHash, KeyWriter};

/// One parameter value. `Int` comes before `Float`, so a JSON integer reads as
/// `Int`; a `Float` must be finite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
}

/// Named parameters, sorted by name.
pub type Params = BTreeMap<String, ParamValue>;

impl ParamValue {
    /// False only for a non-finite `Float`.
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Float(v) => v.is_finite(),
            Self::Bool(_) | Self::Int(_) | Self::Text(_) => true,
        }
    }
}

/// Variants are keyed by name, so `Int(1)` and `Float(1.0)` differ.
impl KeyHash for ParamValue {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        match self {
            Self::Bool(v) => w.variant("bool", |r| r.field("value", v)),
            Self::Int(v) => w.variant("int", |r| r.field("value", v)),
            Self::Float(v) => w.variant("float", |r| r.field("value", v)),
            Self::Text(v) => w.variant("text", |r| r.field("value", v)),
        }
    }
}
