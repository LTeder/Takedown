//! The project document (core-types §2.14). Persistence rules:
//! 1. One version per document: `project.json`, `controls/*.json` and
//!    `teach/*.json` each carry a `schema_version`; the `.tkev` and hit caches
//!    carry a container version, and a mismatch is a cache miss.
//! 2. Every persisted struct and enum is `deny_unknown_fields`. Nothing on disk
//!    uses `serde(default)` or `skip_serializing_if`.
//! 3. Every on-disk change bumps the version, additive changes included.
//! 4. A new field arrives through a migration step that writes its reproducing
//!    value into old files; names are never reused.
//! 5. Newer documents are refused (`TooNew`), never opened and truncated.
//! 6. Each version bump freezes a fixture of the old version.
//! 7. `save` is atomic and backs up a migrated original first.
//! 8. `project.json` holds no paths; [`paths`] derives them from ids.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::ids::{InterpId, LayerId, PaletteId, ProjectId, TakeId, TeachSetId};
use crate::interp::{ControlSource, InterpError, Interpretation};
use crate::key::KeyError;
use crate::layer::Layer;
use crate::migrate::{self, DocumentError, MigrationError, Step};
use crate::take::{HostAnchor, Take};
use crate::time::SampleRate;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteMeta {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub id: ProjectId,
    pub output_rate: SampleRate,
    #[serde(deserialize_with = "crate::strict::map")]
    pub takes: BTreeMap<TakeId, Take>,
    #[serde(deserialize_with = "crate::strict::map")]
    pub palettes: BTreeMap<PaletteId, PaletteMeta>,
    pub teach_set: Option<TeachSetId>,
    pub interpretations: Vec<Interpretation>,
    pub layers: Vec<Layer>,
    pub next_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeAdded {
    New,
    Existing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub project: Project,
    pub migrated_from: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProjectError {
    /// Malformed JSON or a typed-decode failure (including unknown fields).
    Json(String),
    Migration(MigrationError),
    UnknownTake(TakeId),
    UnknownPalette(PaletteId),
    UnknownInterp(InterpId),
    /// A local id used twice, or not below `next_id`.
    DuplicateId(u32),
    NonFinite(LayerId),
    /// A RenderRef whose key is not the key of its inputs, or that sits in
    /// the slot of another rate.
    RenderKey(LayerId),
    Interp(InterpId, InterpError),
    Key(KeyError),
    /// A take filed under another id, with no channels, or with a non-finite
    /// host anchor.
    InvalidTake(TakeId),
    /// Every local id has been allocated (`next_id == u32::MAX`).
    IdsExhausted,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(m) => write!(f, "project.json: {m}"),
            Self::Migration(e) => write!(f, "migration: {e}"),
            Self::UnknownTake(t) => write!(f, "unknown take {t}"),
            Self::UnknownPalette(p) => write!(f, "unknown palette {p}"),
            Self::UnknownInterp(i) => write!(f, "unknown interpretation {}", i.0),
            Self::DuplicateId(i) => write!(f, "local id {i} is reused or not allocated"),
            Self::NonFinite(l) => write!(f, "layer {} holds a non-finite value", l.0),
            Self::RenderKey(l) => write!(f, "layer {} has a render whose key lies", l.0),
            Self::Interp(i, e) => write!(f, "interpretation {}: {e}", i.0),
            Self::Key(e) => write!(f, "key: {e}"),
            Self::InvalidTake(t) => write!(f, "take {t} is malformed"),
            Self::IdsExhausted => f.write_str("every local id has been allocated"),
        }
    }
}

impl std::error::Error for ProjectError {}

