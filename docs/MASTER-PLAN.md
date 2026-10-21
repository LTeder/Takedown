# Takedown: Master Plan v1.1

*2026-10-20. Supersedes research docs 00 through 04 where they conflict, and revises v1 of this plan after the founder's pushback. Distilled from the four research passes (research/01–04), the founder discussion recorded in research/00 §8, the four adversarial reviews (research/05–08), and the founder's direction on aesthetic and workflow. Decisions made after this date go in DECISIONS.md.*

---

## 1. What Takedown is

An open-source Rust tool. You bark an embarrassing beatbox fill, hum a squeal, breathe a whoosh, and it renders something that belongs on a JPEGMAFIA, Death Grips, SOPHIE or Aphex Twin record. Percussive and noisy sounds are the priority. Tonal input is an expression source, not something to transcribe cleanly: the founder plays piano and does not need voice-to-notes. It also exports what it rendered as MIDI/MPE describing the same performance, so a hit can trigger Serum or a drum rack in the DAW instead.

**The interface thesis.** The translation from voice to machine sound is subjective. The product is the minimal interface that yields the maximal range of expression: a handful of sliders, dice per aspect with locks, a palette selector, play, and export. You nudge and reroll until it sounds right. Everything else (recording takes, comping, arranging, mixing, reverb, OTT) is Reaper's job.

**What makes it different from DDSP-VST, Neutone and Baby Audio Humanoid:** those resynthesise the voice through a fixed model in real time. Takedown listens to the whole take offline, decides what each sound *is* (hit, pitched, noise), and renders a designed sound whose body is synthesized and whose character comes from a mangle stage, with the voice's own transient, loudness, brightness and breath shaping every hit. The voice is never heard as a voice unless you want it as raw material.

---

## 2. Decisions

