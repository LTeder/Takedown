# Core types: v0.1 design

*2026-10-21. Output of the step-3 design panel: three attempts at MASTER-PLAN §4.1 under §3 (renderer lens A, round-trip lens B, persistence lens C), three adversarial judges, one synthesis. The implementer follows this document in one pass. MASTER-PLAN wins on conflict, except for the deviations listed in §7, each of which also goes in DECISIONS.md. The attempts and judge reports are not kept in the repository.*

---

## 1. Summary

**Winner: attempt A.** No attempt had a fatal break. Counting distinct major breaks across the three judges, A had 6, B 7 and C 8. A had none in its render contract (driver, segment plan, latency, allocation), which is the part most expensive to retrofit and the one every CLAUDE.md render property rests on. A's six majors close with contained grafts:
- B's gate spans replace A's gate curve, which also simplifies A's activity computation.
- B's `quantise` projection and edit application are added as modules.
- Per-trigger seeds become anchor-local.
- C's persistence rules replace A's.

B's render majors would have needed its backend trait replaced wholesale by A's driver. C needed both other designs.

The result takes A's time base, render contract and seed tree, B's ControlTrack shape, MIDI projection and edit semantics, and C's keys, evidence cache, migrations and render slots. A's live-playback machinery (seek, track swap, latency reporting) is deferred to the plugin. The render contract lives in `takedown-render::contract`, where MASTER-PLAN §4.3 puts it.

| Attempt | Fatal | Major: J1 render / J2 round trip / J3 persistence | Distinct majors | Cost to close |
|---|---|---|---|---|
| A | 0 | 0 / 4 / 2 | 6 | three module grafts from B, one seed-derivation change, two persistence rules |
| B | 0 | 3 / 4 / 1 | 7 | replace its backend trait and driver with A's; thread an anchor through track and codec; four small fixes |
| C | 0 | 3 / 5 / 2 | 8 | needs A's driver and B's track shape and edits, i.e. both other designs |

---

## 2. Types

### 2.0 Placement and dependencies

| Crate::module | Holds |
|---|---|
| `takedown-core::{time, ids, key, seed, params}` | time base, identities, canonical key encoding, seed tree and noise, typed parameters |
| `takedown-core::{take, evidence, hit}` | Take and its hasher, Evidence and the `.tkev` container, HitSet, teach labels |
| `takedown-core::{control, views, musical, canonical}` | ControlTrack, NoteView and HitView, TimeMap, the MPE lattice and `quantise` |
| `takedown-core::{edit, lens, interp, layer, project, migrate}` | edits and their application, LensSettings, Interpretation, Layer and render keys, Project, migrations |
| `takedown-render::contract` | `RenderBackend`, `BoundTrack`, segment plan, driver, `render_offline`, `testkit` (cargo feature) |
| `takedown-midi` | SMF bytes and the MPE codec (`encode`, `decode`); depends on core only |

`render::contract` imports nothing from the rest of `render`. It can therefore become its own crate when `infer` or `plugin` needs the trait without Plaits.

Dependencies, added to `[workspace.dependencies]`:

| Crate | Features | Used by | Why |
|---|---|---|---|
| serde 1 | derive | core | every persisted type |
| serde_json 1 | **float_roundtrip** | core | `project.json`, container headers and imported tracks. Without the feature about 13% of f64 values in [0, 300) do not survive text; with it none fail (judge 3). Declared once for the workspace so feature unification cannot lose it |
| blake3 1 | | core | take ids, keys, content hashes, seed derivation; already in the plan's stack |
| proptest 1 | | dev, core and render | property tests |

No uuid, rand or chrono. Ids are project counters, seed roots and project ids come from `app`, and timestamps are strings. `core` is `#![forbid(unsafe_code)]`. The only `unsafe` is testkit's counting allocator, with a comment.

### 2.1 `core::time`

```rust
pub const CONTROL_HZ: u32 = 200;         // frame k sits at k/200 s, independent of any rate; never changes
pub const MIN_GAP_FRAMES: u64 = 10;      // 50 ms: the reset gap of principle 8
pub const MIN_RATE_HZ: u32 = 1_000;      // >= 5 samples per frame: the clock carries at most once per sample
pub const MAX_RATE_HZ: u32 = 1_536_000;  // < 2^24: rem and rate are exact in f32

/// Validated to [MIN_RATE_HZ, MAX_RATE_HZ] on construction and on load.
#[serde(try_from = "u32", into = "u32")]
pub struct SampleRate(u32);
impl SampleRate {
    pub const fn new(hz: u32) -> Option<Self>;
    pub const fn hz(self) -> u32;
    pub fn samples_for_ms(self, ms: u32) -> u64;   // round half up
}

/// At the timeline rate: the take's capture rate, or the declared rate of a track
/// with no take. Used by Evidence, hits, edits, ControlTrack and seeds.
#[serde(transparent)] pub struct TimelinePos(pub u64);
/// At a backend's output rate. Used only by render::contract and backends.
#[serde(transparent)] pub struct RenderPos(pub u64);
/// Half-open [start, end) with start <= end. Serialised as [start, end].
pub struct TimelineSpan { start: TimelinePos, end: TimelinePos }

// All arithmetic is u128 and saturates at u64::MAX; nothing panics.
pub fn control_index(pos: u64, rate: SampleRate) -> (u64, u32);    // (frame, rem): frame*rate + rem == pos*200, rem < rate
pub fn frame_start(frame: u64, rate: SampleRate) -> u64;           // ceil(frame*rate/200): first sample of the frame
pub fn frames_covering(samples: u64, rate: SampleRate) -> u64;     // ceil(samples*200/rate)
pub fn rescale_round(pos: u64, from: SampleRate, to: SampleRate) -> u64;        // half up; up then down is the identity
pub fn rescale_ceil(pos: u64, from: SampleRate, to: SampleRate) -> (u64, f32);  // first sample at or after, plus lead in [0, 1)
pub fn seconds(pos: u64, rate: SampleRate) -> f64;                 // display and logs only; never fed back

/// Exact incremental clock: the state after n advance() calls from p equals at(p + n).
pub struct ControlClock { rate: u32, frame: u64, rem: u32 }
impl ControlClock {
    pub fn at(rate: SampleRate, pos: u64) -> Self;
    pub fn advance(&mut self);     // rem += 200; if rem >= rate { rem -= rate; frame += 1 }
    pub fn frame(&self) -> u64;
    pub fn frac(&self) -> f32;     // rem as f32 / rate as f32: one correctly rounded division
}
```

- Interpolation is `a + (b - a) * frac` in f32; Rust never contracts this into an FMA. It is linear in semitones, in dB and on the unit range. Past the last frame the last value holds.
- At 44.1 kHz frames alternate between 220 and 221 samples.
- f64 and seconds appear only in display code and in `HostAnchor`.

### 2.2 `core::ids` and `core::key`

```rust
/// A blake3 digest, serialised as 64 lowercase hex characters.
pub struct Hash32(pub [u8; 32]);

// Typed newtypes over Hash32 that cannot be swapped for one another. Each kind
// hashes under its own blake3 derive_key context "takedown 2026-10 <kind> v1".
pub struct TakeId(Hash32);      // TakeHasher over framed PCM (2.4)
pub struct EvidenceKey(Hash32); // key of (take, Provenance): what was asked for
pub struct HitSetKey(Hash32);   // key of (evidence key, classifier stamp, teach set)
pub struct TeachSetId(Hash32);  // key of the teach labels
pub struct InterpKey(Hash32);   // key of an Interpretation's render-relevant content (2.12)
pub struct RenderKey(Hash32);   // key of RenderInputs (2.13)
pub struct ControlId(Hash32);   // content of an imported ControlTrack (raw framing)
pub struct PaletteId(Hash32);   // bytes of palettes/<id>.json as snapshotted
pub struct WavId(Hash32);       // bytes of renders/<id>.wav
pub struct MidiFileId(Hash32);  // bytes of midi/<id>.mid
pub struct ProjectId(Hash32);   // supplied by app at creation

/// Local identities. All come from one counter, Project::alloc_id, and are never
/// reused, so an id in the undo history cannot change meaning. Outside every key.
pub struct InterpId(pub u32);
pub struct LayerId(pub u32);
pub struct EditId(pub u32);
```

Keys hash canonical bytes, never JSON text:

```rust
pub trait KeyHash { fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError>; }
pub enum KeyError { NonFinite, DuplicateKey }
pub enum KeyKind { Evidence, HitSet, TeachSet, Interp, Render }
pub fn key_of<T: KeyHash + ?Sized>(kind: KeyKind, v: &T) -> Result<Hash32, KeyError>;

pub struct KeyWriter { buf: Vec<u8> }
impl KeyWriter {
    pub fn int(&mut self, v: i128);                          // tag 0x01, 16 bytes LE: every integer width encodes alike
    pub fn f32(&mut self, v: f32) -> Result<(), KeyError>;   // 0x02, bits; -0.0 as +0.0; NaN or infinity is NonFinite
    pub fn bool(&mut self, v: bool);                         // 0x03
    pub fn str(&mut self, s: &str);                          // 0x04, u64 length + UTF-8
    pub fn bytes(&mut self, b: &[u8]);                       // 0x05, u64 length + bytes
    pub fn record(&mut self, f: impl FnOnce(&mut Record<'_>) -> Result<(), KeyError>) -> Result<(), KeyError>;   // 0x06, fields sorted by name
    pub fn variant(&mut self, name: &str, f: impl FnOnce(&mut Record<'_>) -> Result<(), KeyError>) -> Result<(), KeyError>; // 0x07, identified by name
    pub fn list<T: KeyHash>(&mut self, items: &[T]) -> Result<(), KeyError>;                                     // 0x08, order kept
    pub fn set<'a, T: KeyHash + 'a>(&mut self, items: impl IntoIterator<Item = &'a T>) -> Result<(), KeyError>; // 0x09, sorted by encoding; duplicate is an error
    pub fn map<'a, K: KeyHash + 'a, V: KeyHash + 'a>(&mut self, items: impl IntoIterator<Item = (&'a K, &'a V)>) -> Result<(), KeyError>; // 0x0A, sorted by encoded key
}
impl Record<'_> {
    pub fn field<T: KeyHash + ?Sized>(&mut self, name: &'static str, v: &T) -> Result<(), KeyError>;
    pub fn opt<T: KeyHash>(&mut self, name: &'static str, v: Option<&T>) -> Result<(), KeyError>;     // None is omitted
    pub fn unless<T: KeyHash + PartialEq>(&mut self, name: &'static str, v: &T, reproducing: &T) -> Result<(), KeyError>; // omitted at its frozen reproducing value
}
```

Rules:
- **Exhaustive destructuring.** Every `KeyHash` impl destructures `Self` completely, for example `let Self { a, b, locks: _ } = self;`. A new field then fails to compile until it is classified as keyed or excluded, so a key-scope bug cannot be silent.
- **Reproducing values.** A field added after v0.1 is written with `unless(name, v, REPRODUCING)`. `REPRODUCING` is the frozen value that reproduces old behaviour, so old objects keep their keys. Reproducing values never change. The default for new projects is a separate constant and may change.
- **No f64 in keyed state.** Musical time is stored as integers (2.9). Changing a field's width or meaning means a new field name plus a migration.
- **Frozen fixture.** `tests/fixtures/frozen-keys.txt` pins one key of each kind plus the seed derivation and noise values. Any drift fails the build.

### 2.3 `core::seed`

