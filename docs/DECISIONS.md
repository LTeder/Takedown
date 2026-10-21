# Decisions

Append-only log. One entry per decision that is not obvious from the code. Newest at the bottom. Each entry: date, decision, why, what it rules out.

## 2026-10-20: Rust, GPL-3.0-or-later, Reaper-first
Chosen over JUCE/C++ and over permissive licensing. Surge XT is a reference oracle only (GPL-3.0-only would pin the project). Reaper is the sole test host until v0.2.

## 2026-10-20: Percussive and noisy first; tonal is expression, not transcription
The founder plays piano. Voice-to-clean-notes is not a goal. Target palette: SOPHIE, Death Grips, JPEGMAFIA, Aphex Twin.

## 2026-10-20: Minimal standalone first, plugin in v0.2
The nudge-and-reroll loop needs a live window. No looper, layers, piano roll or Reaper companion in v0.1.

## 2026-10-20: The renderer never sees notes
Backends take a ControlTrack only. MIDI is a serialisation of the same object and must round-trip.

## 2026-10-20: Mangle is synthesis, polish is the DAW's
Per-voice drive, fold, crush, filter scream, pitch envelope, transient shaping, ring-mod live in the engine. Reverb, delay, compression, OTT, EQ stay in Reaper. No global effects bus.

## 2026-10-20: Training-data policy is pragmatic
Prefer clean, redistributable sets and record provenance; pretrained checkpoints are not forbidden. See LICENSES.md.

## 2026-10-20: Research docs live under docs/research and are expected to rot
They are planning artifacts. CLAUDE.md, MASTER-PLAN.md, LICENSES.md and this file are the maintained rules. The master plan is expected to shrink into ARCHITECTURE.md and ROADMAP.md as code lands.
## 2026-10-20: Workspace lint and toolchain baseline
Edition 2024, MSRV 1.90, stable toolchain via `rust-toolchain.toml`. Clippy `all` + `pedantic` at warn in `[workspace.lints]`, with the numeric-cast and doc lints that fight DSP code allowed; CI will run `-D warnings` so warn locally means fail in CI. `unsafe_code` is warn, not forbid, so a justified `unsafe` block with a comment stays possible. `cargo test` profile uses opt-level 2 so DSP and property tests run at useful speed. Rules out per-crate lint drift.

## 2026-10-20: Research environment pinned to Python 3.13 via uv
`research/` is a non-package uv project (`.python-version` 3.13, `uv.lock` committed, mir_eval pinned to 0.8.2 exactly, numpy and scipy locked by `uv.lock`). 3.13 rather than 3.14 for wheel availability across the scientific stack. Python is an evaluation tool only; nothing in the shipped binary depends on it.

## 2026-10-20: Dataset fetch pins Zenodo MD5 plus our own SHA-256
`research/fetch_data.py` is standard-library only and verifies size, the MD5 Zenodo publishes and a SHA-256 recorded on first fetch (2026-10-20). A changed upstream file fails loudly instead of silently changing eval numbers. Archives are kept under `research/data/archives/` so re-extraction needs no network; each dataset unpacks into its own directory with path-escape checks. Both datasets are 44.1 kHz, which exercises the non-integer 220.5-samples-per-control-frame case from day one.

## 2026-10-21: Core types from a three-way design panel
`docs/design/core-types.md` holds the result and its reasoning. Three independent designs (renderer, round-trip and persistence lenses) were attacked by three judges with concrete breaking scenarios; no fatal breaks, and attempt A had the fewest major ones (6 against 7 and 8), none in its render contract. A is the base, with B's track shape, MIDI projection and edit semantics and C's persistence rules grafted on. The assumptions the panel made, each a decision the founder did not take:

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

## 2026-10-21: MIDI projection deferred to the midi step
`ControlTrack::quantise`, the MPE wire lattice (`pitch_to_wire` and friends) and `reattach_anchors`, with their property tests (design §3.4 quantise and reattach rows, all of §3.7), land with the `midi` crate in weeks 3 to 4, where they are first used. Their types (TempoMap, TimeMap, RoleMap, MpeProfile, MidiBinding, QuantiseReport) land now because `Interpretation` persists a `MidiBinding`. Nothing in v0.1 steps 3 to 6 exports MIDI, and a schema bump is cheap before the first real project exists.

