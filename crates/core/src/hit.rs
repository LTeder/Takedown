//! Hits, hit sets and teach labels (core-types §2.6). Roles persist by name,
//! never by index; teach labels are times, never feature vectors.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{EvidenceKey, Hash32, HitSetKey, TakeId, TeachSetId, context};
use crate::key::{KeyError, KeyHash, KeyKind, KeyWriter, RawHasher, SetOf, key_of};
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

/// `cache/hits/<key>.json` as written: the hit set under its container version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitCacheFile {
    pub container_version: u32,
    pub set: HitSet,
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