```rust
/// Serialised as 16 hex digits, because JSON numbers lose precision above 2^53.
pub struct Seed(pub u64);
impl Seed {
    /// blake3 derive_key "takedown 2026 seed-derive v1" over the parent (LE), the
    /// label (length, then bytes) and idx (LE); the first 8 bytes. Called at bind
    /// or lens time, never on the audio thread.
    pub fn derive(self, label: &str, idx: u64) -> Seed;
    pub fn noise(self, stream: u64) -> NoiseStream;   // two integer mixes; fine on the audio thread
}
/// Counter-based noise: value(n) = mix(mix(n + a) ^ b), with the SplitMix64 finaliser. Integer only.
pub struct NoiseStream { a: u64, b: u64 }
impl NoiseStream {
    pub fn u64_at(self, n: u64) -> u64;
    pub fn unit_at(self, n: u64) -> f32;      // [0, 1), 24-bit, exact
    pub fn bipolar_at(self, n: u64) -> f32;   // [-1, 1), 24-bit, exact
}

/// Dice aspects. Their labels ("aspect/body", ...) feed every seed and are frozen.
pub enum Aspect { Body, Mangle, TimingFeel, MouthAmount, PitchReading }

/// A time-anchored reroll of one aspect. The UI arrives in v0.3; the type and its semantics exist now.
pub struct RegionRoll { pub aspect: Aspect, pub span: TimelineSpan, pub roll: u32 }

pub struct SeedTree {
    pub root: Seed,                     // supplied by app
    pub rolls: BTreeMap<Aspect, u32>,   // a missing entry means 0; zero entries are removed
    pub locked: BTreeSet<Aspect>,       // the dice skip these; outside keys
    pub regions: Vec<RegionRoll>,       // sorted by (aspect, span), at most one entry per (aspect, span)
}
impl SeedTree {
    pub fn aspect(&self, a: Aspect) -> Seed;                       // root.derive(label(a), roll(a))
    pub fn aspect_at(&self, a: Aspect, at: TimelinePos) -> Seed;   // aspect(a), folded in order with each region roll of a whose span contains at
    pub fn roll(&mut self, a: Aspect) -> bool;                     // returns false and changes nothing when a is locked
    pub fn roll_unlocked(&mut self);
    pub fn roll_region(&mut self, a: Aspect, span: TimelineSpan);
    pub fn voice_seeds(&self, voice: &str) -> VoiceSeeds;          // aspect(Body | Mangle).derive(voice, 0); never folds region rolls
    /// aspect_at(Body | Mangle, anchor).derive(voice, 0).derive(lane label, anchor).derive("ordinal", ordinal)
    pub fn trigger_seeds(&self, voice: &str, lane: Lane, anchor: TimelinePos, ordinal: u32) -> VoiceSeeds;
}
/// What a backend receives at reset and with each trigger.
#[derive(Clone, Copy)]
pub struct VoiceSeeds { pub body: Seed, pub mangle: Seed }
```

- **Aspect independence.** Rolling one aspect changes only that aspect's seeds. A lock just means "this counter does not move" (principle 6).
- **Region rolls stay local.** A region roll folds only into the per-event seeds of events whose anchor lies inside it. Voice seeds never see region rolls, so a roll cannot reach an event anchored outside its region, however segments merge.
- **Ordinal.** `ordinal` counts the earlier events with the same (lane, anchor) in canonical order. A layered kick and sub from one onset therefore get uncorrelated noise.
- **Lens seeds.** The lens derives its own seeds the same way, through `aspect_at(a, anchor)`, for TimingFeel, MouthAmount and PitchReading.

### 2.4 `core::take`

```rust
/// Hashes under domain "takedown/take-pcm/v1": rate (u32), channels (u16), "f32le",
/// every sample's bits (LE), then the frame count at finish. The disk-writer thread
/// feeds it during capture, and chunking cannot change the id. NaN or infinity
/// means a capture bug and is rejected.
pub struct TakeHasher { .. }
impl TakeHasher {
    pub fn new(rate: SampleRate, channels: u16) -> Self;
    pub fn update(&mut self, interleaved: &[f32]) -> Result<(), TakeError>;
    pub fn finish(self) -> Result<(TakeId, u64 /* frames */), TakeError>;
}
pub struct AudioInfo { pub rate: SampleRate, pub channels: u16, pub frames: u64 }
pub enum CaptureSource {
    Recorded { device: String, latency_compensated: bool },
    Dropped { original_name: String, source_format: String },
}
/// What the host reported at capture. None in v0.1. Not keyed, so f64 is safe here (float_roundtrip).
pub struct HostAnchor { pub host_rate: SampleRate, pub host_sample_at_start: i64, pub ppq_at_start: f64, pub tempo_bpm: f64, pub time_sig: (u8, u8) }
pub struct Take {
    pub id: TakeId,              // content identity; stored as audio/<id>.wav (f32 WAV at audio.rate), immutable
    pub audio: AudioInfo,        // audio.rate is the timeline rate of everything anchored to this take
    pub source: CaptureSource,
    pub anchor: Option<HostAnchor>,
    pub name: String,            // display only
    pub captured_at: String,     // RFC 3339, display only, supplied by app
}
```

Identical PCM is one take. `Project::add_take` returns the existing record instead of adding a second (§7).

### 2.5 `core::evidence`

```rust
/// The computation that produced the evidence. params holds fixed analyser constants
/// only (window sizes, the candidate search's hard bounds). A user's range
/// calibration is lens work and never appears here.
pub struct Provenance { pub analyzer: String, pub version: u32, pub models: BTreeMap<String, Hash32>, pub params: Params }
impl Provenance { pub fn key(&self, take: TakeId) -> Result<EvidenceKey, KeyError>; }

/// Column names are frozen; the container header lists them.
pub enum Column {
    F0Hz0, F0Hz1, F0Hz2, F0Sal0, F0Sal1, F0Sal2,   // top-3 candidates; a missing candidate is 0 Hz at salience 0
    VoicingP, LoudnessDbA, CentroidHz, BrightnessHarm, TiltDbOct, CppDb,
    Vowel0, Vowel1, ClassPitched, ClassNoise, ClassTransient, ClassSilence, Odf,
}
pub struct OnsetCandidate { pub at: TimelinePos, pub strength: f32, pub rise_db: f32 }
pub struct EvidenceHeader { pub key: EvidenceKey, pub take: TakeId, pub take_rate: SampleRate, pub take_frames: u64, pub provenance: Provenance }
pub struct Evidence { header: EvidenceHeader, n_frames: usize, columns: Vec<(Column, Vec<f32>)>, onsets: Vec<OnsetCandidate> }
impl Evidence {
    /// Invariants: n_frames == frames_covering(take_frames, take_rate); every column has
    /// n_frames values; columns are unique and in Column order; all values are finite;
    /// onsets are sorted and inside the take; header.key == provenance.key(take).
    pub fn new(header: EvidenceHeader, columns: Vec<(Column, Vec<f32>)>, onsets: Vec<OnsetCandidate>) -> Result<Self, EvidenceError>;
    pub fn column(&self, c: Column) -> Option<&[f32]>;   // an analyser may omit columns (analyser v1 has no vowel or class hints)
    pub fn onsets(&self) -> &[OnsetCandidate];
    pub fn content_hash(&self) -> Hash32;                 // raw framing of n_frames, column bits and onsets: what was produced
    pub fn to_container(&self) -> Vec<u8>;
    pub fn from_container(bytes: &[u8], expected: EvidenceKey) -> Result<Self, EvidenceError>;
}
pub const CONTAINER_VERSION: u32 = 1;
pub enum EvidenceResolution { UseCached, RecomputeSame, NeedsUpgrade { pinned: EvidenceKey, available: EvidenceKey } }
pub fn resolve_evidence(pinned: EvidenceKey, cache_has_pinned: bool, take: TakeId, running: &Provenance) -> Result<EvidenceResolution, KeyError>;
```

- **Container `.tkev`.** Layout: `"TKEV"`, version (u32), header length (u32), header JSON (evidence header, n_frames, column names, n_onsets, content hash), the columns as f32 LE in Column order, the onsets as (u64, f32, f32) LE, then a blake3 hash of everything before. It is about 8.7 MB and 30 ms for a 10-minute take.
- **Decoding checks:** the frame hash, the magic, the version, the legal length, finiteness, onset order and the content hash. It also checks that the stored key equals `expected`, so a file is never a silent substitute. A failed or old container counts as a cache miss.
- **Reuse rule.** This is the whole of "never silently recompute under a new model":
  - pinned key in the cache: load it;
  - not cached, but the running analyser's provenance hashes to the pinned key: recompute without asking;
  - otherwise: `NeedsUpgrade`, the UI asks, and layers keep their WAVs.
- **Fit with the evidence crate.** The crate's `Analysis` converts into this type. Its frames are centred on `floor(k·rate/200)`, at most one sample from `frame_start`; that offset is analyser detail.

### 2.6 `core::hit`

```rust
/// [a-z0-9_]{1,32}; app turns user names into this form. Roles persist by name, never by index.
#[serde(try_from = "String")] pub struct RoleName(String);
pub struct ClassifierStamp { pub id: String, pub version: u32, pub params: Params }
/// drums output: observations plus soft class scores.
pub struct Hit {
    pub onset: TimelinePos,   // refined onset: the hit's identity and the anchor of everything derived from it
    pub span: TimelineSpan,
    pub loudness_db: f32,
    pub brightness: f32,
    pub decay_db_per_s: f32,
    pub scores: Vec<f32>,     // aligned with HitSet.roles
}
pub struct HitSet { pub key: HitSetKey, pub roles: Vec<RoleName>, pub hits: Vec<Hit> }   // cache/hits/<key>.json
impl HitSet { pub fn content_hash(&self) -> Hash32; }   // raw framing
pub fn hit_set_key(evidence: EvidenceKey, classifier: &ClassifierStamp, teach: Option<TeachSetId>) -> Result<HitSetKey, KeyError>;
/// A teach label is a time, not a feature vector, so a feature-extractor change keeps every label.
pub struct TeachLabel { pub take: TakeId, pub at: TimelinePos, pub role: RoleName }
pub struct TeachSet { pub labels: Vec<TeachLabel> }   // teach/<id>.json, durable
/// Selects the nearest anchor within tol samples (timeline rate); a tie goes to the earlier one.
pub struct HitAnchor { pub at: TimelinePos, pub tol: u32 }
impl HitAnchor { pub fn resolve(&self, anchors: &[TimelinePos]) -> Option<usize>; }
```

The plan's `Hit` has an exciter slice ref. It is not stored: the slice is derived from the anchor (2.7), so neither a cache file nor MIDI has to carry it.

### 2.7 `core::control`