impl Project {
    pub fn new(id: ProjectId, output_rate: SampleRate) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id,
            output_rate,
            takes: BTreeMap::new(),
            palettes: BTreeMap::new(),
            teach_set: None,
            interpretations: Vec::new(),
            layers: Vec::new(),
            next_id: 0,
        }
    }

    /// The next local id. Ids are never reused, so an id in the undo history
    /// cannot change meaning. `IdsExhausted` once `next_id` is `u32::MAX`: a
    /// valid project may hold that value, and then cannot grow, but never
    /// panics.
    pub fn alloc_id(&mut self) -> Result<u32, ProjectError> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(ProjectError::IdsExhausted)?;
        Ok(id)
    }

    /// Identical PCM is one take: an existing record is kept as it is.
    pub fn add_take(&mut self, take: Take) -> TakeAdded {
        match self.takes.entry(take.id) {
            std::collections::btree_map::Entry::Occupied(_) => TakeAdded::Existing,
            std::collections::btree_map::Entry::Vacant(v) => {
                v.insert(take);
                TakeAdded::New
            }
        }
    }

    /// Referential integrity (takes, palettes, interpretations), unique ids,
    /// finite values, every `RenderRef.key == key(inputs)`, every
    /// Interpretation valid.
    ///
    /// The schema version is not checked here: [`from_json`](Self::from_json)
    /// has already migrated the document to the version it decodes.
    pub fn validate(&self) -> Result<(), ProjectError> {
        let Self {
            schema_version: _,
            id: _,
            output_rate: _,
            takes,
            palettes,
            teach_set: _,
            interpretations,
            layers,
            next_id,
        } = self;
        for (id, take) in takes {
            let finite = take.anchor.as_ref().is_none_or(HostAnchor::is_finite);
            if take.id != *id || take.audio.channels == 0 || !finite {
                return Err(ProjectError::InvalidTake(*id));
            }
        }
        // Interpretation, layer and edit ids come from one counter.
        let mut used = BTreeSet::new();
        let mut claim = |id: u32| {
            if id < *next_id && used.insert(id) {
                Ok(())
            } else {
                Err(ProjectError::DuplicateId(id))
            }
        };
        for interp in interpretations {
            claim(interp.id.0)?;
            if let ControlSource::Lens { edits, .. } = &interp.source {
                for e in edits {
                    claim(e.id.0)?;
                }
            }
            if let Some(t) = interp.take
                && !takes.contains_key(&t)
            {
                return Err(ProjectError::UnknownTake(t));
            }
            let palette = interp.lens.palette.id;
            if !palettes.contains_key(&palette) {
                return Err(ProjectError::UnknownPalette(palette));
            }
            interp
                .validate(takes)
                .map_err(|e| ProjectError::Interp(interp.id, e))?;
        }
        for layer in layers {
            claim(layer.id.0)?;
            if !interpretations.iter().any(|i| i.id == layer.interp) {
                return Err(ProjectError::UnknownInterp(layer.interp));
            }
            if !layer.gain_db.is_finite() {
                return Err(ProjectError::NonFinite(layer.id));
            }
            for (rate, render) in &layer.renders {
                let key = render.inputs.key().map_err(ProjectError::Key)?;
                if render.inputs.rate != *rate || key != render.key {
                    return Err(ProjectError::RenderKey(layer.id));
                }
            }
        }
        Ok(())
    }

    /// Deterministic: the same project always gives the same bytes. Pretty
    /// printed with a trailing newline, so a project diffs well; struct fields
    /// in declaration order and every map sorted. Written by [`json`], so every
    /// f32 reads back exactly.
    pub fn to_json(&self) -> Vec<u8> {
        // Every map key is a hex id, a rate or an aspect name, so this never fails.
        let mut out = json::to_vec_pretty(self).expect("a project serialises");
        out.push(b'\n');
        out
    }

    /// Reads the version (required) -> TooNew? -> migrates the untyped JSON ->
    /// typed decode (deny_unknown_fields) -> validate.
    pub fn from_json(bytes: &[u8]) -> Result<Loaded, ProjectError> {
        Self::from_json_with(bytes, migrate::STEPS, SCHEMA_VERSION)
    }

    /// [`from_json`](Self::from_json) under an explicit migration chain to
    /// `current`, for testing chains that production does not have yet.
    ///
    /// The decode is [`migrate::load`]'s: it never reads floats out of a
    /// `serde_json::Value`, which holds f64 (see [`json`]), and refuses a
    /// repeated key on either path.
    pub fn from_json_with(
        bytes: &[u8],
        steps: &[Step],
        current: u32,
    ) -> Result<Loaded, ProjectError> {
        let (project, migrated_from) =
            migrate::load::<Project>(bytes, steps, current).map_err(|e| match e {
                DocumentError::Json(m) => ProjectError::Json(m),
                DocumentError::Migration(e) => ProjectError::Migration(e),
                DocumentError::Key(e) => ProjectError::Key(e),
            })?;
        project.validate()?;
        Ok(Loaded {
            project,
            migrated_from,
        })
    }

    /// Writes a temp file in `dir`, fsyncs it and renames it over
    /// `project.json`. Before the first save over a migrated file it copies the
    /// original to `project.v{N}.json.bak` (the same way); a backup that already
    /// exists holds the true original and is never overwritten. An invalid
    /// project is refused with `InvalidData` and nothing is written.
    pub fn save(&self, dir: &Path, migrated_from: Option<u32>) -> io::Result<()> {
        self.validate()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let target = dir.join(paths::PROJECT);
        if let Some(v) = migrated_from {
            let backup = dir.join(paths::backup(v));
            if !backup.try_exists()? {
                match fs::read(&target) {
                    Ok(original) => write_atomic(dir, &backup, &original)?,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
        }
        write_atomic(dir, &target, &self.to_json())
    }
}

/// Writes `bytes` to a fresh temp file in `dir`, fsyncs it and renames it over
/// `target`. The temp file never outlives a failure.
fn write_atomic(dir: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = target
        .file_name()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let tmp = dir.join(format!(".{name}.tmp-{}-{n}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, target)?;
        sync_dir(dir);
        Ok(())
    })();
    if result.is_err() {
        // Best effort: the original error is the one worth reporting.
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Makes the rename durable. Best effort: some filesystems refuse to fsync a
/// directory, and the data itself is already synced.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

/// Windows has no directory handle to fsync; the rename is already durable
/// once `MoveFileEx` returns.
#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

/// JSON text for persisted documents: serde_json's, except for the f32s whose
/// shortest decimal would round twice when read through an f64.
///
/// serde_json writes an f32 as its shortest decimal, which a typed f32 field
/// reads back exactly. But serde buffers numbers as f64 inside internally
/// tagged and untagged enums (an `Edit`, a `ControlSource`, a `ParamValue`),
/// and `serde_json::Value` holds f64. For two patterns, ±7.038531e-26, that
/// decimal parses to the f64 exactly midway between two f32s, and the second
/// rounding lands on the neighbour, changing the value and any key over it.
/// For those this writer prints the f64 decimal of the exact value instead,
/// which every path reads back exactly. Every other byte is serde_json's.
pub mod json {
    use std::io;

    use serde::Serialize;
    use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter, Serializer};

    /// Compact, like `serde_json::to_vec`.
    pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<Vec<u8>> {
        write(value, CompactFormatter)
    }

    /// Two-space indented, like `serde_json::to_vec_pretty`.
    pub fn to_vec_pretty<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<Vec<u8>> {
        write(value, PrettyFormatter::new())
    }

    /// Whether serde_json's own text for `x` reads back as another f32 when it
    /// is parsed as an f64 first.
    pub fn double_rounds(x: f32) -> bool {
        if !x.is_finite() {
            return false;
        }
        serde_json::to_string(&x)
            .ok()
            .and_then(|text| text.parse::<f64>().ok())
            .is_some_and(|d| (d as f32).to_bits() != x.to_bits())
    }

    fn write<T: Serialize + ?Sized, F: Formatter>(
        value: &T,
        formatter: F,
    ) -> serde_json::Result<Vec<u8>> {
        let mut out = Vec::new();
        value.serialize(&mut Serializer::with_formatter(
            &mut out,
            ExactF32(formatter),
        ))?;
        Ok(out)
    }

    /// `F`, with f32s written so that they survive an f64 on the way back.
    struct ExactF32<F>(F);

    impl<F: Formatter> Formatter for ExactF32<F> {
        fn write_f32<W: ?Sized + io::Write>(&mut self, w: &mut W, value: f32) -> io::Result<()> {
            if double_rounds(value) {
                // The f64 decimal of the exact value: as an f64 it is the
                // value itself, and as an f32 it is deep inside its interval.
                self.0.write_f64(w, f64::from(value))
            } else {
                self.0.write_f32(w, value)
            }
        }

        fn begin_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.begin_array(w)
        }

        fn end_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_array(w)
        }

        fn begin_array_value<W: ?Sized + io::Write>(
            &mut self,
            w: &mut W,
            first: bool,
        ) -> io::Result<()> {
            self.0.begin_array_value(w, first)
        }

        fn end_array_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_array_value(w)
        }

        fn begin_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.begin_object(w)
        }

        fn end_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_object(w)
        }

        fn begin_object_key<W: ?Sized + io::Write>(
            &mut self,
            w: &mut W,
            first: bool,
        ) -> io::Result<()> {
            self.0.begin_object_key(w, first)
        }

        fn end_object_key<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_object_key(w)
        }

        fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.begin_object_value(w)
        }

        fn end_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_object_value(w)
        }
    }
}

