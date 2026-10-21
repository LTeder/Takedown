//! Strict decoding of persisted maps and sets (core-types §2.14, rule 2).
//!
//! serde's own `BTreeMap` keeps the last of two equal keys and its `BTreeSet`
//! drops a repeated element, so `{"body":1,"body":2}` would load as
//! `{body: 2}` and a hand-edited or corrupted file would change meaning
//! silently. Every persisted map field uses [`map`] and every persisted set
//! field [`set`], which refuse the repeat instead:
//!
//! ```ignore
//! #[serde(deserialize_with = "crate::strict::map")]
//! pub rolls: BTreeMap<Aspect, u32>,
//! ```
//!
//! Keys compare after decoding, so `"a"` and `"a"` are the same key.
//! Struct fields need nothing: serde's derive already refuses a repeated
//! field. [`check_unique_keys`] covers the one path where typed decoding
//! cannot see the original text: a migrated document, which passes through a
//! `serde_json::Value` (whose objects keep only the last of equal keys).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A map whose keys each appear once.
pub fn map<'de, D, K, V>(d: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    d.deserialize_map(MapVisitor(PhantomData))
}

/// A set, written as an array, whose elements each appear once.
pub fn set<'de, D, T>(d: D) -> Result<BTreeSet<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Ord + fmt::Debug,
{
    d.deserialize_seq(SetVisitor(PhantomData))
}

struct MapVisitor<K, V>(PhantomData<fn() -> (K, V)>);

impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a map with unique keys")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut out = BTreeMap::new();
        while let Some(k) = access.next_key::<K>()? {
            if out.contains_key(&k) {
                return Err(de::Error::custom(format_args!("duplicate map key {k:?}")));
            }
            let v = access.next_value::<V>()?;
            out.insert(k, v);
        }
        Ok(out)
    }
}

struct SetVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for SetVisitor<T>
where
    T: Deserialize<'de> + Ord + fmt::Debug,
{
    type Value = BTreeSet<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of unique elements")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = BTreeSet::new();
        while let Some(x) = seq.next_element::<T>()? {
            if out.contains(&x) {
                return Err(de::Error::custom(format_args!(
                    "duplicate set element {x:?}"
                )));
            }
            out.insert(x);
        }
        Ok(out)
    }
}

/// Refuses a JSON document in which any object, at any depth, names a key
/// twice. Run before a document is read into a `serde_json::Value`.
pub fn check_unique_keys(bytes: &[u8]) -> serde_json::Result<()> {
    serde_json::from_slice::<UniqueKeys>(bytes).map(|_| ())
}

/// Any JSON value whose objects have unique keys; holds nothing.
struct UniqueKeys;

impl<'de> Deserialize<'de> for UniqueKeys {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(UniqueKeysVisitor)
    }
}

struct UniqueKeysVisitor;

impl<'de> Visitor<'de> for UniqueKeysVisitor {
    type Value = UniqueKeys;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<UniqueKeys, E> {
        Ok(UniqueKeys)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<UniqueKeys, E> {
        Ok(UniqueKeys)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<UniqueKeys, E> {
        Ok(UniqueKeys)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<UniqueKeys, E> {
        Ok(UniqueKeys)
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<UniqueKeys, E> {
        Ok(UniqueKeys)
    }

    fn visit_unit<E: de::Error>(self) -> Result<UniqueKeys, E> {
        Ok(UniqueKeys)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<UniqueKeys, A::Error> {
        while seq.next_element::<UniqueKeys>()?.is_some() {}
        Ok(UniqueKeys)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<UniqueKeys, A::Error> {
        let mut seen = BTreeSet::new();
        while let Some(k) = access.next_key::<String>()? {
            if seen.contains(&k) {
                return Err(de::Error::custom(format_args!(
                    "duplicate object key {k:?}"
                )));
            }
            seen.insert(k);
            access.next_value::<UniqueKeys>()?;
        }
        Ok(UniqueKeys)
    }
}