```rust
pub const AMP_FLOOR_DB: f32 = -120.0;
pub const MAX_HIT_MS: u32 = 60_000;
pub const MAX_ROLES: usize = 128;

pub enum Curve { PitchSt, AmpDb, Bright, Breath, VowelX, VowelY }
/// Six curves on the 200 Hz grid, each frames_covering(len, rate) long (at least 1).
/// Ranges: pitch_st is MIDI semitones (69 = A4), finite, in [-64, 192]; amp_db in
/// [AMP_FLOOR_DB, 12]; bright and breath in [0, 1]; vowel_x and vowel_y in [-1, 1].
/// Values outside spans are whatever the lens wrote; MIDI fills them canonically (2.9).
pub struct Curves { pub pitch_st: Vec<f32>, pub amp_db: Vec<f32>, pub bright: Vec<f32>, pub breath: Vec<f32>, pub vowel_x: Vec<f32>, pub vowel_y: Vec<f32> }
pub struct ControlFrame { pub pitch_st: f32, pub amp_db: f32, pub bright: f32, pub breath: f32, pub vowel: [f32; 2] }

#[serde(try_from = "u8", into = "u8")] pub struct U7(u8);         // 0..=127
#[serde(try_from = "u8", into = "u8")] pub struct Velocity(u8);   // 1..=127

pub enum Link {
    Detached,    // a gap comes before this span, or it is the first span
    Retrigger,   // abuts the previous span: envelopes restart, the gate never falls
    Legato,      // abuts: nothing restarts; a notational boundary for NoteView and MIDI only
}
pub enum Lane { Phrase, Hits }

/// The monophonic phrase lane. The gate is exactly the union of the spans; there is
/// no gate curve, so the gate and NoteOn/NoteOff cannot disagree.
pub struct GateSpan { pub on: TimelinePos, pub off: TimelinePos, pub anchor: TimelinePos, pub vel: Velocity, pub link: Link }

/// The polyphonic hit lane. Every field is something MIDI carries, so the lane is
/// self-contained: hit voices read these fields, a slice and seeds, never curves.
pub struct HitEvent {
    pub at: TimelinePos,      // when it fires, after timing feel and perceptual-centre pre-roll
    pub anchor: TimelinePos,  // identity: the evidence onset before anything moved it; seeds and the take slice key on it
    pub dur: u64,             // samples; drives decay; the MIDI note length
    pub role: u8,             // index into ControlTrack.roles
    pub vel: Velocity,        // accent, from loudness
    pub bright: U7,           // tone
    pub breath: U7,           // noise amount
    pub mouth: U7,            // how much of the take's own transient strikes the body
}

pub struct TrackParts {
    pub rate: SampleRate,       // timeline rate
    pub len: u64,               // samples, 1..=2^40
    pub take: Option<TakeId>,   // where slices come from; when Some, rate equals that take's rate
    pub roles: Vec<RoleName>,   // sorted, unique, at most MAX_ROLES
    pub curves: Curves,
    pub spans: Vec<GateSpan>,
    pub hits: Vec<HitEvent>,
}

/// The only renderer input. It can exist only validated: serde goes through TrackParts.
#[serde(try_from = "TrackParts", into = "TrackParts")]
pub struct ControlTrack { /* the fields of TrackParts */ }
impl ControlTrack {
    pub fn new(parts: TrackParts) -> Result<Self, TrackError>;   // sorts hits canonically, then validates
    pub fn into_parts(self) -> TrackParts;
    pub fn rate(&self) -> SampleRate;
    pub fn len(&self) -> u64;
    pub fn take(&self) -> Option<TakeId>;
    pub fn roles(&self) -> &[RoleName];
    pub fn curves(&self) -> &Curves;
    pub fn spans(&self) -> &[GateSpan];
    pub fn hits(&self) -> &[HitEvent];
    pub fn frames(&self) -> u64;
    pub fn value_at(&self, rate: SampleRate, pos: u64) -> ControlFrame;   // a pure function of the absolute sample, at any rate
    pub fn at_clock(&self, clock: &ControlClock) -> ControlFrame;         // the same value through an incremental clock
    pub fn gate_at(&self, pos: TimelinePos) -> Option<usize>;             // the span containing pos
    pub fn events(&self) -> EventIter<'_>;                                // canonical order; allocates nothing
    pub fn slice_span(&self, anchor: TimelinePos, len: u64) -> Option<TimelineSpan>;  // [anchor, anchor + len) clipped to the take; None without a take
    pub fn content_id(&self) -> ControlId;                                // raw framing of every field
    pub fn reattach_anchors(&mut self, reference: Option<&ControlTrack>, onsets: &[TimelinePos], tol: u64);
}
pub struct Event { pub at: TimelinePos, pub anchor: TimelinePos, pub kind: EventKind }
pub enum EventKind { NoteOn { span: u32, vel: Velocity }, Retrigger { span: u32, vel: Velocity }, NoteOff { span: u32 }, Hit { hit: u32 } }
pub enum TrackError {
    Length,
    CurveLength { curve: Curve },
    OutOfRange { curve: Curve, frame: u64 },
    EmptySpan(usize), SpanOrder(usize), SpanBeyondEnd(usize), LinkMismatch(usize),
    HitDuration(usize), HitBeyondEnd(usize), HitOrder(usize), RoleIndex(usize),
    Roles,
    AnchorBeyondEnd,
}
/// controls/<ControlId>.json: an imported track as a versioned document.
#[serde(deny_unknown_fields)] pub struct ControlFile { pub schema_version: u32, pub track: ControlTrack }
```

**Invariants**, checked by `new` and on load:
- `len` is in 1..=2^40. Every curve is `frames_covering(len, rate)` long, with finite values in range.
- Spans are non-empty, sorted and non-overlapping, with `off <= len`.
- The first span is `Detached`. For any later span, `link == Detached` exactly when `on` is after the previous span's `off`.
- Hits satisfy:
  - `1 <= dur <= MAX_HIT_MS` at `rate`;
  - `at.checked_add(dur) <= len` (overflow is an error, never a panic);
  - `role < roles.len()`;
  - sorted by `(at, role, anchor, vel, bright, breath, mouth, dur)`, so equal tracks have equal vectors.
- Every anchor is `<= len`.

**Events** come in canonical order: by `at`; at equal positions, phrase events before hits, and hits in canonical order.
- At `on`, a span emits `NoteOn` if Detached, `Retrigger` if Retrigger, and nothing if Legato.
- At `off`, it emits `NoteOff` unless the next span abuts it.
- Gate events carry no pitch. A backend reads pitch from the curve, so the renderer never sees notes.

**Anchors:**
- **Lens order.** The lens builds its draft with `at == anchor`, where the anchor is the HitSet onset for a hit or the evidence onset for a span. It then applies edits against anchors. Only after that does it move `at` for timing feel and perceptual-centre pre-roll. Anchors never move.
- **Re-attaching after import.** MIDI carries no anchors, so `reattach_anchors` restores them after an import. Each hit is processed in canonical order and takes the first of:
  1. the anchor of an unmatched reference hit with the same role and the same `at`;
  2. failing that, the anchor of the unmatched same-role reference hit nearest `at` within `tol`;
  3. failing that, the nearest onset within `tol`;
  4. failing that, `at`.

  Spans are matched the same way, by `on`.
- **The reference** is `quantise` of the track that was exported. An unedited round trip therefore restores every anchor exactly, and a hit nudged in Reaper keeps its sound.

### 2.8 `core::views`

```rust
pub struct NoteView { pub span: usize, pub on: TimelinePos, pub off: TimelinePos, pub key: u8, pub vel: Velocity, pub link: Link, pub frames: Range<u64> }
pub struct HitView { pub at: TimelinePos, pub dur: u64, pub role: RoleName, pub key: Option<u8>, pub vel: Velocity, pub bright: U7, pub breath: U7, pub mouth: U7 }
pub fn note_view(t: &ControlTrack) -> Vec<NoteView>;                 // exactly one note per span
pub fn hit_view(t: &ControlTrack, roles: &RoleMap) -> Vec<HitView>;
pub fn derive_key(pitch: &[f32], frames: Range<u64>, rep: usize) -> u8;
```

- **Boundaries.** A note's boundaries, velocity and link come only from its span. No curve change can move, split or merge a note.
- **Key.** The key is the rounded lower median of `pitch_st` over the frames the note covers, clamped to 0..=127. A covered frame is one whose `frame_start` lies in `[on, off)`. A span under 5 ms covers no frame and uses the pitch at the frame containing `on`.
  - The median ignores scoops, vibrato and short octave blips. The key changes only when the median crosses a half-semitone.
- **Use.** Views are read-only, derived and never stored. The MIDI importer produces a ControlTrack, never views.

### 2.9 `core::musical` and `core::canonical`: the MIDI projection

```rust
// core::musical. Musical time is a lens over integer SMF time; no f64 is stored.
pub struct TempoSegment { pub tick: u64, pub us_per_quarter: u32 }    // SMF's own unit
/// Non-empty, first tick 0, strictly increasing ticks, tempi in 1..=0xFF_FFFF.
#[serde(try_from = "Vec<TempoSegment>", into = "Vec<TempoSegment>")]
pub struct TempoMap(Vec<TempoSegment>);
impl TempoMap {
    pub fn constant(us_per_quarter: u32) -> Result<Self, TimeMapError>;
    pub fn from_bpm(bpm: f64) -> Result<Self, TimeMapError>;   // the one lossy step, done once
}
/// Timeline samples <-> ticks with exact u128 rationals, piecewise across tempo changes.
pub struct TimeMap { ppq: u16, tempo: TempoMap, origin_tick: u64 /* the tick of timeline sample 0 */ }
impl TimeMap {
    pub fn new(ppq: u16, tempo: TempoMap, origin_tick: u64) -> Result<Self, TimeMapError>;  // PPQ 1..=32767; any SMF map can be read
    pub fn default_export() -> Self;                                                         // 960 PPQ, 120 BPM, origin 0
    pub fn check_exportable(&self, rate: SampleRate) -> Result<(), TimeMapError>;            // a tick is <= 5 ms and > 1 sample
    pub fn exportable_for(&self, rate: SampleRate) -> Option<TimeMap>;                       // coarsest integer PPQ refinement that passes
    pub fn tick_of_sample(&self, s: TimelinePos, rate: SampleRate) -> u64;                   // nearest
    pub fn sample_of_tick(&self, t: u64, rate: SampleRate) -> TimelinePos;                   // nearest
    pub fn tick_of_frame(&self, k: u64) -> u64;
}

// core::canonical
/// Role <-> MIDI key, injective. fallback is one of the keys; roles missing from the map export as it.
/// gm_default(): kick 36, snare 38, hat_closed 42, hat_open 46, clap 39, rim 37,
/// tom_low 45, tom_high 50, crash 49, perc 56 (the fallback).
pub struct RoleMap { pub keys: BTreeMap<RoleName, u8>, pub fallback: RoleName }
/// One MPE lower zone. Defaults: phrase lane on channels 2-4, hit lane on 5-16, bend
/// range ±48, CC74 brightness, CC2 breath, CC16/17 vowel; unassigned channels are read as hits.
pub struct MpeProfile {
    pub version: u16,
    pub bend_range_st: u8,
    pub phrase_channels: (u8, u8),   // zero-based inclusive range, at least 3 channels; default (1, 3)
    pub hit_channels: (u8, u8),      // zero-based inclusive range; default (4, 15)
    pub cc_bright: u8,
    pub cc_breath: u8,
    pub cc_vowel: (u8, u8),
    pub roles: RoleMap,
    pub unassigned: Lane,
}
/// How an interpretation serialises to MIDI. It affects export only and stays outside InterpKey.
pub struct MidiBinding { pub profile: MpeProfile, pub time: TimeMap }
pub struct QuantiseReport { pub choked_hits: u32, pub dropped_spans: u32, pub clamped_frames: u32, pub remapped_roles: u32 }
impl ControlTrack {
    /// The one named lossy step: exactly what an MPE file under `b` can carry.
    pub fn quantise(&self, b: &MidiBinding) -> Result<(ControlTrack, QuantiseReport), QuantiseError>;
}
/// Snaps to the nearest lattice point that has the same rounded semitone as p.
pub fn pitch_to_wire(p: f32, key: u8, range: u8) -> u16;
// The other to/from-wire and snap functions follow the table below.
```

`quantise` establishes exactly these six conditions and changes nothing else; anchors and the take are kept:
1. Every span boundary and every hit start and end sits on the sample nearest some tick. Spans and hits last at least one tick; shorter spans are dropped and reported.
2. Every frame a span covers holds a lattice value relative to that span's derived key.
3. Frames no span covers hold canonical fill:
   - after a span, the last covered value holds;
   - the frame just before a span preloads its first value;
   - with no spans, rest values (pitch 60, the amp floor, 0 for the others).
4. Links follow tick adjacency. A gap is Detached. An abutting span with Legato intent stays Legato, and any other abutting span is Retrigger.
5. At most `hit_channels` hits overlap at any tick, counting a note's end tick as busy. When more would overlap, the hit that ends soonest is choked.
6. Roles missing from the RoleMap become the fallback.

| Quantity | Wire | Lattice | Worst error |
|---|---|---|---|
| pitch_st inside a span | 14-bit member bend about the span's key | key + j·range/8192 st, with the input's rounded semitone | one step: 0.59 cent at ±48 |
| amp_db inside a span | channel pressure | −63.5 + 0.5·p dB | 0.25 dB; floored at −63.5 dB |
| bright, breath | CC74, CC2 | q/127 | 1/254 |
| vowel_x, vowel_y | CC16, CC17 | (q − 64)/64 | 1/128 |
| times | ticks | the sample nearest a tick | ½ tick: 0.26 ms at 960 PPQ and 120 BPM |
| hit vel, bright, breath, mouth | velocity; CC74, CC2 and pressure before note-on on the hit's channel | already 7-bit in the type | none |
| anchors | not carried | restored by `reattach_anchors` | exact for unedited hits |
| take slices | not carried | derived from anchors | none |

The renderer plays the lens's full-resolution track:
- Audio and `.mid` describe the same performance within this table (principle 2).
- An imported `.mid` renders exactly as `quantise` of its source would.
- The polyphony choke and the −63.5 dB floor therefore affect only the file, and the export reports them.

### 2.10 `core::edit`

