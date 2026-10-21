//! Migrations on untyped JSON (core-types §2.14). Steps run before typed
//! decoding, so old Rust types never have to exist.
//!
//! A step from version `n` turns an `n` document into an `n + 1` document.
//! [`migrate`] writes `schema_version = n + 1` after each step, so a step only
//! moves fields. A new field arrives through a step that writes its frozen
//! reproducing value, never the new-project default.

use std::fmt;

use serde_json::Value;

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
            Self::MissingVersion => f.write_str("document has no schema_version"),
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
    let v = doc
        .get(VERSION_FIELD)
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
