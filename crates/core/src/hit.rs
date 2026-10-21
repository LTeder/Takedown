//! Hits, hit sets and teach labels (core-types §2.6). Roles persist by name,
//! never by index; teach labels are times, never feature vectors.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{EvidenceKey, Hash32, HitSetKey, TakeId, TeachSetId, context};
use crate::key::{KeyError, KeyHash, KeyKind, KeyWriter, RawHasher, SetOf, key_of};
use crate::migrate::{self, DocumentError, Step};
use crate::params::Params;
use crate::time::{TimelinePos, TimelineSpan};

/// The longest role name.
pub const MAX_ROLE_NAME_LEN: usize = 32;

/// `[a-z0-9_]{1,32}`; app turns user names into this form.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoleName(String);

/// A string that is not a valid role name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleNameError(pub String);

impl fmt::Display for RoleNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a role name ([a-z0-9_]{{1,32}})", self.0)
    }
}

impl std::error::Error for RoleNameError {}

impl RoleName {
    pub fn new(s: &str) -> Result<Self, RoleNameError> {
        let ok = (1..=MAX_ROLE_NAME_LEN).contains(&s.len())
            && s.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_');
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(RoleNameError(s.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RoleName {
    type Error = RoleNameError;

    fn try_from(s: String) -> Result<Self, RoleNameError> {
        Self::new(&s)
    }
}

impl From<RoleName> for String {
    fn from(r: RoleName) -> String {
        r.0
    }
}

impl fmt::Display for RoleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RoleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RoleName({})", self.0)
    }
}

impl KeyHash for RoleName {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.str(&self.0);
        Ok(())
    }
}

/// Which classifier produced a hit set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassifierStamp {
    pub id: String,
    pub version: u32,
    #[serde(deserialize_with = "crate::strict::map")]
    pub params: Params,
}

impl KeyHash for ClassifierStamp {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            id,
            version,
            params,
        } = self;
        w.record(|r| {
            r.field("id", id)?;
            r.field("version", version)?;
            r.field("params", params)
        })
    }
}

/// drums output: observations plus soft class scores. The plan's exciter
/// slice ref is not stored; the slice derives from the onset (§2.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hit {
    /// Refined onset: the hit's identity and the anchor of everything derived from it.
    pub onset: TimelinePos,
    pub span: TimelineSpan,
    pub loudness_db: f32,
    pub brightness: f32,
    pub decay_db_per_s: f32,
    /// Aligned with `HitSet::roles`.
    pub scores: Vec<f32>,
}

/// `cache/hits/<key>.json`, inside a [`HitCacheFile`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitSet {
    pub key: HitSetKey,
    pub roles: Vec<RoleName>,
    pub hits: Vec<Hit>,
}

impl HitSet {
    /// Raw framing of every field: what was produced.
    pub fn content_hash(&self) -> Hash32 {
        let Self { key, roles, hits } = self;
        let mut h = RawHasher::new(context::HIT_SET_CONTENT);
        h.raw(&key.hash32().0);
        h.count(roles.len());
        for r in roles {
            h.str(r.as_str());
        }
        h.count(hits.len());
        for hit in hits {
            let Hit {
                onset,
                span,
                loudness_db,
                brightness,
                decay_db_per_s,
                scores,
            } = hit;
            h.u64(onset.0);
            h.u64(span.start().0);
            h.u64(span.end().0);
            h.f32(*loudness_db);
            h.f32(*brightness);
            h.f32(*decay_db_per_s);
            h.f32s(scores);
        }
        h.finish()
    }
}

/// The container version of the hit cache. A mismatch is a cache miss.
pub const HIT_CACHE_VERSION: u32 = 1;

/// The version field of a hit cache file.
pub const HIT_CACHE_VERSION_FIELD: &str = "container_version";

/// `cache/hits/<key>.json` as written: the hit set under its container version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitCacheFile {
    pub container_version: u32,
    pub set: HitSet,
}

/// Why a hit cache file was not used. Every one is a cache miss: recompute
/// the set and write it again.
#[derive(Clone, Debug, PartialEq)]
pub enum HitCacheError {
    /// Malformed JSON or a typed-decode failure.
    Json(String),
    /// No container version: never read as 0.
    MissingVersion,
    /// Written under another container version, older or newer; never
    /// decoded as this build's.
    VersionMismatch { found: u32, supported: u32 },
    /// The file holds the set of another key.
    KeyMismatch,
}

impl fmt::Display for HitCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(m) => write!(f, "hit cache: {m}"),
            Self::MissingVersion => f.write_str("hit cache has no container version"),
            Self::VersionMismatch { found, supported } => write!(
                f,
                "hit cache container version {found}, this build reads {supported}"
            ),
            Self::KeyMismatch => f.write_str("hit cache holds another key's set"),
        }
    }
}

impl std::error::Error for HitCacheError {}

impl HitCacheFile {
    /// `set` under the current container version.
    pub fn new(set: HitSet) -> Self {
        Self {
            container_version: HIT_CACHE_VERSION,
            set,
        }
    }

    /// Compact and deterministic, written by [`crate::project::json`] so
    /// every f32 reads back exactly.
    pub fn to_json(&self) -> Vec<u8> {
        // A hit set holds a key, role names, integers and f32s, and no map.
        crate::project::json::to_vec(self).expect("a hit cache file serialises")
    }