```rust
pub const DEFAULT_TOL_MS: u32 = 25;   // the onset F1 window
pub const SPLIT_SNAP_MS: u32 = 20;
pub const MIN_PIECE_MS: u32 = 10;

pub enum Articulation { Retrigger, Legato }
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    ForcePitch { span: TimelineSpan, st: f32 },   // every frame whose time lies in span
    SplitAt { at: TimelinePos, articulation: Articulation },
    JoinAt { at: TimelinePos, tol: u32 },
    ForceRole { hit: HitAnchor, role: RoleName },
    DeleteHit { hit: HitAnchor },
    AddHit { hit: HitAnchor, dur: u64, role: RoleName, vel: Velocity },
    MuteSpan { span: TimelineSpan },
}
pub struct EditEntry { pub id: EditId, pub edit: Edit }   // the id is outside keys
pub enum Dormancy { NoTarget, TooShort, OutOfRange }
pub enum Outcome { Applied, Merged, NoOp, Dormant(Dormancy) }
pub struct EditReport { pub id: EditId, pub outcome: Outcome }
/// Pure: the same draft and edits give the same track and the same reports.
pub fn apply_edits(draft: TrackParts, edits: &[EditEntry]) -> Result<(ControlTrack, Vec<EditReport>), TrackError>;
impl Edit {
    /// Positions via rescale_round, tolerances scaled, dur kept >= 1.
    pub fn rebased(&self, from: SampleRate, to: SampleRate) -> Edit;
}
```

- **Anchoring.** Anchors are timeline samples, the one clock that no slider, tempo, render rate or re-segmentation can move. Edits apply in list order, so a later edit wins. They apply to the lens's draft after derivation and before timing feel and perceptual-centre pre-roll move `at`, and they find their targets by anchor.
- **Targeting.** An edit takes the nearest anchor within its tolerance, the earlier one on a tie. It is never retargeted outside the tolerance.
- **Dormancy.** An edit that finds no target is dormant:
  - it is kept and reported per id;
  - it is retried on every derivation and never deleted automatically;
  - it applies again when a target reappears.
- **ForcePitch and MuteSpan** target time itself. They are `Dormant(OutOfRange)` only when their span misses the track. MuteSpan clips spans, splitting one that straddles it. It removes hits anchored inside it and cuts the tails of hits that started before it.
- **SplitAt.**
  - If an internal boundary (an abutting pair) lies within `SPLIT_SNAP_MS`, that boundary moves to `at` and takes the articulation.
  - Otherwise the covering span splits at `at`, and the new span's anchor is `at`.
  - A piece shorter than `MIN_PIECE_MS` gives `Dormant(TooShort)`. With no covering span, the result is `Dormant(NoTarget)`.
  - Applying a split twice is the same as applying it once.
- **JoinAt** merges the consecutive pair whose junction is nearest `at` within `tol`; the junction is empty when the pair abuts. A gap between them is filled. The merged span keeps the first span's anchor, velocity and link.
- **ForceRole** sets the role by name, adding it to the role table if needed. **DeleteHit** removes the hit.
- **AddHit.**
  - If any hit lies within `tol` of `hit.at`, that hit takes `role`: the outcome is `Merged`, or `NoOp` if it already had that role.
  - Otherwise a hit is inserted with `at = anchor = hit.at`, `dur` clamped to `len`, the given role and velocity, bright 64, breath 0 and mouth 0. The lens stages that run after edits (Mouth amount, timing feel) treat it like any other hit.
  - Outside the track the outcome is `Dormant(OutOfRange)`.
- **Scope.** In v0.1 only Lens sources have edits. An Imported track is edited in Reaper.

### 2.11 `core::lens` and `core::params`

```rust
/// Names are frozen: they appear in project.json and in keys.
pub enum Slider { MouthAmount, Fidelity, Mangle, TimingFeel, Register, GlideRetrigger }
impl Slider { pub fn shapes_track(self) -> bool; }   // true for all but Mangle; those sliders are inert for an Imported source
pub struct Sliders {
    pub mouth_amount: f32,      // 0..=1
    pub fidelity: f32,          // 0..=1
    pub mangle: f32,            // 0..=1
    pub timing_feel: f32,       // 0..=1
    pub register_st: i8,        // -36..=36
    pub glide_retrigger: f32,   // -1 (always retrigger) ..= 1 (always glide)
}
pub struct PaletteRef { pub name: String /* display; outside keys */, pub id: PaletteId }
/// Versioned by the project schema, not on its own.
pub struct LensSettings { pub palette: PaletteRef, pub sliders: Sliders, pub locks: BTreeSet<Slider> /* outside keys */ }
impl LensSettings { pub fn validate(&self) -> Result<(), LensError>; }

#[serde(untagged)]
pub enum ParamValue { Bool(bool), Int(i64), Float(f32), Text(String) }   // Int comes before Float; Float must be finite
pub type Params = BTreeMap<String, ParamValue>;
```

- **Palettes.** The snapshot `palettes/<id>.json` is render's schema: voices with stable names, engines, ranges and each voice's route. Core stores the file and hashes its bytes. Editing a palette makes a new snapshot, so it cannot change an old project's sound or make its key lie.
- **Locks.** Core stores slider locks; what a lock prevents is lens and app behaviour.
- **Quantise.** The plan's quantise setting arrives with the `notes` lens in v0.2 as a new field (§6).

### 2.12 `core::interp`

```rust
pub struct Interpretation {
    pub id: InterpId,            // outside keys
    pub name: String,            // outside keys
    pub take: Option<TakeId>,    // required for Lens; optional for Imported (anchors and slices)
    pub source: ControlSource,
    pub lens: LensSettings,      // kept for both sources: palette and Mangle still apply to Imported
    pub seeds: SeedTree,         // the dice live here, so an Imported source keeps its rolls
    pub midi: MidiBinding,       // export only; outside keys
}
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlSource {
    /// Re-derived in milliseconds; the track is never stored.
    Lens { evidence: EvidenceKey, hits: Option<HitSetKey>, edits: Vec<EditEntry> },
    /// An edited .mid, decoded once, with anchors re-attached, stored at
    /// controls/<control>.json. The original file stays at midi/<file>.mid as provenance.
    Imported { control: ControlId, file: Option<MidiFileId> },
}
impl Interpretation {
    pub fn key(&self) -> Result<InterpKey, KeyError>;
    /// A Lens source needs a take; the evidence key's take must equal that take;
    /// a track's timeline rate must equal its take's rate.
    pub fn validate(&self, takes: &BTreeMap<TakeId, Take>) -> Result<(), InterpError>;
}
```

| InterpKey | Includes | Excludes |
|---|---|---|
| Lens | take, evidence key, hits key, edit ops in order, palette id, all six sliders, seed root, non-zero rolls, region rolls | id, name, slider locks, aspect locks, edit ids, palette name, MidiBinding |
| Imported | control id, take, palette id, Mangle, seed root, non-zero rolls, region rolls | the above, plus the track-shaping sliders and the file id |

Edit order stays in the key: edits apply in order, so order is meaning.

### 2.13 `core::layer`

```rust
pub struct BackendStamp { pub id: String, pub version: u32 }
/// Everything that determines a render's bits, as the running build sees it.
pub struct RenderInputs {
    pub interp: InterpKey,
    pub evidence: Option<Hash32>,      // content hash of the evidence actually used; None for Imported
    pub hits: Option<Hash32>,          // content hash of the hit set actually used
    pub lens_version: u32,             // the running render::lens::LENS_VERSION
    pub backends: Vec<BackendStamp>,   // sorted and unique: the backends actually used
    pub contract: u32,                 // the running RENDER_CONTRACT_VERSION
    pub rate: SampleRate,
}
impl RenderInputs { pub fn key(&self) -> Result<RenderKey, KeyError>; }
pub struct PlatformClass { pub arch: String, pub os: String }   // provenance; outside the key
pub struct RenderRef { pub key: RenderKey, pub inputs: RenderInputs, pub wav: WavId, pub frames: u64, pub platform: PlatformClass }
pub struct Layer { pub id: LayerId, pub interp: InterpId, pub gain_db: f32, pub mute: bool, pub renders: BTreeMap<SampleRate, RenderRef> }
pub enum StaleReason {
    InterpretationChanged,
    EvidenceContentChanged,
    HitsChanged,
    LensUpgraded { from: u32, to: u32 },
    BackendsChanged { from: Vec<BackendStamp>, to: Vec<BackendStamp> },
    ContractUpgraded { from: u32, to: u32 },
}
pub enum LayerStatus { NotRendered, Fresh, Stale(Vec<StaleReason>) }
impl Layer {
    pub fn status(&self, current: &RenderInputs) -> LayerStatus;   // compares against the slot at current.rate
    pub fn store_render(&mut self, r: RenderRef);
}
```

- **Durable WAVs.** A render is a mono f32 WAV at `renders/<WavId>.wav`, outside `cache/`. A stale render is still the durable artefact (principle 9): it stays playable and exportable.
- **Slots per rate.** There is one slot per output rate, so a rate switch never discards a render. Gain and mute are outside the key.
- **Platform class** is provenance only. A macOS render opened on Windows is not forced to re-render.

### 2.14 `core::project` and `core::migrate`

```rust
pub const SCHEMA_VERSION: u32 = 1;
pub struct PaletteMeta { pub name: String }
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub id: ProjectId,
    pub output_rate: SampleRate,
    pub takes: BTreeMap<TakeId, Take>,
    pub palettes: BTreeMap<PaletteId, PaletteMeta>,
    pub teach_set: Option<TeachSetId>,
    pub interpretations: Vec<Interpretation>,
    pub layers: Vec<Layer>,
    pub next_id: u32,
}
pub enum TakeAdded { New, Existing }
pub struct Loaded { pub project: Project, pub migrated_from: Option<u32> }
impl Project {
    pub fn new(id: ProjectId, output_rate: SampleRate) -> Self;
    pub fn alloc_id(&mut self) -> u32;
    pub fn add_take(&mut self, take: Take) -> TakeAdded;
    /// Referential integrity (takes, palettes, interpretations), unique ids, finite
    /// values, every RenderRef.key == key(inputs), every Interpretation valid.
    pub fn validate(&self) -> Result<(), ProjectError>;
    pub fn to_json(&self) -> Vec<u8>;   // deterministic: the same project always gives the same bytes
    /// Reads the version (required) -> TooNew? -> migrates the untyped JSON ->
    /// typed decode (deny_unknown_fields) -> validate.
    pub fn from_json(bytes: &[u8]) -> Result<Loaded, ProjectError>;
    pub fn save(&self, dir: &Path, migrated_from: Option<u32>) -> io::Result<()>;
}
pub mod paths { /* audio(TakeId), render(WavId), control(ControlId), midi(MidiFileId), palette(PaletteId),
                   teach(TeachSetId), evidence_cache(EvidenceKey), hits_cache(HitSetKey), peaks(TakeId) */ }

// core::migrate
pub type StepFn = fn(&mut serde_json::Value) -> Result<(), MigrationError>;
pub struct Step { pub from: u32, pub apply: StepFn }
pub const STEPS: &[Step] = &[];   // the production chain is empty at schema 1
pub enum MigrationError { MissingVersion, TooNew { found: u32, supported: u32 }, Gap { at: u32 }, Step { from: u32, msg: String } }
pub fn read_version(doc: &serde_json::Value) -> Result<u32, MigrationError>;   // a missing version is an error, never 0
pub fn migrate(doc: &mut serde_json::Value, steps: &[Step], current: u32) -> Result<Option<u32>, MigrationError>;
pub fn chain_is_total(steps: &[Step], current: u32) -> bool;
```

**Persistence rules:**
1. **One version per document.** `project.json`, `controls/*.json` and `teach/*.json` each carry a `schema_version`. The `.tkev` and hit caches carry a container version, and a mismatch is a cache miss.
2. **Explicit, strict files.** Every persisted struct and enum is `deny_unknown_fields`. Nothing on disk uses `serde(default)` or `skip_serializing_if`: every field is written explicitly.
3. **Every on-disk change bumps the version,** additive changes included.
   - Steps are `fn(&mut Value)` and run before typed decoding, so old Rust types never have to exist.
   - `chain_is_total` is tested.
