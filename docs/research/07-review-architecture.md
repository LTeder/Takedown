# Takedown: adversarial architecture review and minimum foundation (2026-10-20)

*Review from the architecture red-team pass. Verifies the Rust stack, attacks the data model and render trait, and proposes the minimum v0.1 foundation.*

## 0. Verdict up front

The Rust stack is viable and nothing in it is dead. The dependencies are all MIT, Apache, ISC or MPL-2.0, so all are GPLv3-compatible. The plugin-GUI layer is the problem: baseview, egui-baseview and nice-plug all shipped breaking releases in the last six weeks, and the keyboard fixes that matter landed in October 2026. Two parts of the proposed architecture are under-specified in ways that would force a rewrite later:

1. **The boundary between Evidence and Interpretation.** Evidence as currently described already contains user-tunable decisions.
2. **"Settings + seed + edits".** It doesn't say how edits survive re-derivation, or how reproducible it can be across platforms.

Section 5 fixes both with a small set of day-one types. The single most important design call: **the renderer never sees notes**. Notes compile into control curves plus trigger events, so the Fidelity knob is a property of the "lens" stage and not of either backend.

---

## 1. Stack verification (October 2026)

| Component | Status found | Verdict |
|---|---|---|
| **nice-plug** | 0.4.2. Last commit 2026-10-18 (baseview 0.4.1 update). "Big editor API changes" 2026-07-30. README says "experimental… expect some bugs" and "limited testing on macOS". ISC. VST3 now uses the `vst3` 0.3.0 crate (MIT/Apache, generated from the MIT SDK headers), not GPL `vst3-sys` ([repo](https://codeberg.org/RustAudio/nice-plug), [crate manifest](https://codeberg.org/RustAudio/nice-plug/raw/branch/main/crates/nice-plug/Cargo.toml), [vst3 crate](https://docs.rs/crate/vst3/latest)) | **Use it, pinned to a commit, behind a thin internal adapter.** Expect API breaks. Open issue #71 is a panic when changing latency, so report a constant latency. Issue #31 proposes moving its CLAP layer to clack, which means more churn |
| **nih-plug** | README says "maintenance mode" and its VST3 bindings are GPLv3 ([repo](https://github.com/robbert-vdh/nih-plug)) | Don't use. The GPL is legally fine for a GPLv3 project, but the code is frozen |
| **nice-plug standalone** | Separate cpal input and output streams synced with `fixed_resample`, because cpal duplex isn't released ([#41](https://codeberg.org/RustAudio/nice-plug/issues/41)). JACK-oriented CLI config | **Don't use `nice_export_standalone`.** Build your own eframe + cpal app as planned |
| **egui-baseview / nice-plug-egui** | egui-baseview 0.9.0 (2026-10-17). Four minor releases since 2026-08-17. nice-plug-egui 0.6.0 pins egui 0.37.0 + egui-baseview 0.9.0 ([crates.io](https://crates.io/api/v1/crates/egui-baseview), [manifest](https://codeberg.org/RustAudio/nice-plug/raw/branch/main/crates/nice-plug-egui/Cargo.toml)) | Healthy but churning. eframe is 0.37.0 (2026-10-15), so **a shared `ui` crate works today only if egui moves in lockstep** across both shells |
| **Known host bugs** | [nice-plug #95](https://codeberg.org/RustAudio/nice-plug/issues/95): egui UI renders black on first open in **REAPER**. Reported on Linux, root cause in baseview; a fix commit (7197104c) post-dates 0.4.2 and isn't confirmed by the reporter. [baseview #169](https://github.com/RustAudio/baseview/issues/169) (no key events in Reaper on macOS) and [#152](https://github.com/RustAudio/baseview/issues/152) (keyboard focus versus host shortcuts) were both **closed 2026-10-02** via PR #202. Releasing focus back to the host is explicitly not implemented. [nih-plug #105](https://github.com/robbert-vdh/nih-plug/issues/105) (egui TextEdit gets no keys) is still open | Keyboard in Reaper on Windows and macOS has been fixed only in the last three weeks and hasn't been tested by anyone at scale. **Design the plugin UI to need no text entry and no keyboard shortcuts in v0.x** (drag-values, buttons) |
| **baseview** | 0.4.1 (2026-10-09), now published on crates.io, with 0.3.x and 0.4.x releasing in parallel ([crates.io](https://crates.io/api/v1/crates/baseview)) | Same as above |
| **cpal** | 0.18.2 (2026-08-16), Apache-2.0. Default backends: WASAPI and CoreAudio. `asio` feature needs the ASIO SDK plus LLVM/Clang. **No WASAPI exclusive mode** in the API or changelog. Duplex streams exist only in unreleased master ([docs](https://docs.rs/crate/cpal/latest), [CHANGELOG](https://raw.githubusercontent.com/RustAudio/cpal/master/CHANGELOG.md)) | Docs 00 and 03 promise "WASAPI shared/exclusive"; **that is false with cpal.** Shared mode is fine for capture-first. Open one input stream; output only for monitoring/playback |
| **ASIO licence** | Now GPLv3 (LibreArts says "GPLv3+"; check Steinberg's text for "or later") or proprietary ([LibreArts](https://librearts.org/2025/11/steinberg-relicenses-vst3-and-asio/)) | Compatible with a GPLv3 project. Defer for build-toolchain reasons, not legal ones |
| **midir** | 0.11.1 (2026-10-10), MIT. Virtual ports on "every platform but Windows" ([docs](https://docs.rs/midir/latest/midir/os/unix/index.html)) | macOS virtual out works. On Windows, Windows MIDI Services is GA on 24H2/25H2 with user-created loopback endpoints that midir can open as ordinary ports ([Windows blog](https://blogs.windows.com/windowsexperience/2026/02/17/making-music-with-midi-just-got-a-real-boost-in-windows-11/)). No driver work needed. Defer |
| **rtrb / arc-swap / triple_buffer** | rtrb 0.4.0 (2026-08-17), arc-swap 1.9.3 (2026-10-18), triple_buffer 9.0.0 (MPL-2.0, which is GPL-compatible) | All fine. One trap with arc-swap is covered in §4 |
| **tract** | tract-onnx 0.23.9 (2026-10-15), MIT/Apache. Has `gru.rs`, `lstm.rs`, `rnn.rs`, `scan.rs` ONNX ops. **Pulsification** for streaming conv models. Metal and CUDA backends exist ([crates.io](https://crates.io/api/v1/crates/tract-onnx), [ops dir](https://github.com/sonos/tract/tree/main/onnx/src/ops/rec), [repo](https://github.com/sonos/tract)) | **Best fit for small conv/GRU models on CPU**, and pulse mode is the live-path story. Plan GPU via tract-metal plus burn, not only burn/candle |
| **ort** | 2.0.0-rc.15 (2026-10-19). Still no stable release after many RCs. Wraps C++ ONNX Runtime ([crates.io](https://crates.io/api/v1/crates/ort)) | Avoid in the plugin (shared-library and symbol clashes). Fine as a Phase 0 reference |
| **burn-onnx / candle-onnx** | burn-onnx 0.22.0 (2026-10-06, imports ONNX as generated code). candle-onnx 0.11.0 (2026-06-26) | burn is the GPU-offline path (wgpu). Skip candle-onnx: its ONNX operator coverage is narrow |
| **PyO3 / maturin** | 0.30.1 (2026-10-14) / 1.15.0 (2026-08-24) | Healthy, but not needed on day one (§5) |
| **AU later** | clap-wrapper-rs 0.3.1 (2026-05-18, MIT/Apache) exports VST3 + AUv2 from a Rust `clap_entry`. Documented with nih-plug, **unverified with nice-plug**. No standalone ([docs](https://docs.rs/crate/clap-wrapper/latest)). The CLAP-first Rust approach is shown in [rust-clap-first-example](https://github.com/CrushedPixel/rust-clap-first-example) | Credible. It is also a fallback VST3 path if nice-plug's VST3 layer breaks |

**Licensing inconsistency to resolve now.** Doc 00 §8 says core crates are MIT/Apache; the brief says GPLv3 everywhere. If it really is GPLv3 everywhere:
- PESTO's LGPL-3.0 code becomes usable, so the "retrain to avoid LGPL" work disappears unless you want the weights trained on your own data anyway.
- Surge XT's GPL-3.0-only DSP becomes portable as a reference. Copying from it would make the combined work GPL-3.0-only rather than "or later".

**Hosting.** Codeberg's July 2026 Terms of Use ban repositories that are "mostly" LLM-generated code without human oversight ([The Register](https://www.theregister.com/ai-and-ml/2026/07/23/codeberg-gives-vibe-coded-projects-the-toss-promotes-human-floss/5277717)). Host Takedown on GitHub. Keep founder-reviewed patch forks of nice-plug and egui-baseview (both on Codeberg) and upstream fixes with human review.

---

## 2. Attacking the data model

**The Evidence/Interpretation boundary is wrong as drawn.** A Viterbi octave penalty, a per-user range prior, a voicing threshold and the Confidence knob are user parameters. If Evidence stores *decoded* f0, Evidence is already an interpretation: every Confidence change means re-analysis, and "Reroll uncertain regions" has nothing to sample from.

**Fix.** Evidence holds parameter-free observations:
- per-frame **top-K pitch candidates** (K=3) with salience;
- voicing probability;
- raw spectral features;
- onset candidates with strength.

Decoding (Viterbi, range prior, voicing hysteresis) moves into the lens and runs in milliseconds. Storage for a 10-minute take at 200 Hz is about 120k frames × roughly 20 floats ≈ 10 MB. That's fine.

**Time base.**
- Take audio: integer sample frames (`u64`) at the take's capture rate. This is canonical and never resampled in place.
- Curves: a fixed grid of **200 Hz (5 ms)**, defined in *seconds* (frame k is at k/200 s). The grid is independent of any sample rate.
  - 10 ms is too coarse for loudness attacks that drive a synth envelope.
  - 200 Hz is an integer number of samples at 48k/96k/16k but not at 44.1k (220.5). So map sample n to a control position with exact integer arithmetic (`n * 200 / sr` with remainder), never by accumulating floats.
  - Model-native hops (10 or 16 ms) are resampled onto the grid, with provenance recorded.
- Onsets: stored as sample positions at capture rate, not frame indices. Drum timing needs about 1 ms precision.
- PPQ is never primary. Musical time is a lens: a `HostAnchor` captured at record time plus an optional tempo map in the Interpretation.

**Sample-rate change.** The take is unaffected. Evidence is unaffected because it's in seconds. Rendered audio is keyed by output rate, so a host rate change invalidates the render cache and triggers a re-render. Don't resample the cached render.

**Spectral features at the wrong rate.** Brightness and breathiness must **not** be computed on the 16 kHz model-input copy: centroid and noise above 8 kHz are the very signal you want. Compute f0 at 16 k and timbre features at capture rate. The mouth-transient exciter also needs full-band raw audio, so it references the take; it is not stored in Evidence.

**Variable host block size.** This is irrelevant to Evidence. It matters only for the renderer; see the invariance rule in §3.

**A 10-minute take.**
- Size: about 115 MB mono f32 at 48 k.
- The audio thread must not grow a `Vec`. It writes to an `rtrb` ring drained by a disk-writer thread straight into a WAV or FLAC file.
- The waveform view needs a precomputed min/max peak pyramid, built by the writer as it goes.
- Rendering must be chunked, cancellable and progressive.
- The take can never go into plugin state (§4).

**Editing while a render runs.** Render jobs are pure functions of `Arc` snapshots (`Arc<Evidence>`, `Arc<Interpretation>`) plus a generation number. A newer generation cancels the older job and replaces its result. Re-rendering only the changed range needs backend state at the chunk boundary, and filters, GRU hidden state and envelopes all make that non-trivial. Options:
- (a) Re-render from the nearest *unvoiced gap* of at least 50 ms before the dirty region, treated as a state reset point, and crossfade back in.
- (b) Have the backend expose `snapshot`/`restore`. Defer this.

Decide now that backends must tolerate **reset at silence**. That makes (a) always valid.

**Undo spanning analysis and edits.** Analysis isn't undoable; it's a content-addressed cache. Undo covers only Interpretation (settings + edits) and project structure (layers).

The real hazard is **edits on derived notes**. When Fidelity changes, segmentation re-runs and note indices change, so "delete note 7" is orphaned. Rules:
- Edits are **time-anchored overrides on the lens output**, for example "force pitch 43 in [12.30 s, 12.55 s]" or "split at 14.02 s", and never references to derived indices.
- Or there is an explicit *Commit to notes* that materializes an owned `NoteTrack`, after which settings stop re-deriving it.

Write these semantics down before the `notes` crate exists.

**"Reproducible from seed".** This is false across tract SIMD kernels, x86 versus ARM, FMA, model updates and GPU. Promise reproducibility only for (Evidence hash, backend id + version, platform class), and make **materialized layer WAVs the durable artifact**, as the founder decided already.

**What gets persisted:**

| Item | Lossy or re-derivable? | Persist? |
|---|---|---|
| Raw take audio | Irreplaceable | Yes, lossless, content-hashed |
| Capture metadata, HostAnchor | Irreplaceable | Yes |
| Evidence | Re-derivable, but model- and version-dependent | Yes, as a cache keyed by (audio hash, analyzer version, model hashes, params hash). Never silently recompute under a new model |
| Interpretation (lens settings, seed, edits, backend ref + preset) | Small, canonical | Yes, in the undo history |
| Rendered layer audio | Not bit-reproducible across platforms | Yes, materialized and keyed by interpretation hash + sample rate |
| Peaks, render scratch | Re-derivable | Cache only |

Project format: a **directory** (`project.json` with schema version and migrations, `audio/`, `cache/`), not a zip. Rewriting a 200 MB zip on every autosave is a bug.

---

## 3. Attacking the RenderBackend trait

**One input type for both backends.** DDSP-class decoders are conditioned on f0 and loudness curves; they don't take note events. If notes reach the renderer as events, the neural backend ignores them and Fidelity means something different per backend. So:
- The **lens** compiles everything (raw curves, decoded pitch, notes, quantize, Glide/Retrigger, Timbre Follow) into a `ControlTrack`.
- Fidelity becomes `pitch = lerp(raw_log_f0, note_snapped_log_f0, fidelity)` with glide shaping.
- Retriggers are **encoded in the curves** as amplitude dips and gate falls, *and* as explicit `Trigger`s.
- The neural backend reacts to the curves. The VA synth may also use triggers for envelope resets. Both see the same musical intent.

**Rate and smoothing contract.**
- Controls arrive at 200 Hz.
- The backend interpolates per sample: pitch linear in semitones, amplitude linear in dB, everything else linear.
- **Any smoothing is a function of absolute sample index, never of block boundaries.**
- Musical smoothing (de-zippering noisy features, 5–20 ms) belongs in the lens, where it is visible, seedable and editable. The backend may add only fixed anti-zipper smoothing that it declares in `BackendInfo`.

**Block-size invariance is the answer to "two implementations".** The same `process()` runs offline (render worker, any block size) and live (host blocks of variable size). The property test is that output is **bit-identical for random block segmentations**. The offline/live difference then shrinks to the *source* of controls:
- Offline: the lens over the full Evidence.
- Live: a causal streaming analyzer that feeds the same `ControlTrack` ring.

Two analyzers are unavoidable; two renderers are not.

**Seeded randomness.**
- No sequential global RNG. Use a **seed tree**: `seed.derive("lens/variance", region_id)`, `seed.derive("backend/noise", 0)`.
- Noise generators must be **counter-based** (`noise = hash(seed, stream, sample_index)`). Chunked, partial and monolithic renders are then identical, and rerolling region R can't reshuffle randomness in region S.
- "Reroll uncertain regions only" becomes a re-derive of child seeds for the flagged regions.

**Neural backends are not real-time-safe on day one.** tract plans allocate. `BackendInfo.realtime_capable=false` routes monitoring to the VA synth until a pulsified, preallocated tract plan is proven.

---

## 4. Threading and plugin integration

**Threads:**
- **Audio thread:** mic samples go to an `rtrb` ring, drained by a disk writer. Playback reads the published render via `arc_swap::ArcSwap<RenderedLayer>`, indexed by host transport sample position minus the HostAnchor (not a free-running counter), so loops and seeks work.
  - **Trap:** if the UI swaps while the audio thread holds the last `Arc`, the deallocation happens on the audio thread. Hand retired `Arc`s back to a GC thread over a second ring, or hold every published buffer in a UI-side retire list for N seconds.
  - Assert no allocations with nice-plug's `assert_process_allocs` feature.
- **Worker pool:** one **process-wide** pool (`OnceLock`) shared by every plugin instance, sized to cores−2, with jobs for analysis then render.
  - Each job carries `Arc<AtomicBool>` cancel, `AtomicU32` progress and a generation number.
  - Results return to the UI over a channel; the UI publishes them to the audio thread. The UI repaints while any job is active.
- **UI:** owns Project and the undo history. The audio thread sees only immutable published buffers and an `rtrb` command queue (arm, stop, play-layer).

**Plugin state versus disk.**
- VST3 and CLAP have no "project directory" API, and Reaper keeps FX state in project files and undo history. 100 MB states would cripple it.
- Host state should be a small JSON: project UUID, take hashes, interpretations, layers, and paths. Target under 100 KB.
- Audio lives in a **content-addressed store**: `<data_dir>/takes/<blake3>.wav`.
- Later, add Reaper-specific "collect into project folder" through Reaper's extension API, plus an "embed if under X MB" option.
- Test moving projects between machines early (§6).

**Drag-out (feasibility verified, with a catch).**
- Neither egui nor baseview has an OS drag *source*.
- The `drag` crate 2.1.1 (MIT/Apache, [docs](https://docs.rs/crate/drag/latest), [repo](https://github.com/crabnebula-dev/drag-rs)) does file drags on Windows (OLE) and macOS from any `HasWindowHandle`.
- Its macOS code reads `NSApp.currentEvent` only for a timestamp, fabricates a `LeftMouseDragged` event, and starts the session on the **window's contentView** ([source](https://raw.githubusercontent.com/crabnebula-dev/drag-rs/main/crates/drag/src/platform_impl/macos/mod.rs)). Inside a plugin, that's the **host's** view, not baseview's child NSView.
- egui decides on the *next frame*, outside the native mouse event handler.

Plan:
1. Vendor about 200 lines.
2. Windows: `DoDragDrop` with `CF_HDROP` (a modal loop; fine).
3. macOS: `beginDraggingSessionWithItems:event:source:` on **baseview's own NSView**.
4. Start the drag *synchronously from the baseview mouse-drag event handler*, by hit-testing drag-handle rectangles that egui registered on the previous frame. This probably needs a small hook in the nice-plug-egui fork.
5. Write the files into a persistent exports folder, not a temp folder. Hosts may reference the file in place.

**Sidechain.**
- nice-plug advertises "variable numbers of input and output ports". Its `AudioIOLayout` docs could not be fetched (404), so the exact mechanism is **unverified**. nih-plug's model is `main_input_channels` plus `aux_input_ports`, which map to a VST3 kAux bus and a non-main CLAP audio port.
- Declare two layouts:
  1. main stereo in + stereo out;
  2. no main in + one aux "Voice" port + stereo out.
- **Reaper:** put the instrument on the vocal track, arm it and turn input monitoring on. Layout 1 receives the mic with no routing. Sidechain sends go to track channels 3/4 via the pin connector. Reaper has VST3 bus-count quirks (the "Request VST3 bus channel count" option; it matches sidechain width to the main bus) ([JUCE forum](https://forum.juce.com/t/vst3-sidechains/24252)).
- Ableton, FL and Cubase need layout 2. Ableton's support for aux inputs on third-party VST3 instruments is **unverified**: test it before promising it.

---

## 5. Minimum v0.1 that is the real foundation

**Crates on day one (five):**
```
crates/
  core/     time, ids, Take, Evidence, ControlTrack, Interpretation, Layer, Project, schema+migrations, SeedTree
  evidence/ DSP-only analysis: YIN/pYIN-style f0 candidates, loudness, centroid, HNR, LPC F1/F2, onset candidates
  render/   RenderBackend trait, lens(), offline driver, 2 backends: VA synth + harmonic-plus-noise (DDSP's DSP half, no network)
  cli/      `takedown analyze|render in.wav --preset --seed --fidelity -o out.wav`  (golden tests + Phase 0 bridge via files)
  app/      eframe + cpal standalone: record, analyze, render, play, drag-out
spikes/plugin-smoke/  nice-plug + egui + sidechain record + drag-out (throwaway, week 1)
```

**Later:** `notes` (after the edit semantics are written), `drums`, `infer` (tract, once Phase 0 picks a model), `ui` (extract from `app` when the plugin is real; meanwhile `app` widgets may depend only on `egui`, never on `eframe`), `plugin`, `py` (PyO3, once Python needs in-process calls; files and the CLI cover Phase 0).

**Why two backends on day one.** The harmonic-plus-noise synth costs about a day once you have the curves. It is exactly what the future neural backend outputs (the network will only predict its amplitudes), and it keeps the trait honest from the first commit.

**Types that must be right from the start (pseudocode):**
```rust
// core::time
pub const CONTROL_HZ: u32 = 200;                       // never changes; frame k at k/200 s
pub struct SampleRate(pub u32);
pub struct SamplePos(pub u64);                          // always paired with a SampleRate
pub fn ctrl_pos(n: SamplePos, sr: SampleRate) -> (u64, u32 /*frac num*/); // exact integer mapping

// core::take
pub struct TakeId(pub [u8; 32]);                        // blake3 of PCM
pub struct Take { id: TakeId, audio: AudioRef /*path, sr, ch, frames*/, meta: CaptureMeta, anchor: Option<HostAnchor> }
pub struct HostAnchor { host_sample_at_start: i64, host_sr: u32, ppq_at_start: f64, bpm: f64, sig: (u8, u8) }

// core::evidence  (parameter-free observations)
pub struct Provenance { analyzer: String, version: u32, model_hashes: Vec<[u8; 32]>, params_hash: [u8; 32] }
pub struct Evidence { take: TakeId, prov: Provenance, n: usize,
    f0_cand_hz: Vec<[f32; 3]>, f0_cand_sal: Vec<[f32; 3]>, voicing_p: Vec<f32>,
    loud_db: Vec<f32>, centroid_hz: Vec<f32>, hnr_db: Vec<f32>, f1_hz: Vec<f32>, f2_hz: Vec<f32>,
    onsets: Vec<Onset /* at: SamplePos (take sr), strength */> }

// core::control  (the ONLY renderer input)
pub struct ControlTrack { start_s: f64, n: usize,
    pitch_st: Vec<f32>, amp_db: Vec<f32>, gate: Vec<f32>, bright: Vec<f32>, breath: Vec<f32>,
    vowel: Vec<[f32; 2]>, triggers: Vec<Trigger> }
pub enum TriggerKind { Retrigger, NoteOn { pitch_st: f32, vel: f32 }, NoteOff, Hit { role: u8, vel: f32, exciter: Option<SliceRef> } }
pub struct Trigger { at_s: f64, kind: TriggerKind }

// core::interp
pub struct Seed(pub u64);
impl Seed { pub fn derive(&self, label: &str, idx: u64) -> Seed; }
pub struct Interpretation { id: Uuid, take: TakeId, evidence: EvidenceKey, lens: LensSettings /*versioned*/,
    backend: BackendRef { id: String, version: u32, preset: serde_json::Value }, seed: Seed, edits: Vec<Edit> /*empty in v0.1; enum exists*/ }
pub enum Edit { ForcePitch { t0: f64, t1: f64, st: f32 }, SplitAt { t: f64 }, /* time-anchored only */ }
pub struct Layer { id: Uuid, interp: Uuid, render: Option<RenderRef /*path, sr, interp_hash*/>, gain_db: f32, mute: bool }

// render
pub fn lens(e: &Evidence, s: &LensSettings, edits: &[Edit], seed: &Seed) -> ControlTrack;
pub struct BackendInfo { id: &'static str, version: u32, realtime_capable: bool, latency: u32, resets_at_silence: bool }
pub trait RenderBackend: Send {
    fn info(&self) -> BackendInfo;
    fn prepare(&mut self, sr: SampleRate, max_block: usize);           // allocate here only
    fn reset(&mut self, seed: Seed, at: SamplePos);
    fn process(&mut self, ctl: &ControlTrack, at: SamplePos, out: &mut [f32]); // block-size invariant
}
pub fn render_offline(b: &mut dyn RenderBackend, ctl: &ControlTrack, sr: SampleRate,
    cancel: &AtomicBool, progress: &AtomicU32) -> Result<Vec<f32>, Cancelled>;
```

**Deliberately hacked or omitted in v0.1:**
- notes, drums, alternates A/B/C (one interpretation), and the edit UI (the `Edit` enum exists but has no UI);
- ML and tract (DSP f0 only);
- monitoring (dry passthrough only), MIDI out, virtual ports, ASIO;
- plugin (spike only), PyO3, GPU, presets (one hard-coded patch per backend);
- undo (settings are the only state and are re-applied);
- macOS signing and notarization (local builds only).

Keep macOS **building and running the CLI tests in CI from day one**.

**The vertical slice, in order:**
1. **Week 1, plugin smoke spike on both OSes:** nice-plug VST3 + CLAP; egui window; record the main or sidechain input into a ring; drag a fixed WAV and MIDI out to a Reaper track.
2. `core` + `evidence` + `render` + `cli`: one founder hum WAV becomes a VA-synth WAV and a harmonic-plus-noise WAV at Fidelity 0. Add golden tests.
3. `app`: record from cpal, analyze, render, play, then drag-out into Reaper.

**Definition of done:**
- On Windows and macOS, record 30 s and 10 min in the standalone. Render with both backends at 44.1 k and 48 k. Drag the WAV into Reaper and it lands on a track.
- Property tests pass:
  - block-size invariance (bitwise);
  - chunked render equals monolithic render;
  - same seed gives the same hash on one platform;
  - changing one region's seed leaves other regions bitwise unchanged.
- The 10-minute take never allocates on the audio thread (`assert_no_alloc` in debug), and peak RSS stays under 600 MB.
- The project directory round-trips through schema version 1 with a migration test stub.
- The plugin spike passes in Reaper 7.x on both OSes: open and close the editor 50 times; run 4 instances; drag-out works.

---

## 6. Top 10 ways this fails, ranked, with the earliest cheap test

1. **Continuous resynthesis of amateur voice curves sounds worse than "notes into a synth".** That kills the voice-to-audio-first thesis. *Test (week 1–2, Python or CLI):* render 10 founder takes at Fidelity 0 and 100, plus a ReaTune→MIDI→same-synth baseline. Run a blind A/B on yourself plus two others. If Fidelity 0 loses, the default must move toward notes.
2. **Plugin UI is broken in Reaper on one OS:** black first open (#95), keys, multi-instance, HiDPI, macOS "limited testing". *Test:* the week-1 smoke spike on Reaper Windows and macOS: VST3 and CLAP, 50 editor open/close cycles, 4 instances, typing in a field, spacebar.
3. **Drag-out fails from inside the plugin**, because the event comes from the wrong NSView or the drag starts outside the native handler. *Test:* the same spike, dragging a WAV onto Reaper's arrange view on both OSes. If it fails, fall back to "Export → insert at cursor" via Reaper's API.
4. **Octave and voicing glitches are audible as blips in continuous mode.** Notes median-filter them; curves don't. *Test:* run pYIN plus one neural tracker on 20 hums. Count octave-jump frames per minute after Viterbi, and listen to the harmonic-plus-noise render.
5. **The neural backend underdelivers.** The domain gap between voice f0/loudness and instrument-trained conditioning (DDSP-VST needed manual pitch and loudness adjustment), plus in-house data scarcity and tract speed. *Test (Phase 0):* train one tiny DDSP on recordings of your own synth, drive it with voice curves, and time tract inference per second of audio.
6. **Edits don't survive re-derivation**, so the editor becomes Melodyne-style commit-only. *Test:* a one-page spec of five edit types and their behaviour under Fidelity and Confidence changes, written before `notes` exists.
7. **Reproducibility claims break** across platforms and model versions. *Test:* CI renders a fixed interpretation on Windows x64 and macOS arm64 and diffs the hashes. Expect tolerance-only equality and design around it.
8. **Ecosystem churn.** egui 0.37, egui-baseview 0.9, baseview 0.4 and nice-plug 0.4.2 all released within weeks; editor API rewrites; egui lockstep between eframe and nice-plug-egui. *Test:* a weekly CI job that runs `cargo update` and builds the app and plugin, with commit-pinned forks.
9. **State bloat or lost audio** (no project-path API; Reaper undo carries plugin state). *Test:* a 10-minute take in the plugin. Save, then measure `.rpp` size and undo latency. Move the project folder and reopen on the other machine.
10. **In-plugin capture is misaligned or awkward.** Input monitoring must be armed, and I/O latency offsets the HostAnchor. *Test:* loop back a click into Reaper through the plugin and compare the plugin take's onset with Reaper's own recorded item. It must be within 1 ms after compensation.

Honourable mentions:
- cpal has no WASAPI exclusive mode (minor for capture-first).
- Multi-instance CPU oversubscription (the shared pool solves it; test with 8 instances analysing at once).
- Ableton's handling of aux inputs on VST3 instruments is unverified (test before any Ableton promise).
- Two-person bandwidth: the plan already has a quarter's worth of "later" crates. The five-crate cut above is the guard.
