//! Canonical key encoding (core-types §2.2). Keys hash canonical bytes, never
//! JSON text.
//!
//! Rules every [`KeyHash`] impl follows:
//! - It destructures `Self` completely (`let Self { a, b, locks: _ } = self;`),
//!   so a new field fails to compile until it is classified as keyed or
//!   excluded.
//! - A field added after v0.1 is written with [`Record::unless`] and a frozen
//!   reproducing value, so old objects keep their keys.
//! - No f64 in keyed state: there is deliberately no `KeyHash` for f64.
//!
//! Encoding: every value starts with a one-byte tag. Integers of any width are
//! 16 bytes LE (so width never changes a key); f32 is its bits with -0.0 written
//! as +0.0; lengths are u64 LE; record fields are sorted by name; sets and maps
//! are sorted by their encodings; variants are identified by name.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::ids::{Hash32, context};

/// A value with a canonical key encoding.
pub trait KeyHash {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyError {
    /// NaN or an infinity in keyed state.
    NonFinite,
    /// A duplicate set element, map key or record field.
    DuplicateKey,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => f.write_str("non-finite float in keyed state"),
            Self::DuplicateKey => f.write_str("duplicate set element, map key or field"),
        }
    }
}

impl std::error::Error for KeyError {}

/// The kinds of key. Each hashes under its own blake3 `derive_key` context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyKind {
    Evidence,
    HitSet,
    TeachSet,
    Interp,
    Render,
}

impl KeyKind {
    /// The frozen blake3 `derive_key` context of this kind.
    pub const fn context(self) -> &'static str {
        match self {
            Self::Evidence => context::EVIDENCE,
            Self::HitSet => context::HIT_SET,
            Self::TeachSet => context::TEACH_SET,
            Self::Interp => context::INTERP,
            Self::Render => context::RENDER,
        }
    }
}

/// The key of `v` under `kind`'s context.
pub fn key_of<T: KeyHash + ?Sized>(kind: KeyKind, v: &T) -> Result<Hash32, KeyError> {
    let bytes = KeyWriter::encode(v)?;
    Ok(Hash32(blake3::derive_key(kind.context(), &bytes)))
}

const TAG_INT: u8 = 0x01;
const TAG_F32: u8 = 0x02;
const TAG_BOOL: u8 = 0x03;
const TAG_STR: u8 = 0x04;
const TAG_BYTES: u8 = 0x05;
const TAG_RECORD: u8 = 0x06;
const TAG_VARIANT: u8 = 0x07;
const TAG_LIST: u8 = 0x08;
const TAG_SET: u8 = 0x09;
const TAG_MAP: u8 = 0x0A;

/// Builds the canonical bytes of a key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyWriter {
    buf: Vec<u8>,
}

/// The fields of a record or variant, collected and then written sorted by name.
#[derive(Debug)]
pub struct Record<'a> {
    fields: &'a mut Vec<(&'static str, Vec<u8>)>,
}