4. **New fields.** A new field arrives through a step that writes its reproducing value into old files. Its key entry uses `unless(.., REPRODUCING)`, so old keys hold. Changing a field's meaning or width means a new name plus a step; names are never reused.
5. **Newer documents are refused** (`TooNew`), never opened and truncated.
6. **Frozen fixtures.** Each version bump freezes a fixture of the old version under `tests/fixtures/`. Every fixture loads forever, with frozen keys.
7. **Atomic save.** `save` writes a temp file in the same directory, fsyncs it and renames it over the target. Before the first save over a migrated file it copies the original to `project.v{N}.json.bak`.
8. **Durable and cache directories.** `project.json` holds no paths; `paths::` derives them from ids.

   | Kind | Paths |
   |---|---|
   | Durable | `project.json`, `audio/`, `renders/`, `controls/`, `midi/`, `palettes/`, `teach/` |
   | Deletable at any time | `cache/evidence/<key>.tkev`, `cache/hits/<key>.json`, `cache/peaks/<take>.pk` |

### 2.15 `render::contract`

```rust
/// Covers core interpolation and rounding, seed derivation and noise, the segment
/// plan, the event order and the slice resampler. Bump on any change to their output.
pub const RENDER_CONTRACT_VERSION: u32 = 1;

pub struct BackendInfo {
    pub id: &'static str,
    pub version: u32,            // bump on any change that alters output
    pub realtime_capable: bool,  // tract plans are not, yet
    pub tail_frames: u32,        // control frames output may continue after activity ends; audited by testkit
    pub cell: u32,               // block-rate grid in samples at the prepared rate (0 or 1: none)
    pub slice_max_ms: u32,       // take audio read per event: 0 none; 30 for exciters; u32::MAX for VoiceMaterial
}
impl BackendInfo { pub fn stamp(&self) -> BackendStamp; }
/// Output lags control time by `latency` samples (resamplers, lookahead). Constant after prepare.
pub struct Prepared { pub latency: u32 }

/// A voice is a backend instance that includes its own mangle chain. There is no global bus.
pub trait RenderBackend: Send {
    fn info(&self) -> BackendInfo;
    fn prepare(&mut self, rate: SampleRate, max_block: usize) -> Prepared;   // the only place allowed to allocate
    fn reset(&mut self, track: &BoundTrack, seeds: VoiceSeeds, at: RenderPos);
    fn cell(&mut self, track: &BoundTrack, at: RenderPos) { let _ = (track, at); }
    fn trigger(&mut self, track: &BoundTrack, trig: &BoundTrigger);
    /// out.len() <= max_block, and no event falls strictly inside the run.
    fn run(&mut self, track: &BoundTrack, start: RenderPos, out: &mut [f32]);
}
```

Laws a backend must obey (the driver guarantees everything else):
1. Only `prepare` allocates, locks, blocks or does I/O.
2. `reset` leaves the state a pure function of `(track, seeds, at)`.
3. **Split law.** `run(s, out[..a+b])` writes the same samples as `run(s, out[..a])` followed by `run(s+a, out[a..a+b])`.
4. Block-rate state (the 32-sample mod matrix) advances only in `cell`, which the driver calls at every absolute multiple of `info.cell`.
5. Randomness comes only from the seeds handed in, through `NoiseStream`.
   - Per-trigger draws use the trigger's seeds and are indexed by `n − trig.at`, so a moved trigger produces a shifted copy of itself.
   - Voice-level draws use the voice seeds and are indexed by the absolute `n`.
6. Output is silent once activity plus `tail_frames` has passed.
7. A hit-only voice (`route.phrase == false`) reads no curves. For such a voice `BoundTrack::cursor` returns `None`.

```rust
/// Which events a voice takes. Hit voices route by role, so MIDI (which carries
/// roles as keys) recovers the routing; the track holds no voice slots.
pub struct VoiceRoute { pub phrase: bool, pub roles: BTreeSet<RoleName> }
pub struct BindSpec<'a> {
    pub voice: &'a str,          // the voice's stable name from the palette; its seeds derive from it
    pub route: &'a VoiceRoute,
    pub out_rate: SampleRate,
    pub info: &'a BackendInfo,
    pub prepared: Prepared,
    pub seeds: &'a SeedTree,
}
/// Reads take audio resampled to the output rate. Implemented by render; its
/// algorithm is covered by RENDER_CONTRACT_VERSION.
pub trait SliceSource { fn read(&mut self, take: TakeId, span: TimelineSpan, out_rate: SampleRate) -> Option<Vec<f32>>; }
pub struct SliceId(pub u32);
/// The union of all slice spans, resampled once into one buffer. Each slice is a
/// range into it: no copy per event, and at most one resampled take in memory.
pub struct SliceBank { data: Box<[f32]>, ranges: Box<[(u32, u32)]> }

pub struct BoundTrigger {
    pub at: RenderPos,       // the first output sample at or after the exact time
    pub lead: f32,           // the exact time is at - lead, with lead in [0, 1): for band-limited onsets
    pub anchor: TimelinePos,
    pub seeds: VoiceSeeds,   // SeedTree::trigger_seeds(voice, lane, anchor, ordinal)
    pub kind: BoundKind,
}
pub enum BoundKind {
    NoteOn { vel: Velocity, slice: Option<SliceId> },
    Retrigger { vel: Velocity, slice: Option<SliceId> },
    NoteOff,
    Hit { role: u8, vel: Velocity, bright: U7, breath: U7, mouth: U7, dur: u64 /* output samples, >= 1 */, slice: Option<SliceId> },
}
/// Active from start to quiet_from; quiet (driver-written zeros) from there to the next start.
pub struct Segment { pub start: RenderPos, pub frame: u64, pub quiet_from: RenderPos }

/// Immutable; built off the audio thread and shared by Arc.
pub struct BoundTrack { id: u64, track: Arc<ControlTrack>, rate: SampleRate, phrase: bool, role_mask: u128, voice_seeds: VoiceSeeds,
                        triggers: Box<[BoundTrigger]>, segments: Box<[Segment]>, end: RenderPos, slices: SliceBank }
impl BoundTrack {
    pub fn bind(track: Arc<ControlTrack>, spec: &BindSpec<'_>, slices: &mut dyn SliceSource) -> BoundTrack;  // allocates; worker thread only
    pub fn id(&self) -> u64;                       // unique within the process
    pub fn rate(&self) -> SampleRate;
    pub fn voice_seeds(&self) -> VoiceSeeds;
    pub fn triggers(&self) -> &[BoundTrigger];     // sorted by at; ties keep canonical order
    pub fn segments(&self) -> &[Segment];          // strictly increasing starts; segments()[0].start == 0
    pub fn end(&self) -> RenderPos;                // the last quiet_from; an offline render needs [0, end)
    pub fn slice(&self, id: SliceId) -> &[f32];    // empty for an unknown id; never panics
    pub fn cursor(&self, pos: RenderPos) -> Option<ControlCursor<'_>>;   // None for a hit-only voice (law 7)
    pub fn segment_at(&self, pos: RenderPos) -> usize;
    pub fn reset_point_before(&self, pos: RenderPos) -> RenderPos;       // where a partial re-render starting at pos begins
}
pub struct ControlCursor<'a> { track: &'a ControlTrack, clock: ControlClock }
impl ControlCursor<'_> { pub fn get(&self) -> ControlFrame; pub fn advance(&mut self); }

pub const ROUNDING_SLACK_FRAMES: u64 = 1;
pub fn activity_frames(track: &ControlTrack, phrase: bool, role_mask: u128) -> Vec<(u64, u64)>;
pub fn segment_plan(activity_with_tail: &[(u64, u64)]) -> Vec<(u64, u64)>;   // (reset frame, quiet-from frame) pairs
```

`bind` works in four steps:
1. **Triggers.** It takes the voice's events from `ControlTrack::events()`: phrase events if `route.phrase`, and hits whose role is in `route.roles`.
   - `at` comes from `rescale_ceil`, with its `lead`.
   - A hit's `dur` comes from `rescale_round`, at least 1.
   - Seeds come from `trigger_seeds`, with the event's ordinal.
2. **Slices,** when `slice_max_ms > 0` and the track has a take:
   - a hit reads `slice_span(anchor, min(dur, slice_max))`;
   - a NoteOn or Retrigger reads `slice_span(anchor, min(time to the voice's next phrase event, slice_max))`.

   A slice never outlives its event, so the activity always covers it.
3. **Activity,** in control frames, so it does not depend on the output rate.
   - A phrase span covers `[frame(on), frames_covering(off))`. A hit covers `[frame(at), frames_covering(at + dur))`.
   - Each end gets `ROUNDING_SLACK_FRAMES` extra. Output-rate rounding moves a trigger's end by under 1.5 output samples, which is under 0.3 frames at any accepted rate.
   - Each interval is extended by `tail_frames + frames_covering(latency, out_rate)`, then the intervals are merged.
4. **Plan.** There is a reset at frame 0 and at the start of every interval that follows at least `MIN_GAP_FRAMES` of quiet. Shorter gaps stay inside one segment, where the backend runs through them and keeps its state. The monolithic render and every partial render reset at the same samples.

```rust
pub enum DriverEvent { Reset { at: RenderPos }, Cell { at: RenderPos }, Trigger { index: usize }, Run { start: RenderPos, len: usize }, Silence { start: RenderPos, len: usize } }
/// Allocates nothing; starting one costs three binary searches.
pub struct EventCursor { .. }
impl EventCursor {
    pub fn new(track: &BoundTrack, at: RenderPos, cell: u32, max_run: usize) -> Self;
    pub fn next_event(&mut self, track: &BoundTrack, end: RenderPos) -> Option<DriverEvent>;
}
pub struct Driver { cursor: Option<EventCursor>, cell: u32, max_block: usize, latency: u64, scratch: Box<[f32]>, bound_id: u64 }
impl Driver {
    pub fn new(info: &BackendInfo, prepared: Prepared, max_block: usize) -> Self;   // allocates the pre-roll scratch; worker thread only
    /// The first output sample is the segment's start. Pre-rolls the latency.
    pub fn start<B: RenderBackend + ?Sized>(&mut self, b: &mut B, t: &BoundTrack, segment: usize);
    /// Takes any out.len(), splits at max_block, allocates nothing. Debug-asserts t.id() is the started track.
    pub fn process<B: RenderBackend + ?Sized>(&mut self, b: &mut B, t: &BoundTrack, out: &mut [f32]);
    pub fn position(&self) -> Option<RenderPos>;
}
pub struct Cancelled;
/// The worker loop. Renders [segments[from].start, end) in max_block blocks and hands
/// each block to `sink`. Render mixes voices into the layer or streams them to disk,
/// so a 10-minute take never needs one buffer per voice (the 600 MB RSS budget).
pub fn render_offline<B: RenderBackend + ?Sized>(b: &mut B, t: &BoundTrack, d: &mut Driver, from: usize, end: RenderPos,
    sink: &mut dyn FnMut(RenderPos, &[f32]), cancel: &AtomicBool, progress: &AtomicU64) -> Result<(), Cancelled>;
```

**Event order at each absolute sample p:**
1. `Reset`, if p is a segment start.
2. `Cell`, if p is a multiple of `info.cell` and not quiet.
3. Every `Trigger` at p, in order.
4. A `Run` up to the next event, the block end, `max_block` or `quiet_from`. In a quiet part it is a `Silence` instead: the driver writes +0.0 and does not call the backend.

Zero-length events at p are emitted only by the call whose block contains p. The event sequence is therefore the same for every segmentation once adjacent runs are merged.

**Latency.** Offline, output sample n comes from control time n + L, and `start` pre-rolls L samples into the scratch buffer. The plan already includes L, so a reset never cuts a delay line that still holds sound.

**`testkit`** (cargo feature `testkit`) holds:
- `ToyBackend`, which uses every hook: a pitch-driven phase accumulator, one-pole amp smoothing per sample, cell-rate brightness smoothing, hit envelopes that play slices, trigger-relative and absolute noise, a latency delay line, and a release whose tail can be declared honestly or not;
- the shared proptest strategies;
- `render_mono`, `render_blocks` and `render_from_segment`;
- `first_bit_difference`;
- `tail_overrun`, which renders the quiet parts through the backend and returns their peak;
- `CountingAlloc`, which counts alloc, dealloc and realloc. It is `unsafe impl GlobalAlloc`, commented; each test binary declares it as its `#[global_allocator]`.