/// Paths inside a project directory, derived from ids. Durable: `project.json`,
/// `audio/`, `renders/`, `controls/`, `midi/`, `palettes/`, `teach/`. Deletable
/// at any time: everything under `cache/`.
pub mod paths {
    use std::path::PathBuf;

    use crate::ids::{
        ControlId, EvidenceKey, HitSetKey, MidiFileId, PaletteId, TakeId, TeachSetId, WavId,
    };

    pub const PROJECT: &str = "project.json";
    pub const CACHE: &str = "cache";

    /// `project.v{N}.json.bak`: the original of a file migrated from schema N.
    pub fn backup(version: u32) -> PathBuf {
        PathBuf::from(format!("project.v{version}.json.bak"))
    }

    pub fn audio(id: TakeId) -> PathBuf {
        PathBuf::from(format!("audio/{id}.wav"))
    }

    pub fn render(id: WavId) -> PathBuf {
        PathBuf::from(format!("renders/{id}.wav"))
    }

    pub fn control(id: ControlId) -> PathBuf {
        PathBuf::from(format!("controls/{id}.json"))
    }

    pub fn midi(id: MidiFileId) -> PathBuf {
        PathBuf::from(format!("midi/{id}.mid"))
    }

    pub fn palette(id: PaletteId) -> PathBuf {
        PathBuf::from(format!("palettes/{id}.json"))
    }

    pub fn teach(id: TeachSetId) -> PathBuf {
        PathBuf::from(format!("teach/{id}.json"))
    }

    pub fn evidence_cache(key: EvidenceKey) -> PathBuf {
        PathBuf::from(format!("cache/evidence/{key}.tkev"))
    }

    pub fn hits_cache(key: HitSetKey) -> PathBuf {
        PathBuf::from(format!("cache/hits/{key}.json"))
    }

    pub fn peaks(id: TakeId) -> PathBuf {
        PathBuf::from(format!("cache/peaks/{id}.pk"))
    }
}
