//! Migrations on untyped JSON (core-types §2.14). Steps run before typed
//! decoding, so old Rust types never have to exist.
//!
//! A step from version `n` turns an `n` document into an `n + 1` document.
//! [`migrate`] writes `schema_version = n + 1` after each step, so a step only
//! moves fields. A new field arrives through a step that writes its frozen
//! reproducing value, never the new-project default.

use std::fmt;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::key::KeyError;

/// The field every versioned document carries.
pub const VERSION_FIELD: &str = "schema_version";

/// One migration step, from version `from` to `from + 1`.
pub type StepFn = fn(&mut serde_json::Value) -> Result<(), MigrationError>;

#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub from: u32,
    pub apply: StepFn,
}

/// The production chain is empty at schema 1.
pub const STEPS: &[Step] = &[];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationError {
    /// A missing version is an error, never 0.
    MissingVersion,
    TooNew {
        found: u32,
        supported: u32,
    },
    /// No step leaves version `at`.
    Gap {
        at: u32,
    },
    Step {
        from: u32,
        msg: String,
    },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVersion => f.write_str("document has no version"),
            Self::TooNew { found, supported } => {
                write!(f, "schema {found} is newer than this build's {supported}")
            }
            Self::Gap { at } => write!(f, "no migration step from schema {at}"),
            Self::Step { from, msg } => write!(f, "migration from schema {from}: {msg}"),
        }
    }
}

impl std::error::Error for MigrationError {}

/// A missing version is an error, never 0. So is a version that is not a
/// non-negative integer. A version beyond `u32` reads as `u32::MAX`, which is
/// newer than any build.
pub fn read_version(doc: &serde_json::Value) -> Result<u32, MigrationError> {
    read_version_field(doc, VERSION_FIELD)
}

/// [`read_version`] of another version field, such as a cache's
/// `container_version`.
pub fn read_version_field(doc: &serde_json::Value, field: &str) -> Result<u32, MigrationError> {
    let v = doc
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(MigrationError::MissingVersion)?;
    Ok(u32::try_from(v).unwrap_or(u32::MAX))
}

/// Migrates `doc` to `current`; `Some(old version)` when a step ran. A newer
/// document is `TooNew` and is left untouched; a version no step leaves is a
/// `Gap`. When several steps leave one version the first is used, but such a
/// chain is not [`chain_is_total`].
pub fn migrate(
    doc: &mut serde_json::Value,
    steps: &[Step],
    current: u32,
) -> Result<Option<u32>, MigrationError> {
    let found = read_version(doc)?;
    if found > current {
        return Err(MigrationError::TooNew {
            found,
            supported: current,
        });
    }
    let mut v = found;
    while v < current {
        let step = steps
            .iter()
            .find(|s| s.from == v)
            .ok_or(MigrationError::Gap { at: v })?;
        (step.apply)(doc)?;
        v += 1;
        match doc {
            Value::Object(map) => {
                map.insert(VERSION_FIELD.to_owned(), Value::from(v));
            }
            _ => {
                return Err(MigrationError::Step {
                    from: v - 1,
                    msg: "the step left a document that is not an object".to_owned(),
                });
            }
        }
    }
    Ok((found != current).then_some(found))
}

/// Whether `steps` lead from every version in `1..current` to `current`:
/// exactly one step leaves each of those versions, and none leaves 0 or a
/// version at or after `current`.
pub fn chain_is_total(steps: &[Step], current: u32) -> bool {
    let in_range = steps.iter().all(|s| (1..current).contains(&s.from));
    in_range && (1..current).all(|v| steps.iter().filter(|s| s.from == v).count() == 1)
}

/// Why a versioned document did not load.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentError {
    /// Malformed JSON or a typed-decode failure: an unknown field, an invalid
    /// value, a repeated key.
    Json(String),
    Migration(MigrationError),
    /// Decoded, but a set the document holds repeats an element.
    Key(KeyError),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(m) => write!(f, "json: {m}"),
            Self::Migration(e) => write!(f, "migration: {e}"),
            Self::Key(e) => write!(f, "key: {e}"),
        }
    }
}

impl std::error::Error for DocumentError {}

/// Reads the version (required) -> TooNew? -> migrates the untyped JSON
/// under `steps` -> typed decode (deny_unknown_fields, strict maps). Returns
/// the document and `Some(old version)` when a step ran.
///
/// The typed decode never reads floats out of a `serde_json::Value`, which
/// holds f64 (see `project::json`): an unmigrated document decodes from its
/// own bytes, and a migrated one from the migrated document printed back to
/// text, which reprints each untouched number as written. A `Value` keeps only
/// the last of two equal keys, so a migrated document's original text is
/// first checked for repeated keys ([`crate::strict::check_unique_keys`]).
pub fn load<T: DeserializeOwned>(
    bytes: &[u8],
    steps: &[Step],
    current: u32,
) -> Result<(T, Option<u32>), DocumentError> {
    let json = |e: serde_json::Error| DocumentError::Json(e.to_string());
    let mut doc: Value = serde_json::from_slice(bytes).map_err(json)?;
    let migrated_from = migrate(&mut doc, steps, current).map_err(DocumentError::Migration)?;
    let decoded = if migrated_from.is_some() {
        crate::strict::check_unique_keys(bytes).map_err(json)?;
        let text = serde_json::to_vec(&doc).map_err(json)?;
        serde_json::from_slice::<T>(&text).map_err(json)?
    } else {
        serde_json::from_slice::<T>(bytes).map_err(json)?
    };
    Ok((decoded, migrated_from))
}