render CI runs `tail_overrun < 1e-3` for every backend at its palette ranges' extremes.

### 2.16 What `takedown-midi` must do

- `encode(track, &MidiBinding) -> Smf` quantises internally and writes the result. The rules below each survive a real DAW edit:
  - MCM and RPN 0 go out at tick 0.
  - The lane is the channel range.
  - A channel is reused only after its previous note ended strictly earlier.
  - Legato is a one-tick overlap, Retrigger abuts, and Detached leaves a gap.
  - Initial expression goes before each note-on, including hit breath on CC2 and mouth as pressure on the hit's channel.
  - After that, a frame's value goes out at that frame's tick only if it changed.
- `decode(smf, rate, hint: Option<&MidiBinding>) -> Imported { track, binding }` produces anchors equal to `at`; the caller then runs `reattach_anchors`.
  - Notes pair first-in-first-out per (channel, key), and expression is looked up by tick, never by event order.
  - The phrase lane resolves monophonically by last-note priority.
  - A file without MCM is plain MIDI, with bend range ±2.
  - Coarse files are read under their own map and canonicalised under `exportable_for`.
- SMF bytes are hand-written or come from a vetted crate, decided when midi is built and recorded in LICENSES.md.

---

## 3. Property tests

proptest runs 256 cases by default and 128 for the render properties. Run a 5,000-case soak before any change to `render::contract` merges. Commit the `proptest-regressions` files. ★ marks the four tests CLAUDE.md requires.

**Shared generators** (`core/tests/common`, re-exported by `testkit`):

| Generator | Produces | Hard cases it must include |
|---|---|---|
| `std_rate` | 8, 11.025, 16, 22.05, 32, 44.1, 48, 88.2, 96, 192 kHz | 44.1 and 88.2 (non-integer samples per frame); 8 kHz (coarsest) |
| `any_rate` | uniform in [1 kHz, 1.536 MHz] | both bounds |
| `pos` | u64 positions | 0; frame boundaries ±1; up to 2^44; 10 minutes at 44.1 kHz (26,460,000) |
| `track(rate)` | valid ControlTrack | length: tiny (1 to 400 samples), typical (1 to 4 s), and one sparse 10-minute track.<br>Spans: Detached, Retrigger and Legato chains; spans under 5 ms; a span ending at `len`.<br>Hits: `dur` from 1 sample to the 60 s cap; `at` up to ±30 ms from `anchor`; two or three hits sharing an anchor; a dense roll at 23 ms spacing with more than 12 overlapping; hits at 0 and ending at `len`.<br>Up to 12 roles. Curve values include −0.0, subnormals and the range bounds |
| `seed_tree` | SeedTree | 0 to 4 region rolls; overlapping regions; regions that contain or just miss anchors (the end is exclusive); locks |
| `blocks(max)` | block lengths | random 1..=2·max; the fixed cycle 1, 7, 220, 221, 4096 |
| `toy_config` | ToyBackend settings | latency {0, 1, 7, 64}; cell {0, 7, 32}; tail {0, 4, 30}; route phrase on and off; role subsets; slice_max {0, 30 ms, unbounded} |
| `project` | Project | every float-typed persisted field, including HostAnchor's f64 fields and Params Float and Int; renders at two rates; every edit kind |
| `rederive(track)` | the same take re-derived | every anchor and boundary jittered by up to ±5 ms; a different pitch curve; two candidates inside one tolerance window; an AddHit target re-detected 1 to 20 ms away |

### 3.1 core: time (`tests/time_props.rs`)

| Test | Generator | Proves |
|---|---|---|
| ★ `sample_control_mapping_is_exact` | `any_rate`, `pos` | `frame·rate + rem == pos·200` and `rem < rate`. `control_index(frame_start(k)).0 == k` and `control_index(frame_start(k) − 1).0 == k − 1`: frame_start is the exact first sample of frame k. The mapping is monotone |
| `clock_never_drifts` | `any_rate`; start up to 2^40; 0 to 10^5 steps; one fixed 10-minute case at 44.1 kHz | `ControlClock::at(p)` advanced n times equals `ControlClock::at(p + n)` |
| `frames_covering_is_tight` | `any_rate`, sample counts | exactly the frames whose time is before n/rate; 10 minutes is 120,000 frames at both 44.1 and 48 kHz |
| `rescale_round_trips_up_then_down` | rate pairs with `to >= from`, integer and non-integer ratios; `pos` | `rescale_round(rescale_round(p, a, b), b, a) == p`; monotone; within half an output sample |
| `rescale_ceil_is_first_sample_at_or_after` | rate pairs, `pos` | the result is the first output sample at or after the exact time; `lead` is exact and in [0, 1) |
| `curve_evaluation_is_position_pure` | `track`, `std_rate` output, any start, run length | a cursor or `at_clock` started anywhere equals `value_at` at every absolute sample, bit for bit; samples on the grid return stored values |

### 3.2 core: seeds (`tests/seed_props.rs`)

| Test | Generator | Proves |
|---|---|---|
| ★ `seed_tree_independence` | `seed_tree`; random sequences of roll, lock, unlock, `roll_unlocked` and `roll_region` on other aspects | the `aspect`, `voice_seeds` and `trigger_seeds` of aspect a change only when a itself is rolled; a locked aspect survives `roll_unlocked` |
| `undo_restores` | `seed_tree`, rolls | setting a counter back restores the exact seed |
| `region_rolls_are_local` | `seed_tree` with regions; anchors inside, on the end boundary and just outside | `trigger_seeds` change only for anchors inside a region of that aspect; `voice_seeds` never change; the order in which regions were added is irrelevant |
| `trigger_seeds_key_on_anchor` | events; insert or remove other events | trigger seeds depend only on (voice, lane, anchor, ordinal), never on an index |
| `same_anchor_events_decorrelate` | 2 or 3 hits sharing an anchor; 2 voices | seeds are distinct per ordinal and per voice |
| `noise_is_counter_based` | seed, stream, random chunkings | random access equals sequential reading under any chunking; values in range; mean and lag-1 correlation within bounds over 10^6 samples |
| `derive_separates_labels_and_indices` | (label, idx) pairs, including shifted label boundaries | 10,000 derived children with no collision |
| `frozen_seeds_and_noise` | fixture | the pinned derivation and noise values are unchanged |

### 3.3 core: keys and persistence (`tests/key_props.rs`, `tests/persistence_props.rs`)

| Test | Generator | Proves |
|---|---|---|
| `keys_ignore_identity_and_locks` | `project`; mutate id, name, slider and aspect locks, edit ids, palette name, MidiBinding | InterpKey is unchanged. Mutating any slider value, the palette id, the root, a non-zero roll, a region, an edit op, or the evidence or hits key changes it |
| `key_encoding_rules` | maps, sets, integers of every width, floats, strings | map and set insertion order and integer width do not change keys; −0.0 equals 0.0; NaN and infinities are rejected; strings with shifted boundaries do not collide; variants are encoded by name |
| `reproducing_values_keep_keys` | a test-only v2 struct with one extra field | at its reproducing value the key equals v1's; any other value changes it |
| `json_round_trip_preserves_keys` | `project` | `project.json` text never perturbs InterpKey, RenderKey or EvidenceKey |
| `project_round_trip_is_byte_stable` | `project` | save, load, save is byte-identical |
| `f32_survives_json` | 10^6 sampled bit patterns in CI; an `#[ignore]` test covers all 2^32 | every finite f32 round-trips exactly through serde_json, directly and inside Params |
| `frozen_keys` | fixture | one pinned key of each kind is unchanged |
| `unknown_fields_refused` | fixtures with an extra field at each nesting level; an unknown edit kind | loading fails, and nothing is truncated (J3 regression) |
| `newer_and_missing_schema_refused` | fixtures | `TooNew`; `MissingVersion`, never read as 0 |
| `migration_chain` | test-only steps: v1→v2 additive and v2→v3 rename, with renders kept; a step that forgets a field; a gap | the v1 fixture loads into today's types. After the additive step its renders are Fresh; after the rename they are `Stale(InterpretationChanged)` and still playable. A forgotten field fails through deny_unknown_fields. A gap is an error. The production chain is total |
| `default_change_keeps_meaning_and_key` | a test-only field added at v2, with reproducing value 0.0 and new-project default 0.25 | a v1 file loads with the 0.0 the step wrote, and its key equals v1's; changing the new-project default changes neither (J3 regression) |
| `evidence_container_round_trip` | random evidence including −0.0, subnormals and missing columns; single flipped bytes | bit-exact round trip; any flipped byte is detected; a wrong expected key is never substituted; an illegal length is refused |
| `evidence_resolution_rules` | pinned and running provenances | `UseCached`, `RecomputeSame` and `NeedsUpgrade` exactly as in 2.5; params order is irrelevant |
| `take_id_chunk_invariant` | random PCM, chunkings, rates, channel counts | the id is independent of chunking and depends on rate and channels; NaN is rejected |
| `sample_rate_change_mid_project` | scenario: 48 kHz takes, output switched to 44.1 kHz, then a 96 kHz take added | no stored position, edit, InterpKey or 48 kHz render changes; a 44.1 kHz slot is added; switching back costs nothing; it all survives save and reload |
| `render_key_tracks_running_versions` | RenderInputs; vary the lens version, a backend stamp, the contract, evidence content, the rate | each change gives a new RenderKey and the matching StaleReason; the platform class changes nothing (J3 regression) |
| `render_files_are_content_addressed` | two renders differing only in lens version | distinct WavIds, both kept outside `cache/`; deleting `cache/` loses no render (J3 regression) |
| `atomic_save_and_backup` | temp directory | no temp file is left behind; the backup is written before the first save over a migrated file |
| `identical_pcm_dedupes` | the same PCM added twice under two names | `Existing`; the first record is kept |

### 3.4 core: control, views, canonical (`tests/control_props.rs`, `tests/canonical_props.rs`, `tests/no_alloc.rs`)

| Test | Generator | Proves |
|---|---|---|
| `control_track_json_roundtrip_is_exact` | `track` | bit-exact through JSON; `content_id` is stable |
| `invalid_tracks_are_refused` | each invariant violated once: NaN, an out-of-range curve, a length mismatch, an empty, overlapping or unsorted span, a wrong link, `off > len`, `dur` of 0, `dur` over 60 s, `at + dur` overflowing u64, unsorted hits, a role index out of range, unsorted roles | refused with the matching TrackError by `new` and on deserialisation, never a panic |
| `events_are_canonical` | `track` | events are sorted; phrase events precede hits at equal `at`; NoteOn, Retrigger or nothing according to the link; NoteOff only before a gap or at the end; every span is opened and closed exactly once |
| `gate_and_notes_agree` | `track`, plus fixed regressions: J2's gated hum and C's retriggered phrase | `note_view` covers exactly the union of spans, and the total note length equals the total span length |
| `boundaries_ignore_curves` | `track`, replacement curves | no note's on, off, vel, link or frames change |
| `keys_stable_under_small_perturbation` | `track`; perturbations smaller than each median's distance to a half-semitone (cases near an edge are counted, not skipped silently) | every key is unchanged |
| `quantise_is_idempotent` | `track`; MidiBinding with bend ranges 1 to 96, channel splits, PPQ 24 to 1920 refined to exportable, 1 to 3 tempo segments, any origin | `quantise(quantise(t)) == quantise(t)` bitwise |
| `quantisation_error_is_bounded` | same | boundaries move at most ½ tick; covered values move at most the table's bound and lie on the lattice; anchors and take are unchanged |
| `quantisation_never_flips_keys` | same | the keys of `quantise(t)` equal the keys of `t` when times are on the lattice |
| `links_preserved_or_downgraded` | same | Legato is never invented; links between spans at least a tick long survive; a sub-tick gap may only become Retrigger |
| `time_map_exactness` | exportable maps with tempo changes and any origin, at every `std_rate` | `tick_of_sample(sample_of_tick(t)) == t`; distinct frames get strictly increasing ticks; monotone; coarse maps refine by an integer factor and keep every instant |
| `reattach_restores_anchors` | `track` with `at ≠ anchor`; reference = `quantise(track)`; anchors stripped | every anchor is restored exactly; a hit moved by up to `tol` keeps its anchor; with no reference, anchors fall back to onsets, then to `at` |
| `views_and_events_do_not_allocate` | `track`; counting allocator | `value_at`, `at_clock` and `events()` make no allocations |