| Topic | Decision | Why |
|---|---|---|
| Language, framework | **Rust.** eframe standalone first; nice-plug VST3+CLAP plugin in v0.2 | Founder preference; stack verified viable 2026-10 |
| License | **GPL-3.0-or-later everywhere.** Surge XT is a reference oracle, never copied, so "or-later" holds | Founder wants libre |
| First shell | **Minimal standalone app:** record or drop a take, evidence overlay, sliders, dice, play, export. No looper, layers, piano roll or comping, ever in v0.1 | The nudge-and-reroll loop needs a live window; a CLI round trip kills it. Reaper does recording and editing better than we will |
| Priority | **Percussive and noisy first, tonal as expression.** No note transcription goal | Founder plays piano; the target palette is drum and texture design |
| Aesthetic target | Hyperpop, industrial, glitch, braindance: SOPHIE, Death Grips, JPEGMAFIA, Aphex Twin, "sounds like it came from Serum" | Founder's stated palette |
| Effects boundary | **Mangle is synthesis, polish is the DAW's.** In the engine: drive, wavefold, hard clip, bitcrush and decimation, resonant filter screams, pitch envelopes, transient shaping, ring-mod and cross-mod against the voice. In Reaper: reverb, delay, compression, OTT, EQ, limiting | The target sounds are defined by distortion and modulation, not by a cleaner oscillator |
| Canonical object | **`ControlTrack`** (200 Hz curves + triggers) compiled by a lens from parameter-free `Evidence`. The renderer never sees notes. MIDI/MPE is its serialisation; audio is a backend rendering it | Audio and MIDI must never disagree; one object, two outputs |
| Voice as material | **Yes.** The take, sliced at onsets, pitched, crushed, folded and stuttered, as a parallel layer under synthesized bodies. Not a sample library | On-palette; "your bark is the raw material" is not a sampler |
| Sound engine | **Assemble.** Mutable Instruments DSP (MIT; Rust Plaits port with 24 engines incl. FM, wavefolder, noise, modal, string, speech, analog kick/snare/hat) plus an own conditioning layer, mod matrix, mangle stage, subtractive voice, harmonic+noise voice, noise-texture voice, voice-mangle voice. Rings-style resonator and Warps-style cross-modulation ported later | The control and mangle layers are the product; Plaits' three smooth macros per engine make dice and learned mapping work |
| Randomness | **Seed tree, per-aspect dice (body, mangle, timing feel, mouth amount, pitch reading), per-slider locks, draws constrained to a palette** | Randomize is only useful inside a small, smooth, coherent space |
| Palettes | Named sets of engines, body presets and mangle ranges (starting points: Rubber, Crushed, Glitch, Breaks). The founder writes their own | The dice needs a space; Serum-ness is preset craft |
| Pitch model | **DSP only in v0.1** (pYIN/Viterbi); retrain SwiftF0 or PESTO on clean data only if DSP fails on hum or noise | No off-the-shelf f0 checkpoint is shippable under the clean-data rule |
| Neural rendering | **None in v0.1.** v1: sound matching onto the project's own engine (CMA-ES first, learned second). DDSP only ever as an optional acoustic-instrument backend | Sound matching trains on voice alone, which is clean data; DDSP has nothing to learn for synth output |
| Drum classification | Per-user MFCC/envelope features with a nearest-neighbour head, confidence gating, soft role assignment that the dice may override | 84% on AVP with no model; stays clear of the CNN-specific claim in US11978473B1 (not legal advice) |
| Evidence | Parameter-free observations: top-3 f0 candidates with salience, voicing probability, loudness, brightness (harmonic units + tilt), breathiness (CPP), 2-D vowel embedding, onset candidates, per-frame class hints | Every slider move must be lens work in milliseconds, never re-analysis |
| Project state | Directory (`project.json` + `audio/` + `cache/`); audio content-addressed by blake3; plugin host state under 100 KB later | Reaper undo carries plugin state; zips rewrite on autosave |
| Reaper companion | **Dropped from v0.1.** Export to a folder; the founder drags from Explorer. A Lua action returns with the plugin | Fifty lines of convenience with little value until the plugin exists |
| Monitoring | Dry voice only in v0.1. Causal YIN + onset monitor in the plugin (v0.2) | Hearing the instrument changes how you perform, but capture-first wins on accuracy |
| Hosting | GitHub | Codeberg's 2026 terms reject mostly-LLM-authored repositories |
| Never | AAX, ARA, cloud, telemetry, a sample library, DAW features, polyphonic voice input, MIDI 2.0 | |

---

## 3. Design principles

1. **The renderer never sees notes.** Backends take a `ControlTrack` only. Fidelity, glide, retrigger, quantise and register are lens properties.
2. **One object, two outputs.** `ControlTrack` → audio and `ControlTrack` → MPE `.mid` describe the same performance. MPE `.mid` → `ControlTrack` round-trips within quantisation, so an edited `.mid` re-renders through the same engine, and the hits can equally drive Serum.
3. **Block-size invariance.** `process()` output is bit-identical for any block segmentation. One renderer for offline and live; smoothing is a function of absolute sample index.
4. **Evidence is observation; the lens is opinion.** Nothing a slider changes lives in Evidence.
5. **Time base.** Audio in integer samples at capture rate; curves on a fixed 200 Hz grid in seconds; onsets as sample positions; musical time is a lens over a `HostAnchor`.
6. **Seeds are a tree, noise is counter-based.** `seed.derive(label, idx)`; noise = hash(seed, stream, sample_index). Rerolling one aspect or region never disturbs another. Locks are just "don't re-derive this child".
7. **Edits are time-anchored, never index-anchored.** Semantics written before the `notes` crate exists.
8. **Backends reset at silence.** Partial re-render starts from the nearest unvoiced gap of 50 ms or more.
9. **Reproducibility per (evidence hash, backend id+version, platform class).** Materialised WAVs are the durable artefact.
10. **Nothing allocates on the audio thread.** Capture goes to a ring drained to disk; renders publish by atomic swap; retired buffers free off-thread.
11. **Mangle is per-voice and ordered.** Each voice owns its mangle chain (drive → fold → crush → filter → clip, reorderable). No global effects bus exists in the engine.
12. **Licence register from commit one.** No Surge code copied; no non-commercial weights or training data; no tainted teachers.