## 2026-10-21: Edit application calls the spec left open
Made while implementing `apply_edits` (`crates/core/src/edit.rs` documents each):
1. When several hits share the nearest anchor, a hit edit targets only the first in canonical order.
2. ForcePitch writes the frames whose first sample lies in the span (the NoteView rule), not frames whose centre time does; the latter can miss every frame of a short note at 44.1 kHz. `st` is clamped to [-64, 192]; a non-finite `st` is `Dormant(OutOfRange)`.
3. ForceRole is `NoOp` when the hit already has the role. ForceRole and AddHit are `Dormant(OutOfRange)` when a new role would exceed `MAX_ROLES`.
4. AddHit tries to merge before the range check, so an AddHit just past the end still merges with a hit inside its tolerance. Merging into a hit that already has the role is `NoOp`.
5. MuteSpan also removes hits that fire inside the span (differs from "anchored inside" only once timing feel moved `at`). Splitting a straddling span anchors the second piece at the mute's end; a span clipped only at its start keeps its anchor.
6. SplitAt in silence is `NoTarget` even with a boundary within 20 ms; a snap that would leave a piece under 10 ms is `TooShort` and does not fall back to a fresh split; a boundary already at `at` counts as `Applied` so outcomes stay stable under re-derivation.
7. ForcePitch and MuteSpan are `Applied` whenever their span overlaps the track, even if nothing changes.

## 2026-10-21: Counter noise derives its second mix from mix(a), not a ^ stream
`NoiseStream` value(n) = mix(mix(n + a) ^ b) is 0 exactly at n = mix⁻¹(b) - a. With b = mix(a ^ stream) that index sat within `stream` of 0, so `seed.noise(0).u64_at(0)` was 0 for every seed (the render agent's toy caught it as a constant Mangle draw). b is now mix(mix(a) ^ stream ^ salt), which moves the single zero to a seed-dependent index near 2^63. The frozen noise fixture was re-pinned before anything was persisted; `noise_has_no_fixed_zero` guards it.

## 2026-10-21: Render contract calls the spec left open
1. Trigger ordinals count every track event with the same (lane, anchor), routed to the voice or not, so routing never moves a seed.
2. When the first activity starts after 10 or more quiet frames, the plan's first segment is empty ((0, 0)); a voice with no activity has `end = 0`.
3. `SliceSource::read` returns output samples from `rescale_ceil(start)` to `rescale_ceil(end)`; the bank reads each merged union once; empty or unreadable slices are `None`; one slice id per trigger.
4. `render_offline` publishes the absolute output position as progress and checks `cancel` before every block, the first included.
5. `Driver::start` clamps an out-of-range segment index; `process` before `start` writes zeros.
6. The render test generator reshapes the shared track plan (one fixed sparse 10-minute case instead of random ones, fewer tiny tracks and end-running hits) so that over a third of renders have several active segments; the raw generator gave 17%.
7. `testkit` is a cargo feature turned on for render's own tests by a self dev-dependency, so `cargo test -p takedown-render` needs no flags.

## 2026-10-21: Evidence may hold NaN in four columns
The DSP analyser reports undefined values (spectral shape of silence, harmonic units with no pitch candidate) as NaN. Core's `Evidence` accepts NaN in CentroidHz, BrightnessHarm, TiltDbOct and CppDb only; every other column must be finite and infinities are always refused. `Evidence::new` canonicalises NaN to the explicit bit pattern 0x7FC0_0000 (Rust does not promise `f32::NAN`'s bits across versions or targets), so content hashes stay deterministic. This deviates from design §2.5, which required all values finite.

## 2026-10-21: All JSON Takedown writes goes through `core::project::json`
An exhaustive check of all 2^32 f32 bit patterns found two (±7.038531e-26) whose shortest decimal lies exactly between two f32s when read as f64; serde reads f32 through f64 inside tagged and untagged enums (`Edit::ForcePitch.st`, `ParamValue::Float`) and `serde_json::Value`, so they came back as the neighbouring float and changed an InterpKey after reload. `project::json` writes those two values with a longer exact decimal and is otherwise byte-identical to serde_json; `Project::to_json` and `.tkev` headers use it, and `from_json` decodes from text, never from a `Value`. Any future JSON writer (hit cache, teach files, imported tracks) must use it too.

## 2026-10-21: Persistence calls the spec left open
1. Evidence onsets must be sorted non-decreasing with `at < take_frames`; Evidence equality is bitwise.
2. `Interpretation::validate` cannot see evidence headers or imported tracks, so `check_evidence` and `check_track` do those checks where the data is at hand.
3. Render keys treat the backend list as a set: order is irrelevant and a duplicate is an error.
4. `Project::save` refuses an invalid project and never overwrites an existing `.bak`; `from_json_with` takes a step list so test-only migration chains can run; `migrate` writes `schema_version` after each step itself.
5. `TimeMap` rounds ties half up; a tick before the origin maps to sample 0.
6. A NaN or infinity fed to `TakeHasher` poisons the rest of that hash (capture bug, rejected at `finish`).