### 3.5 core: edits (`tests/edit_props.rs`)

| Test | Generator | Proves |
|---|---|---|
| `edits_survive_rederivation` | `track`, edits of every kind, `rederive(track)` | every edit has the same outcome; the results agree up to the jitter; SplitAt boundaries agree exactly |
| `edits_ignore_index_shifts` | `track`, edits, unrelated hits inserted anywhere | no outcome and no edited hit changes |
| `add_hit_merges_with_rederived_hit` | AddHit plus a re-detected hit 1 to 20 ms away with any role; J2's 48000/48144 case | exactly one hit near the anchor, carrying the edit's role; the outcome is `Merged` |
| `split_semantics` | spans, split positions | splitting twice equals splitting once; a split within 20 ms of a boundary moves it; a piece under 10 ms gives `Dormant(TooShort)`; a split in silence gives `Dormant(NoTarget)` (J2 regression on C) |
| `dormant_edit_resurrects` | an edit whose target vanishes and returns | reported dormant, then applied again once a target is back within tolerance |
| `mute_span_silences` | `track`, spans to mute | no span, hit anchor or hit tail remains inside the span |
| `rebase_up_and_down_is_identity` | edits, rate pairs with b ≥ a | rebasing from a to b and back gives the original edits |

### 3.6 render: contract (`crates/render/tests/contract_*.rs`, testkit feature)

| Test | Generator | Proves |
|---|---|---|
| ★ `block_size_invariance` | `track` with `std_rate` timeline and output rates (unequal included); `toy_config`; two voices with different routes; two random block sequences plus the fixed cycle; one sparse 10-minute case | the monolithic render and both segmentations are bit-identical |
| `event_sequence_invariance` | same, any `max_run` | the driver's event sequence is identical once runs are merged |
| ★ `chunked_equals_monolithic` | same | a fresh backend started at every segment start, fed random blocks, reproduces the monolithic suffix bit for bit |
| `segment_plan_is_sound` | `track` at every `std_rate`, `toy_config` | starts strictly increase from 0; quiet parts last at least 50 ms; no trigger falls in a quiet part; every trigger span plus tail and latency ends before its quiet part; every slice ends inside its event's activity |
| `latency_compensation_is_a_pure_shift` | latency 1 to 199 | compensated output equals raw output shifted by L; J1's case sounds at sample 10000, not 10064, for all 480 samples |
| `release_survives_to_declared_tail` | J1/J3 case: a 100 ms release, then a 60 ms gap; random variants | no reset falls inside a declared release |
| `region_reroll_locality` | `track`, `seed_tree`, one added region roll | output is bit-identical before the first trigger anchored in the region and in every segment that contains no such trigger. Rolling Mangle or a region changes the output of a voice that uses those seeds (J1 regression on B) |
| `inserting_a_hit_keeps_other_seeds` | J1's case on A (an insertion that merges segments) plus random | existing triggers keep their seeds; output changes only from the new hit's segment start to the first reset point both plans share |
| `moving_a_trigger_shifts_its_output` | hit-only toy with voice-level noise off, timeline rate = output rate, one isolated hit moved by d with the same anchor | in its segment the output is the original shifted by d (J2 regression: a 1-sample move had changed 2000 of 2000 samples) |
| `hit_voices_never_read_curves` | `route.phrase == false` | `cursor()` is `None`; output is unchanged when every curve is replaced |
| `tail_overrun_flags_dishonest_tails` | toy with honest and under-declared tails | overrun below 1e-3 for the honest tail, above it for the dishonest one |
| `process_path_never_allocates` | `toy_config`; blocks 1..=max_block; starts at every segment; triggers with slices; cells; latency pre-roll | no alloc, dealloc or realloc during `start` and `process`; the counter checks itself |
| `render_offline_matches_and_cancels` | `track`, `toy_config` | equals the testkit loop; reports progress; cancels; works through `dyn RenderBackend` |
| `generators_are_not_degenerate` | the strategies above | more than ¾ of renders are audible, more than ⅓ have several segments, more than ½ have triggers |

Before merging the contract, mutation-check the suite. Each of these deliberate bugs must be caught:
- the toy breaks the split law;
- cells are counted from block starts;
- the plan ignores latency;
- seeds are keyed by index;
- region rolls are folded into voice seeds;
- the driver skips a reset;
- trigger noise is indexed by absolute sample.

### 3.7 midi: round trip (when the midi crate is built, weeks 3–4)

| Test | Generator | Proves |
|---|---|---|
| `decode_of_encode_is_quantise` | `track` at 7 rates; bindings as in 3.4; more than 12 overlapping hits; unmapped roles; pitch beyond ±48 | `decode(encode(t))`, after `reattach_anchors` with `reference = quantise(t)`, equals `quantise(t)` bitwise |
| `encode_is_a_fixed_point` | same | export, import, export reproduces the first export event for event |
| `any_file_stabilises_after_one_import` | arbitrary SMF: random channels, orphan note-offs, overlaps, stray RPNs, 24 to 960 PPQ, tempo changes, no MCM | `decode(encode(decode(f))) == decode(f)`, for both track and map |
| `same_tick_reordering_is_harmless` | any permutation of non-setup events within a tick | decodes to a bit-identical track |
| `channel_permutation_within_lane_is_harmless` | channels rotated within each lane | decodes to a bit-identical track |
| `imported_midi_renders_identically` | `track`, the interpretation's own SeedTree (never a fixed seed); render dev-depends on midi | `render(decode(encode(t))) == render(quantise(t))` bitwise |
| `nudged_hit_keeps_its_sound` | a hit moved by up to `tol` in the file | after import, its anchor, seeds and slice are unchanged |
| `split_survives_midi_round_trip` | user SplitAt edits | still a boundary within ½ tick, with the same articulation |

---

## 4. Breaking scenarios

No judge found a fatal break. Every major break, the attempts it broke, and how the final design closes it:

| # | Scenario (judge) | Broke | Final design |
|---|---|---|---|
| 1 | Backend declares 64 samples of latency; the hit sounds 64 samples late and only 416 of its 480 samples survive (J1) | B, C | The driver pre-rolls L offline, and the plan extends activity by L. `latency_compensation_is_a_pure_shift` |
| 2 | A reset at the midpoint of a 60 ms silent run cuts an honest 100 ms release (peak 0.329, then 0) (J1, J3) | C | `BackendInfo.tail_frames`; resets go at the start of the next activity after at least 50 ms of quiet beyond tail and latency; `tail_overrun` audits every backend. `release_survives_to_declared_tail` |
| 3 | Only a Body-derived seed reaches the backend, so Mangle rolls and region rolls are inaudible and rolling Body moves the mangle randomness (J1) | B, C | `VoiceSeeds { body, mangle }` at reset, plus per-trigger seeds with region rolls folded in by anchor. `seed_tree_independence`, `region_reroll_locality` through a render |
| 4 | Hit identity is its position: a timing-feel roll or a one-tick Reaper nudge re-rolls the body (2000 of 2000 samples differ), and the exciter slice misses the transient (J1, J2) | B, C | `anchor` on hits and spans; seeds and slices derive from it; trigger noise is indexed from `at`. `moving_a_trigger_shifts_its_output`, `nudged_hit_keeps_its_sound` |
| 5 | Gate curve and NoteOn/NoteOff disagree: a 1 s audible hum exports zero notes, and a NoteOn under gate 0 is reset mid-note (J2) | A, C | No gate curve: the gate is spans with links, and activity and NoteView both come from the spans. `gate_and_notes_agree` |
| 6 | NoteView drops retriggered segments, covering 48,000 of 95,000 sounding samples (J2) | C | One note per span. `gate_and_notes_agree` regression |
| 7 | No lossy projection exists, and the track holds things MIDI cannot carry (anchor, slice ref, voice slot, f32 hit fields, NoteOn pitch) (J2) | A, C | `ControlTrack::quantise` in core; anchors re-attached on import; slices derived; voices route by role; 7-bit hit fields; gate events without pitch. Round-trip tests in 3.4 and 3.7 |
| 8 | Region rolls widened to whole segments re-roll every hit of a dense take; an insertion that merges segments re-rolls an unrelated hit (J2 major, J1 minor) | A | Region rolls fold only into per-trigger seeds, by anchor; voice seeds never fold them. `region_rolls_are_local`, `inserting_a_hit_keeps_other_seeds` |
| 9 | Edit application is undefined: no apply, no snapping, no dormancy; a SplitAt in silence is reported as applied (J2) | A, C | `apply_edits` with Outcome and Dormancy, 20 ms split snapping and a 10 ms minimum piece. `split_semantics`, `edits_survive_rederivation` |
| 10 | AddHit doubles a re-derived hit 3 ms away, giving a flam and a doubled MIDI note (J2) | B | AddHit carries a HitAnchor, and a hit within tolerance takes the role instead (`Merged`). `add_hit_merges_with_rederived_hit` |
| 11 | `midi_exact` lets the 12-channel hit lane choke audio decays: a 400 ms hat renders 280 ms (J2) | B | The renderer plays the full-resolution track; choking exists only in the `.mid` and is reported. `midi_exact` is deferred and, when added, excludes the choke |
| 12 | Dice are lost on `.mid` import, so every hit re-rolls after a one-note edit (J2) | B (and C) | SeedTree and LensSettings sit on the Interpretation for both sources. `imported_midi_renders_identically` uses the interpretation's own tree |
| 13 | An older build opens newer lens settings and silently drops the new field on save (J3) | A, B | deny_unknown_fields on every persisted struct, no per-struct versions, a schema bump on every on-disk change, and TooNew. `unknown_fields_refused`, `newer_and_missing_schema_refused` |
| 14 | Durable renders sit in a deletable `cache/`, and render paths collide across lens and backend versions (J3) | A, B | `renders/<WavId>.wav` outside `cache/`, and RenderRef holds its full inputs. `render_files_are_content_addressed` |
| 15 | RenderKey omits the running lens version, so a stale render is reported fresh (J3) | B | RenderInputs record the running lens version, backend stamps, contract version, and evidence and hits content. `render_key_tracks_running_versions` |
| 16 | Changing a skipped default silently changes meaning under an unchanged key (J3) | C | Every field is written to disk; keys omit only frozen reproducing values; new fields arrive by migration. `default_change_keeps_meaning_and_key` |

Minor breaks, fixed or accepted:

| Minor break (judge) | Broke | Resolution |
|---|---|---|
| An exciter outlives a short hit and is hard-cut (J1) | A | A slice never outlives its event |
| Triggers sharing an anchor share noise and sum coherently (J1) | A | Ordinal in trigger seeds |
| A backend can play NoteOn's pitch (J1, J2) | A, C | Gate events carry no pitch |
| A republished track resets the backend mid-phrase (J1) | A | Deferred with seek and swap; a swap will happen only at a reset point |
| Live seek, swap and latency reporting machinery in v0.1 (J1, J2, J3) | A | Deferred to v0.2 (§6) |
| Take-rate and render-rate positions mix without a compile error (J1) | B | `TimelinePos` and `RenderPos` |
| A hit's `at + dur` overflows: a panic in debug, accepted in release (J1) | B, C | `checked_add` and a 60 s cap |
| `midi_exact` brings 7-bit CC steps into the audio (J1) | B | Deferred; the renderer plays full resolution |
| A sample rate of 8 Hz is accepted (J1) | B | 1 kHz minimum |
| Validation accepts nonsense values (J1) | C | Validated ControlTrack |
| Every backend scans triggers and aligns cells itself; the driver calls `prepare` (J1) | B, C | The driver owns event order; `prepare` stays outside it |
| Exciters have no path to take audio (J1) | C | `SliceSource` and `SliceBank` at bind time |
| Two voices share one pitch curve, so per-note bends conflict (J2) | A | One monophonic phrase lane |
| A hits-only take loses its curves under quantise (J2) | B | Hit fields are self-contained (breath added); the renderer plays the unquantised track |
| Region rolls have no path to the renderer (J2) | B | Trigger seeds |
| Serum plays a one-tick Legato overlap as a retrigger; a tail glides to the next note's preloaded pitch (J2) | B | Documented; CC68 is an extension point |
| Hashes over serde_json text include edit ids (J2, J3) | A, B | KeyWriter; ids excluded |
| Edit order is part of the key (J2) | C | Accepted: order is meaning |
| An f32 preset widened into `serde_json::Value` drifts after one save (J3) | A, B | Typed Params; float_roundtrip |
| Adding a field re-keys every render (J3) | A, B | Reproducing values are omitted from keys |
| A serde_json printing change silently re-keys everything (J3) | A, B | Bit-based encoding and frozen fixtures |
| RenderKey omits the evidence actually used (J3) | A | Evidence and hits content in RenderInputs |
| 10-minute evidence as 28 MB of JSON (J3) | A, B | `.tkev` container |
| A missing migration step panics; a missing schema is read as 0 (J3) | A, B | `chain_is_total`; `MissingVersion` |
| Toggling a lock re-keys the render (J3) | A, B, C | Locks are outside keys |
| One render slot per layer (J3) | A, B | One slot per rate |
| ForceRole stores a role index (J3) | A, B | `RoleName` |
| The take id needs the whole take in memory (J3) | A, B | `TakeHasher` |
| Platform class in the key marks portable WAVs stale (J3) | B | Provenance only |
| f64 grid and host-anchor values drift through JSON (J3) | C | No f64 in keyed state; float_roundtrip |
| Widening an f32 field to f64 changes keys (J3) | C | Widening is a rename plus a migration |
| An integer preset parameter is rounded through f32 (J3) | C | `ParamValue::Int` |
| Takes with identical PCM collide (J3) | C | Accepted: deduplicated to one take (§7) |
| A MIDI-sourced layer cannot be saved (J3) | C | `ControlSource::Imported` |
| The migration test strips renders (J3) | C | `migration_chain` keeps them |
| A newer project cannot be opened read-only (J3) | C | Deferred (§6) |
| `render_segmented` allocates (J3) | C | The plan is precomputed at bind time |
| `Provenance.params` would hold a user's range calibration (J3) | C | Fixed analyser constants only |
| A home-grown 428-line serde encoder (J1, J2, J3) | C | A small KeyWriter with exhaustive destructuring |

---

## 5. Grafts and rejected alternatives

| From | Grafted |
|---|---|
| A (base) | the time base: `TimelinePos` and `RenderPos`, the integer `ControlClock`, exact u128 rescaling with `lead`; BoundTrack, the segment plan (tail, latency, slack frame, reset at the start of activity), the driver's event order, latency pre-roll; the SeedTree on the Interpretation; anchor separate from `at`; `VoiceSeeds`; the exciter bank, now `SliceBank`; `ControlSource::Imported` with a stored track; validated-only ControlTrack; testkit (`tail_overrun`, counting allocator, non-degeneracy guard) |
| B | gate spans with Detached, Retrigger and Legato links; gate events without pitch; Legato emits no trigger; `U7` and `Velocity` hit fields; `quantise` with its lattice and semitone-preserving snap; NoteView keys from the lower median; integer TempoMap and TimeMap with `exportable_for`; MpeProfile with lanes as channel ranges and its encoding rules; `apply_edits` with Outcome and Dormancy, split snapping, minimum piece, JoinAt, `rebased`; slices clamped to the event; gate events before hits at ties |
| C | the canonical key rules (floats by bits, integer width ignored, names sorted, variants by name, None omitted, frozen fixtures); EvidenceKey kept apart from content hash, `resolve_evidence`; the `.tkev` container; `TakeHasher`; RenderInputs with contract version and evidence content; one render slot per rate, `StaleReason`; durable directories outside `cache/`; migrations on untyped JSON, deny_unknown_fields, `chain_is_total`, frozen fixtures, TooNew, atomic save with backup; roles and teach labels by name and time; palettes by content hash; hex seeds; typed Params; project-counter ids |

| Rejected | From | Why |
|---|---|---|
| Live seek, `provisional_until`, track swap, `LatencyMode::Report` in v0.1 | A | v0.1 renders offline and plays WAVs. All three judges called it v0.2 weight |
| A continuous gate curve | A, C | A second source of truth that lets audio and MIDI disagree (scenarios 5, 6) |
| `NoteOn { pitch_st }` | A, C | Lets a backend become a note player (hard rule 1) |
| Region rolls folded into segment seeds | A | Re-rolls hits outside the region (scenario 8) |
| A per-struct `LensSettings.version` | A, B | Invites silent truncation (scenario 13). One version per document |
| Content hashes over serde_json text | A, B | Hash depends on float printing and preserve_order, and includes ids |
| Render files under `cache/`, named by interp hash | A, B | Durable artefacts became deletable and collided (scenario 14) |
| `midi_exact` on by default | B | Quantises the audio and chokes decays (scenario 11; J1 minor) |
| `TimeWarp` and HostAnchor-to-TimeMap in v0.1 | B | Nothing uses them before the notes lens |
| The MPE codec in core | B | MASTER-PLAN §4.3 puts it in midi; core keeps only `quantise` |
| One backend seed derived from Body | B | Mangle and region rolls inaudible (scenario 3) |
| Dice inside LensSettings | B, C | Lost on import (scenario 12) |
| AddHit deduplicated only on an exact (at, role) match | B | Doubles re-derived hits (scenario 10) |
| The `.mid` as the durable source, decoded on every load | B | Makes renders depend on codec versions; storing the decoded track at `controls/` and keeping the file as provenance avoids that |
| A reset at the midpoint of the silent run | C | Cuts tails (scenario 2) |
| A generic serde Serializer for keys | C | 428 lines whose design bugs become key-scope bugs. A small KeyWriter with compile-time exhaustiveness is smaller and makes scope visible |
| `skip_serializing_if` on disk | C | The default-change trap (scenario 16) |
| `RenderBackend` and the driver in core | A, B, C | MASTER-PLAN §4.3 places them in render. `render::contract` depends only on core and can be split out later |
| A take record id separate from the audio content id | J3 suggestion | The plan defines the take id as the PCM hash; identical PCM deduplicates (§7) |
| A `Canonical<ControlTrack>` newtype | B weakness | `encode` quantises internally, so there is nothing to guard |

---

## 6. Deferred

| Item | Lands | Extension point that keeps it cheap |
|---|---|---|
| Live seek, track swap at a reset point, latency reporting to the host, provisional output | v0.2 plugin | `EventCursor::new(at)` is already a binary-search seek; `Driver` gains `seek` and a `LatencyMode`; `BoundTrack::id` detects a swap |
| Backend snapshot and restore, for partial re-render of drones with no 50 ms gap | when a drone needs it | default methods on `RenderBackend`; the driver starts from a snapshot instead of a reset point |
| Stereo voices | revisit before the third backend lands | `BackendInfo.channels` and a channel-slice `run`; the split law is unchanged |
| `midi_exact` (render `quantise(t)`) | when the founder asks | an additive LensSettings field; render calls `quantise` minus the choke |
| Quantise, notes lens, TimeWarp, musical time from HostAnchor | v0.2 `notes` | additive LensSettings field by migration; TimeMap already exact; `HostAnchor` already stored |
| Region reroll UI | v0.3 | `SeedTree.regions` and `trigger_seeds` exist and are tested |
| Edits on Imported sources; "commit to notes" | v0.2 | `ControlSource::Imported` already holds a fixed track; edits apply to `TrackParts` |
| Per-voice curves | if two continuous voices need different pitch | a ControlTrack format bump with a migration |
| Binary imported-track format | if 10-minute imports matter | `ControlFile.schema_version` |
| Per-column evidence provenance | when analyser upgrades become frequent | the container lists columns by name; container version 2 |
| Read-only open of newer projects | v0.2 | `TooNew` carries the found version |
| CC68 legato, 14-bit expression | when a DAW edit breaks legato | `MpeProfile.version` |
| Undo-history-aware render GC, lock file against two instances | app | renders are content-addressed and `project.json` holds no paths |

---

## 7. Assumptions the founder did not decide

Each is one DECISIONS.md entry.

1. Attempt A is the base design, and B's track shape, MIDI projection and edit semantics plus C's persistence rules are grafted onto it, because A had the fewest unresolved major breaks and none in its render contract.
2. `RenderBackend`, `BoundTrack` and the driver live in `takedown-render::contract`, as MASTER-PLAN §4.3 places them, depending on core only so they can become a crate later; all three attempts had put them in core.
3. The plan's `process(ctl, at, out)` and `reset(seed, at)` become driver-called hooks `reset(track, seeds, at)`, `cell`, `trigger` and `run`, so block-size invariance, reset placement and latency compensation are guaranteed once by the driver instead of by every backend.
4. ControlTrack has no gate curve: the gate is a monophonic list of spans with Detached, Retrigger and Legato links, which matches monophonic voice input and makes gate and NoteOn/NoteOff one source of truth.
5. NoteOn and Retrigger carry no pitch; NoteView derives each note's key from the pitch curve as the rounded lower median.
6. Hits and spans carry an `anchor` (the evidence onset, their identity) separate from `at` (firing time), and seeds, take slices and edit targets key on the anchor.
7. Exciter and voice-material slices are derived from the anchor, clamped to the event's duration and to a per-backend maximum (30 ms for exciters), so `Hit` stores no slice reference and MIDI need not carry one.
8. Hit voices route by role, with no voice slot in the track; hit voices never read curves, so each hit carries its own 7-bit velocity, brightness, breath and mouth.
9. The renderer plays the lens's full-resolution track and the `.mid` is its `quantise`, so audio and MIDI agree within quantisation; `midi_exact` is deferred.
10. `quantise` lives in core and the SMF/MPE codec in `takedown-midi`.
11. The default export map is 960 PPQ at 120 BPM, and until HostAnchor arrives in v0.2 the founder sets the export tempo to the Reaper project's tempo.
12. The SeedTree and LensSettings sit on the Interpretation for both Lens and Imported sources, and MidiBinding stays outside the interpretation key.
13. Region rolls fold only into per-event seeds by anchor; voice seeds are constant per voice.
14. Per-trigger noise is indexed from the trigger's `at`, voice-level noise from the absolute sample.
15. An imported `.mid` is stored decoded at `controls/<id>.json` with anchors re-attached, the original file is kept at `midi/<id>.mid`, and only Lens sources take edits in v0.1.
16. The project directory adds durable `renders/`, `controls/`, `midi/`, `palettes/` and `teach/` alongside the plan's `audio/` and `cache/`; only `cache/` is deletable.
17. LensSettings is versioned by the project schema version, not by its own version field.
18. Every persisted field is written explicitly, every persisted struct denies unknown fields, every on-disk change bumps the schema, and newer files are refused.
19. Keys omit only frozen reproducing values, and new fields arrive through a migration that writes that value into old files.
20. Keys come from a small hand-written canonical encoder with exhaustive destructuring, never from JSON text or a generic serde serialiser.
21. serde_json's `float_roundtrip` feature is enabled workspace-wide and keyed state holds no f64.
22. RenderKey is built from the running lens, backend and contract versions plus the evidence and hits content actually used; platform class is provenance outside the key; each layer keeps one render per output rate.
23. Ids, names, slider and aspect locks, edit ids and the palette name are outside keys; edit order is inside.
24. Two takes with identical PCM are one take; the first record's metadata is kept.
25. Accepted sample rates are 1 kHz to 1.536 MHz.
26. Voices and layer renders are mono in v0.1.
27. AddHit merges with any hit within its tolerance by forcing that hit's role instead of adding a second hit.
28. Edits apply to the lens draft before timing feel and perceptual-centre pre-roll move `at`, with a 25 ms default tolerance, 20 ms split snapping and a 10 ms minimum piece.
29. Curves floor amplitude at −120 dB, while the MIDI pressure lattice floors at −63.5 dB inside notes.
30. Local ids are project counters that are never reused, and seed roots and project ids are supplied by the app.
31. Evidence provenance params hold fixed analyser constants only; a user's range calibration is a lens parameter.
32. Evidence columns are optional per analyser, so analyser v1 may omit vowel and class hints.
33. Role names are `[a-z0-9_]{1,32}` and the default role-to-key map is GM drums, with `perc` as the fallback.