---

## 4. Architecture

### 4.1 Canonical objects

```
Take           immutable audio + capture metadata + optional HostAnchor; id = blake3(PCM)
Evidence       parameter-free per-frame observations at 200 Hz, keyed by (take, analyzer version, model hashes)
Hit            onset-anchored event: time (samples), duration, loudness, brightness, decay slope, class scores, exciter slice ref
LensSettings   versioned: palette id, fidelity, glide/retrigger, register, mouth amount, timing feel, quantise, per-aspect seeds + locks
Edit           time-anchored override on lens output (ForcePitch, SplitAt, ForceRole, MuteSpan, ...)
Interpretation (take, evidence key, lens settings, backend ref + preset, seed, edits)
ControlTrack   200 Hz curves: pitch_st, amp_db, gate, bright, breath, vowel[2]; Triggers (Retrigger, NoteOn, NoteOff, Hit{role, vel, bright, dur, exciter})
RenderBackend  trait: info(), prepare(sr, max_block), reset(seed, at), process(ctl, at, out)
Voice          a backend instance plus its mangle chain and body preset; a palette is a set of voices with ranges
Layer          (interpretation, materialised render keyed by interp hash + sample rate, gain, mute)
Project        directory: project.json (schema-versioned, migrations) + audio/ + cache/
```

`NoteView` and `HitView` are derived, read-only views of a `ControlTrack` for the MIDI exporter and the display; the MIDI importer produces a `ControlTrack`.

### 4.2 Pipeline

```
mic or dropped WAV ──▶ Take
   ──▶ evidence (DSP: pYIN/Viterbi top-3, voicing, loudness, brightness, CPP, vowel PCA, onsets, class hints)
   ──▶ lens(evidence, palette, settings, edits, seed tree) ──▶ ControlTrack
   ──▶ voices.process (bodies + mangle + voice-material layer) ──▶ WAV
   ──▶ MPE serialiser ──▶ .mid (hits with velocity and expression; phrases as MPE notes)
   export folder ──drag──▶ Reaper
   edited .mid ──▶ MPE parser ──▶ ControlTrack ──▶ voices ──▶ WAV
```

### 4.3 Crates

v0.1, seven crates plus a spike:

```
crates/
  core/      time, ids, Take, Evidence, Hit, ControlTrack, views, Interpretation, Layer, Project, migrations, SeedTree
  evidence/  DSP analysis, no models
  drums/     onset refinement, per-event features, per-user kNN head, doubled-sound merge, Hit events
  render/    lens(), RenderBackend, offline driver, conditioning layer, mod matrix, mangle chain,
             voices: Plaits (git-pinned mi-plaits-dsp, 48 kHz internal + rubato), VoiceMono, HarmonicNoise,
                     NoiseTexture, VoiceMaterial (take slices: pitch, crush, fold, stutter, reverse)
  midi/      MPE serialiser and parser, plain SMF
  cli/       takedown analyze | render | render-midi | batch; CSV/JSON for the eval harness
  app/       eframe + cpal (input only): record/drop, overlay, sliders, dice + locks, palette, A/B, play, export
research/    uv-pinned Python eval (mir_eval), golden set
spikes/plugin-smoke/   nice-plug VST3+CLAP + egui + input capture + drag-out, both OSes, throwaway
```

Later: `notes/` (HMM segmenter, quantise, key, groove, after edit semantics are written), `infer/` (tract behind one trait, only once a model earns its place), `plugin/` (nice-plug, pinned, thin adapter), `ui/` (widgets extracted from `app` when the plugin needs them; `app` widgets depend on `egui` only, never `eframe`), `py/` (PyO3 when Python needs in-process calls).

### 4.4 Stack, verified October 2026