    /// The set stored for `expected`. Reads the container version first
    /// (required); any other version is [`HitCacheError::VersionMismatch`],
    /// a cache miss, and the file is not decoded. A file holding another
    /// key's set is never a silent substitute.
    pub fn from_json(bytes: &[u8], expected: HitSetKey) -> Result<HitSet, HitCacheError> {
        let json = |e: serde_json::Error| HitCacheError::Json(e.to_string());
        let doc: serde_json::Value = serde_json::from_slice(bytes).map_err(json)?;
        let found = migrate::read_version_field(&doc, HIT_CACHE_VERSION_FIELD)
            .map_err(|_| HitCacheError::MissingVersion)?;
        if found != HIT_CACHE_VERSION {
            return Err(HitCacheError::VersionMismatch {
                found,
                supported: HIT_CACHE_VERSION,
            });
        }
        let file: HitCacheFile = serde_json::from_slice(bytes).map_err(json)?;
        if file.set.key == expected {
            Ok(file.set)
        } else {
            Err(HitCacheError::KeyMismatch)
        }
    }
}

/// The key of (evidence key, classifier stamp, teach set).
pub fn hit_set_key(
    evidence: EvidenceKey,
    classifier: &ClassifierStamp,
    teach: Option<TeachSetId>,
) -> Result<HitSetKey, KeyError> {
    struct Input<'a> {
        evidence: EvidenceKey,
        classifier: &'a ClassifierStamp,
        teach: Option<TeachSetId>,
    }
    impl KeyHash for Input<'_> {
        fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
            let Self {
                evidence,
                classifier,
                teach,
            } = self;
            w.record(|r| {
                r.field("evidence", evidence)?;
                r.field("classifier", *classifier)?;
                r.opt("teach", teach.as_ref())
            })
        }
    }
    key_of(
        KeyKind::HitSet,
        &Input {
            evidence,
            classifier,
            teach,
        },
    )
    .map(HitSetKey::from_hash)
}

/// A teach label is a time, not a feature vector, so a feature-extractor
/// change keeps every label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeachLabel {
    pub take: TakeId,
    pub at: TimelinePos,
    pub role: RoleName,
}

impl KeyHash for TeachLabel {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { take, at, role } = self;
        w.record(|r| {
            r.field("take", take)?;
            r.field("at", at)?;
            r.field("role", role)
        })
    }
}

/// `teach/<id>.json` (inside a [`TeachFile`]), durable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeachSet {
    pub labels: Vec<TeachLabel>,
}

impl TeachSet {
    /// The key of the labels as a set: order is irrelevant, and a duplicate
    /// label is `DuplicateKey`.
    pub fn id(&self) -> Result<TeachSetId, KeyError> {
        key_of(KeyKind::TeachSet, self).map(TeachSetId::from_hash)
    }
}

impl KeyHash for TeachSet {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { labels } = self;
        w.record(|r| r.field("labels", &SetOf(labels)))
    }
}

/// The schema version of `teach/*.json`.
pub const TEACH_SCHEMA_VERSION: u32 = 1;

/// `teach/<id>.json` as a versioned document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeachFile {
    pub schema_version: u32,
    pub set: TeachSet,
}

/// The migration chain of `teach/*.json`: empty at schema 1.
pub const TEACH_STEPS: &[Step] = &[];

impl TeachFile {
    /// `set` as a document of the current schema.
    pub fn new(set: TeachSet) -> Self {
        Self {
            schema_version: TEACH_SCHEMA_VERSION,
            set,
        }
    }

    /// Compact and deterministic, written by [`crate::project::json`].
    pub fn to_json(&self) -> Vec<u8> {
        // Ids, positions and role names only.
        crate::project::json::to_vec(self).expect("a teach file serialises")
    }

    /// Reads the version (required) -> TooNew? -> typed decode. The labels
    /// are a set: a repeated label is `Key(DuplicateKey)`. No migration step
    /// exists yet, so an older version is a `Gap`.
    pub fn from_json(bytes: &[u8]) -> Result<Self, DocumentError> {
        let (file, _) = migrate::load::<Self>(bytes, TEACH_STEPS, TEACH_SCHEMA_VERSION)?;
        file.set.id().map_err(DocumentError::Key)?;
        Ok(file)
    }
}

/// Selects the nearest anchor within `tol` samples (timeline rate); a tie
/// goes to the earlier one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitAnchor {
    pub at: TimelinePos,
    pub tol: u32,
}

impl HitAnchor {
    /// The index of the nearest anchor within `tol`. A tie goes to the earlier
    /// position, then to the lower index. `anchors` need not be sorted.
    pub fn resolve(&self, anchors: &[TimelinePos]) -> Option<usize> {
        let mut best: Option<(u64, TimelinePos, usize)> = None;
        for (i, a) in anchors.iter().enumerate() {
            let d = a.0.abs_diff(self.at.0);
            if d > u64::from(self.tol) {
                continue;
            }
            let cand = (d, *a, i);
            if best.is_none_or(|b| cand < b) {
                best = Some(cand);
            }
        }
        best.map(|(_, _, i)| i)
    }
}

impl KeyHash for HitAnchor {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self { at, tol } = self;
        w.record(|r| {
            r.field("at", at)?;
            r.field("tol", tol)
        })
    }
}