## 2026-10-21: Evidence stores pYIN candidates; decoding is a separate pure function
The evidence crate stores per frame the top three pYIN troughs (frequency and probability) and the total voiced probability. Viterbi decoding (`evidence::decode`) reads only those, takes every opinion as a `DecodeParams` field (range prior, step size, octave-jump penalty, switching cost, voicing threshold) and re-decodes a 10-minute take without touching audio, so the lens can call it per slider move. It lives in `evidence` rather than `render` because `cli analyze` needs a default decode for evaluation; the lens will own the parameters. Top-3 loses the probability mass of further troughs; on Vocadito that cost nothing measurable.

## 2026-10-21: Pitch and voicing are decoded separately, not in pYIN's joint HMM
pYIN's joint model scores unvoiced states at `(1 - p) / n_bins` each, so any frame with a candidate far above `1 / n_bins` decodes voiced: 27.6% voicing false alarm on Vocadito. Scoring unvoiced states at `1 - p` fixed false alarms but let the unvoiced layer's pitch memory drift (RPA fell from 98.7 to 87.7). The decoder now runs a pitch-only Viterbi (unvoiced mass spread over all bins as a floor, so the path holds through gaps) and a separate two-state voicing Viterbi with threshold 0.5 and switch probability 0.01. Vocadito: RPA 98.7, VR 90.3, VFA 6.7.

## 2026-10-21: Octave-jump penalty is an explicit transition, not a prohibition
Transitions are a triangular kernel of +/-2.155 semitones per 5 ms frame (pYIN's 35.92 octaves/s) plus near-octave links at 1e-3 of a local move's weight. A one-frame octave blip cannot win; a sustained octave disagreement can be corrected. Range prior default 55 to 1100 Hz (lens setting; the founder's range calibration replaces it).

## 2026-10-21: f0 runs at capture rate in v0.1, not on a 16 kHz copy
MASTER-PLAN 4.6 says f0 at 16 kHz. Running YIN at capture rate avoids a resampler dependency and its drift for now; a 10-minute 48 kHz take analyses in 8.3 s with 306 MB peak RSS. Downsampling for f0 is an optimisation to take if analysis time matters in the app.

## 2026-10-21: Evidence analysis constants
All fixed, all part of `ANALYZER_VERSION` 1: candidate range 40 to 2000 Hz; YIN integration window 25 ms centred on the frame; loudness window 23.2 ms (A-weighted mean square, dBFS, floor -120); spectral window 46.4 ms for centroid, tilt (third-octave band regression 100 Hz to 8 kHz, mean power per bin) and CPP (quefrency 60 to 880 Hz, regression over the same range); undefined values (silence below -90 dBFS, no pitch candidate for harmonic units) are NaN. Centroid in harmonic units divides by the most probable candidate, which is parameter-free; dividing by the decoded f0 instead is lens work.

## 2026-10-21: Onset candidates and the default pick
Evidence keeps every SuperFlux local maximum (24 bands/octave, 30 Hz to 17 kHz, lag 2 frames, max over 10 ms before and 20 ms after, strength above a 150 ms trailing mean) refined by backtracking to where the 2 ms energy envelope (dB) crossed 30% of the rise from its 60 ms floor to the hit's peak, plus that rise in dB. SuperFlux's 50 ms post-max and the 15%-of-peak backtrack from docs/research/08 both put onsets 20 to 40 ms late on two-part beatbox sounds (a soft "p" before the loud "kh"). The default pick in `onset::pick` (threshold 1.1, 60 ms refractory, and inside 100 ms of a kept onset only candidates rising at least 25 dB) is for `cli analyze` and evaluation; the drums crate owns the real decision. The post-max, backtrack and pick values were chosen while looking at AVP, so AVP numbers are optimistic; the golden set is the honest test.

## 2026-10-21: Fixes from an independent review of the evidence crate
- Pitch transitions use one normaliser for every row. Per-row normalisation (as in librosa's `transition_local`) made the range edges attractors, so held pitch through a long unvoiced gap fell by octaves; now 220 Hz holds through 400-frame gaps. When no frame has an in-range candidate, f0 is 0 throughout.
- Non-finite input samples are treated as 0 (one NaN otherwise erased onsets for 150 ms).
- CPP smooths the power cepstrum over 1 ms of quefrency (CPPS-style); on a voice-like tone with a 40 dB floor it varies under 1.5 dB across 218 to 222 Hz. In noiseless synthetic tones it still varies by 2 to 3 dB because sidelobe dips dominate; real voices always have a floor.
- Spectral centroid stops at 16 kHz so it does not scale with capture rate (it read 765 Hz at 44.1 kHz and 901 Hz at 192 kHz for the same tone).
- The tilt fit includes the nominal 8 kHz third-octave band (centre 8063 Hz) as documented.
- A WAV with sample rate 0 is an error, not a panic.
- Kept on purpose: audio before the take is silence, so a take that starts mid-sound (or on loud room tone) has an onset candidate at its first frame; AVP files begin with a hit at 0.000 s. Instant attacks read up to 1 ms early (centred 2 ms envelope box); shifting by half a box made AVP timing slightly worse.