impl KeyWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// The canonical bytes of `v` on their own.
    pub fn encode<T: KeyHash + ?Sized>(v: &T) -> Result<Vec<u8>, KeyError> {
        let mut w = Self::new();
        v.write_key(&mut w)?;
        Ok(w.buf)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    fn len_prefix(&mut self, n: usize) {
        self.buf.extend_from_slice(&(n as u64).to_le_bytes());
    }

    /// Tag 0x01, 16 bytes LE: every integer width encodes alike.
    pub fn int(&mut self, v: i128) {
        self.buf.push(TAG_INT);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Tag 0x02, the bits LE; -0.0 as +0.0; NaN or infinity is `NonFinite`.
    pub fn f32(&mut self, v: f32) -> Result<(), KeyError> {
        if !v.is_finite() {
            return Err(KeyError::NonFinite);
        }
        // Adding +0.0 maps -0.0 to +0.0 and leaves every other value alone.
        let v = v + 0.0;
        self.buf.push(TAG_F32);
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
        Ok(())
    }

    /// Tag 0x03.
    pub fn bool(&mut self, v: bool) {
        self.buf.push(TAG_BOOL);
        self.buf.push(u8::from(v));
    }

    /// Tag 0x04, u64 length, UTF-8.
    pub fn str(&mut self, s: &str) {
        self.buf.push(TAG_STR);
        self.len_prefix(s.len());
        self.buf.extend_from_slice(s.as_bytes());
    }

    /// Tag 0x05, u64 length, bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.push(TAG_BYTES);
        self.len_prefix(b.len());
        self.buf.extend_from_slice(b);
    }

    fn record_body(
        &mut self,
        f: impl FnOnce(&mut Record<'_>) -> Result<(), KeyError>,
    ) -> Result<(), KeyError> {
        let mut fields = Vec::new();
        f(&mut Record {
            fields: &mut fields,
        })?;
        fields.sort_unstable_by(|a, b| a.0.cmp(b.0));
        if fields.windows(2).any(|p| p[0].0 == p[1].0) {
            return Err(KeyError::DuplicateKey);
        }
        self.len_prefix(fields.len());
        for (name, bytes) in &fields {
            self.str(name);
            self.buf.extend_from_slice(bytes);
        }
        Ok(())
    }

    /// Tag 0x06: named fields, written sorted by name. A repeated name is
    /// `DuplicateKey`.
    pub fn record(
        &mut self,
        f: impl FnOnce(&mut Record<'_>) -> Result<(), KeyError>,
    ) -> Result<(), KeyError> {
        self.buf.push(TAG_RECORD);
        self.record_body(f)
    }

    /// Tag 0x07: an enum variant identified by name, with its fields as a record.
    pub fn variant(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Record<'_>) -> Result<(), KeyError>,
    ) -> Result<(), KeyError> {
        self.buf.push(TAG_VARIANT);
        self.str(name);
        self.record_body(f)
    }

    /// Tag 0x08: order kept.
    pub fn list<T: KeyHash>(&mut self, items: &[T]) -> Result<(), KeyError> {
        self.buf.push(TAG_LIST);
        self.len_prefix(items.len());
        for item in items {
            item.write_key(self)?;
        }
        Ok(())
    }

    /// Tag 0x09: sorted by encoding, so insertion order is irrelevant; a
    /// duplicate is `DuplicateKey`.
    pub fn set<'a, T: KeyHash + 'a>(
        &mut self,
        items: impl IntoIterator<Item = &'a T>,
    ) -> Result<(), KeyError> {
        let mut enc = items
            .into_iter()
            .map(|v| Self::encode(v))
            .collect::<Result<Vec<_>, _>>()?;
        enc.sort_unstable();
        if enc.windows(2).any(|p| p[0] == p[1]) {
            return Err(KeyError::DuplicateKey);
        }
        self.buf.push(TAG_SET);
        self.len_prefix(enc.len());
        for e in &enc {
            self.buf.extend_from_slice(e);
        }
        Ok(())
    }

    /// Tag 0x0A: entries sorted by encoded key; a duplicate key is `DuplicateKey`.
    pub fn map<'a, K: KeyHash + 'a, V: KeyHash + 'a>(
        &mut self,
        items: impl IntoIterator<Item = (&'a K, &'a V)>,
    ) -> Result<(), KeyError> {
        let mut enc = items
            .into_iter()
            .map(|(k, v)| Ok((Self::encode(k)?, Self::encode(v)?)))
            .collect::<Result<Vec<_>, KeyError>>()?;
        enc.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        if enc.windows(2).any(|p| p[0].0 == p[1].0) {
            return Err(KeyError::DuplicateKey);
        }
        self.buf.push(TAG_MAP);
        self.len_prefix(enc.len());
        for (k, v) in &enc {
            self.buf.extend_from_slice(k);
            self.buf.extend_from_slice(v);
        }
        Ok(())
    }
}

impl Record<'_> {
    pub fn field<T: KeyHash + ?Sized>(
        &mut self,
        name: &'static str,
        v: &T,
    ) -> Result<(), KeyError> {
        let bytes = KeyWriter::encode(v)?;
        self.fields.push((name, bytes));
        Ok(())
    }

    /// `None` is omitted.
    pub fn opt<T: KeyHash>(&mut self, name: &'static str, v: Option<&T>) -> Result<(), KeyError> {
        match v {
            Some(v) => self.field(name, v),
            None => Ok(()),
        }
    }

    /// Omitted at its frozen reproducing value, so objects from before the
    /// field existed keep their keys.
    pub fn unless<T: KeyHash + PartialEq>(
        &mut self,
        name: &'static str,
        v: &T,
        reproducing: &T,
    ) -> Result<(), KeyError> {
        if v == reproducing {
            Ok(())
        } else {
            self.field(name, v)
        }
    }
}