eframe/egui 0.37, cpal 0.18 (input stream only; shared mode), nice-plug 0.4.2 + egui-baseview 0.9 for the v0.2 plugin (churning; pin to commits), tract-onnx 0.23 if a model is ever needed, rtrb, arc-swap, midir 0.11, mi-plaits-dsp (MIT, git dependency), rubato, realfft, hound, symphonia, blake3, serde, toml. Avoid: `ort` in-process, nih-plug, HexoDSP (AGPL), RAVE (NC code).

### 4.5 Sound engine

- **Conditioning layer:** clean f0 (octave repair, confidence gating, hold-through-unvoiced crossfading to noise), per-take percentile normalisation (p5–p95) of loudness, brightness, breathiness and vowel, response curves, asymmetric slew, derived vibrato and pitch deviation, non-causal zero-phase smoothing offline.
- **Mod matrix** at 32-sample block rate, per-sample ramps; MPE semantics internally.
- **Bodies.** Plaits engines (two-op and six-op FM, VA, wavetable, wavefolder/waveshaper, additive, particle and filtered noise, modal, string, inharmonic, chiptune, speech, analog and synthetic kick/snare/hat); VoiceMono (2 osc VA + wavetable, sub, noise, SVF, nonlinear ZDF ladder at 2× oversampling, formant filter, envelopes, glide); HarmonicNoise (DDSP's DSP half); NoiseTexture (multi-band filtered noise with resonant peaks, driven by loudness, brightness and breath; the whoosh and hiss voice); VoiceMaterial (the take itself, onset-sliced).
- **Mangle chain, per voice:** drive, wavefolder, hard clip, bitcrush and sample-rate decimation, resonant SVF with self-oscillation, pitch envelope (sweep depth and time), transient shaper, ring-mod or XOR against the voice or another voice. Reorderable. This is where Serum-ness, rubber and crush come from.
- **Hits.** Each hit's loudness → accent, brightness → tone and decay, duration → decay, and the hit's own 5–30 ms transient as exciter into a modal body or blended over a synthesized attack ("Mouth amount"). No two hits identical, so no round-robins. Timing is sample-accurate and pre-rolled so perceived attacks align.
- **Later:** Rings-style resonator and Elements-style exciters (MIT ports), Warps-style cross-modulation and vocoder (MIT port), granular voice-material (Clouds-style), morph pad, spectral morph in H+N form, layer-as-impulse-response convolution.
- **Honest limits:** acoustic kits, sampled-cymbal 909 hats, dusty boom-bap record texture, ensembles. The target palette does not need them.

### 4.6 Evidence and ML

- **v0.1 is DSP only** (see docs 08 §2 for the per-curve recipe). f0 at 16 kHz, timbre features at capture rate.
- **Drums:** per-event MFCC + envelope features, per-user z-score, kNN with k=3, fit-time augmentation, confidence from distance ratio, 30–60 ms refractory window, "b + oom" merge. Roles are soft: the lens may reassign bodies under the dice. Teach flow in the app: perform each role a few times; a leave-one-out separability matrix comes free.
- **Model only if earned:** if DSP fails on hum or noise, retrain SwiftF0 or PESTO on clean data (VocalSet, Vocadito, NSynth, PTDB-TUG, the founder's CC0 golden set, self-made resynthesis set). If several curves fail, a single causal multi-task CRNN trained as a student of the DSP teacher.
- **Sound matching (v1):** CMA-ES fit of curve-to-parameter splines against the real engine on 10–20 s of a take, then a learned mapping. This is what Mouth amount and Timbre Follow grow into.
- **Licence rule, strict:** shipped weights only from redistributable training data. Pretrained checkpoints for evaluation only.

---

## 5. Build plan

Two people. Claude writes most code; the founder records, listens, designs palettes, decides, reviews.

### Week 0 (2 days): the existential pre-check
Python. Onset-slice ten of the founder's beatbox fills, extract loudness, brightness and duration per hit, drive a quick Plaits-via-Python or Surge drum patch, mangle with a distortion, listen. Separately, SwiftF0 (evaluation use) on three hums → MPE → an FM patch. If none of it excites, stop. Record the golden set the same week: beatbox hits and fills on two mics in two rooms, noisy and breathy phrases, hums and scat, range slides; annotate onsets against a click and f0 in Tony.

### Weeks 1–2: foundation, evidence, hits
- Repo on GitHub, GPL-3.0-or-later, licence register, CI on Windows and macOS, weekly `cargo update` canary.
- `core` types with property tests for exact sample↔control mapping and seed-tree independence.
- `evidence` DSP, `drums` onset refinement and features, `cli analyze` → CSV, `research/eval` scoring onset F1 at ±25 ms, per-user hit accuracy at 5 and 10 shots, RPA/RCA/voicing on hums, and a curve-jitter metric.
- Gate E1: onset F1 ≥ 0.9 on the golden set; 4-class hit accuracy ≥ 85% with 10 shots; RPA ≥ 95% and octave errors ≤ 2 points on hums.
- Plugin smoke spike in parallel, both OSes, Reaper: nice-plug VST3 + CLAP, egui window, 50 editor open/close cycles, 4 instances, input capture, drag-out. Throwaway; decides v0.2's plugin shape.

### Weeks 3–4: render and the go/no-go
- `render`: conditioning layer, mod core, mangle chain, Plaits bodies, NoiseTexture, VoiceMaterial, offline driver, golden-file tests, block-size invariance and chunked-equals-monolithic property tests.
- `midi`: MPE serialiser for hits and phrases; `cli render`.
- Two starter palettes (Rubber, Crushed) hand-tuned by the founder with Claude.
- **Week-4 gate.** Ten embarrassing beatbox fills, rendered through each palette with three dice rolls each. Blind, shuffled. **Continue** if the founder would put at least six renders in a track as-is, and at least one is a sound they could not have made another way. **Pivot** if renders sound like a mouth at every Mouth-amount setting (then the exciter path is cut and hits become pure trigger + parameters, which is still a product), or if hit classification is so unreliable that the dice cannot rescue it (then the role head gets a model, or roles are dropped in favour of pure body-by-brightness mapping).

### Weeks 5–7: finish v0.1
- `app`: eframe window; record from cpal or drop a WAV; waveform with onsets, loudness and f0 overlay and per-segment class colouring; palette selector; six sliders (Mouth amount, Fidelity, Mangle, Timing feel, Register, Glide/Retrigger); dice per aspect with locks; A/B; play; export WAV + MPE to a folder. No text entry needed anywhere.
- VoiceMono and HarmonicNoise bodies; MPE parser and `cli render-midi`; perceptual-centre onset correction; `cli batch`.
- Two more palettes (Glitch, Breaks). Property tests from §3 pass; a 10-minute take renders under 600 MB RSS.

**v0.1 definition of done:** record a fill in the app, pick a palette, roll dice and nudge sliders with sub-second re-render, play it, export WAV and MIDI, drag them into Reaper, and have the MIDI trigger the same hits in a drum rack or Serum.

### v0.2: plugin and the monitor (about 6 weeks)
- `plugin`: nice-plug VST3 + CLAP, pinned; capture via the vocal track's input; shared process-wide worker pool; host state under 100 KB; drag-out via a vendored drag source started from baseview's own mouse handler; the app's panel reused as the editor.
- Causal monitor: FFT-YIN plus onset, provisional pitch with glide; about 20–25 ms at fmin 100 Hz.
- Reaper Lua action returns: selected item → app, latest export → take.
- `notes` lens with time-anchored edits, scale lock, quantise strength and swing, groove extraction from hit timing.

### v0.3 and v1
- Rings, Warps and Clouds-style ports; morph pad; sound matching; scat articulation (plosive → retrigger and accent, vowel continuation → legato); region reroll; "say it again" consensus; multi-task evidence model if DSP failed anywhere; AU via clap-wrapper; macOS notarisation; community dataset export with a CC0 contributor statement.

---

## 6. Risk register

| # | Risk | Earliest cheap test |
|---|---|---|
| 1 | Renders sound like a mouth, not a machine | Week 0 pre-check; week-4 gate at several Mouth amounts |
| 2 | Hit classification confuses your snare and hat past 3–4 roles | E1 with 5 and 10 shots; soft roles plus dice as the fallback |
| 3 | Double triggers and "b + oom" merges mis-time fills | Onset F1 at ±25 ms and refractory-window tuning in week 2 |
| 4 | The dice space is lumpy, so randomize frustrates | Palette ranges constrained to Plaits macros plus a few mangle depths; test by rolling 20 times and counting usable results |
| 5 | Mangle without the DAW's OTT and limiter sounds thin in isolation | Gate renders are judged in a Reaper session with the founder's usual master chain |
| 6 | Curve jitter and voicing flicker cause warble on pitched squeals | Jitter metric; hold-through-unvoiced with noise crossfade |
| 7 | Plugin UI broken in Reaper on one OS | Week-1 smoke spike |
| 8 | Ecosystem churn in egui, baseview, nice-plug | Pin commits; weekly canary; founder-reviewed patch forks |
| 9 | Edits do not survive re-derivation | One-page edit-semantics spec before `notes` exists |
| 10 | Mic and room dependence of brightness and breathiness | Two mics, two rooms in the golden set; percentile normalisation |
| 11 | Two-person bandwidth | Seven crates, two palettes, one window, and the week-4 gate are the guard |

---

## 7. Confirmations (resolved 2026-10-20)

1. **Training-data rule: pragmatic, not strict.** The founder's view is that weights-as-derivative is mostly ignored in practice and is not a concern at this stage. When a model is first shipped, prefer clean sets (VocalSet, Vocadito, NSynth, PTDB-TUG, the golden set) and retrain where cheap, but pretrained checkpoints are not forbidden. Record provenance in `LICENSES.md` either way.
2. **GPL-3.0-or-later, with Surge XT as an uncopied reference.** Confirmed.

---

## 8. Kickoff brief for the coding thread

> Building **Takedown**, an open-source (GPL-3.0-or-later) Rust tool: beatbox, hum or breathe into a mic and get designed electronic drum and texture sounds in the SOPHIE / Death Grips / JPEGMAFIA / Aphex Twin palette, plus MPE MIDI describing the same performance. Percussive and noisy sounds first; tonal input is an expression source, not transcription. v0.1 is a minimal eframe standalone (record or drop a take, evidence overlay, palette, six sliders, per-aspect dice with locks, A/B, play, export WAV + MPE to a folder). No looper, layers, piano roll or Reaper integration in v0.1; plugin via nice-plug in v0.2. Windows and macOS build in CI from day one; Reaper is the only test host.
>
> Architecture (`docs/MASTER-PLAN.md` §3–4 supersedes everything else in `docs/`): `Take` → parameter-free `Evidence` at 200 Hz plus `Hit` events → `lens(evidence, palette, settings, edits, seed tree)` → `ControlTrack` → voices (`RenderBackend` bodies + per-voice mangle chain + a VoiceMaterial layer from the take itself) → WAV; `ControlTrack` ↔ MPE `.mid` round-trips. The renderer never sees notes. Mangle (drive, fold, crush, filter scream, pitch envelope, transient shaping, ring-mod) is synthesis and lives in the engine; reverb, delay, compression and OTT are the DAW's. Seeds are a tree with per-aspect dice and locks; noise is counter-based. Project is a directory; audio is content-addressed.
>
> v0.1 crates: `core`, `evidence` (DSP only), `drums` (onset refinement, per-user kNN head, soft roles), `render` (conditioning, mod matrix, mangle chain, Plaits via git-pinned mi-plaits-dsp, VoiceMono, HarmonicNoise, NoiseTexture, VoiceMaterial), `midi`, `cli`, `app`, plus `research/eval` (mir_eval, golden set) and a week-1 plugin smoke spike. Week 0 is a two-day Python pre-check on ten beatbox fills. Week-4 gate: ten fills through two palettes, blind; continue if six are track-ready and one is a sound not otherwise makeable. Licence register from commit one: no Surge code copied, no non-commercial weights or training data, no tainted teachers.