/// Keys a slice as a set: order is irrelevant and a duplicate is an error.
#[derive(Clone, Copy, Debug)]
pub struct SetOf<'a, T>(pub &'a [T]);

impl<T: KeyHash> KeyHash for SetOf<'_, T> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.set(self.0)
    }
}

macro_rules! key_int {
    ($($t:ty),*) => {$(
        impl KeyHash for $t {
            fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
                w.int(i128::from(*self));
                Ok(())
            }
        }
    )*};
}

key_int!(u8, u16, u32, u64, i8, i16, i32, i64, i128);

impl KeyHash for usize {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.int(*self as i128);
        Ok(())
    }
}

impl KeyHash for bool {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.bool(*self);
        Ok(())
    }
}

impl KeyHash for f32 {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.f32(*self)
    }
}

impl KeyHash for str {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.str(self);
        Ok(())
    }
}

impl KeyHash for String {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.str(self);
        Ok(())
    }
}

impl<T: KeyHash + ?Sized> KeyHash for &T {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        (**self).write_key(w)
    }
}

impl<T: KeyHash> KeyHash for [T] {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.list(self)
    }
}

impl<T: KeyHash> KeyHash for Vec<T> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.list(self)
    }
}

impl<T: KeyHash> KeyHash for BTreeSet<T> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.set(self)
    }
}

impl<K: KeyHash, V: KeyHash> KeyHash for BTreeMap<K, V> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        w.map(self)
    }
}

/// Raw framing for content hashes ("what was produced"): values exactly as
/// stored (-0.0 stays -0.0), integers at their own width, little-endian,
/// lengths as u64. Not canonical across widths or orderings, by design.
#[derive(Clone, Debug)]
pub struct RawHasher(blake3::Hasher);

impl RawHasher {
    /// Hashes under the blake3 `derive_key` context `context` (see
    /// [`crate::ids::context`]).
    pub fn new(context: &'static str) -> Self {
        Self(blake3::Hasher::new_derive_key(context))
    }

    pub fn u8(&mut self, v: u8) {
        self.0.update(&[v]);
    }

    pub fn u16(&mut self, v: u16) {
        self.0.update(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.0.update(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.0.update(&v.to_le_bytes());
    }

    pub fn i64(&mut self, v: i64) {
        self.0.update(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.0.update(&v.to_bits().to_le_bytes());
    }

    pub fn f64(&mut self, v: f64) {
        self.0.update(&v.to_bits().to_le_bytes());
    }

    /// A length or count, as u64.
    pub fn count(&mut self, n: usize) {
        self.u64(n as u64);
    }

    /// Length-prefixed bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.count(b.len());
        self.0.update(b);
    }

    /// Bytes with no length prefix, for fixed-size or self-delimiting data.
    pub fn raw(&mut self, b: &[u8]) {
        self.0.update(b);
    }

    /// Length-prefixed UTF-8.
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    pub fn f32s(&mut self, v: &[f32]) {
        self.count(v.len());
        for x in v {
            self.f32(*x);
        }
    }

    pub fn finish(&self) -> Hash32 {
        Hash32(*self.0.finalize().as_bytes())
    }
}
