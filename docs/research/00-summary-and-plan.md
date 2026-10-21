# Voice-to-Synth Tool: Survey Summary and Plan

> **Superseded.** After the founder discussion and four adversarial reviews (docs 05–08), the current plan is [MASTER-PLAN.md](MASTER-PLAN.md). This document is kept as the record of the initial survey synthesis; where it conflicts with the master plan, the master plan wins.

*Synthesis of four research passes run 2026-10-20. The detail lives in the companion documents:*
- [01-market-survey.md](01-market-survey.md): competitors, open-source blocks, patents, gap analysis
- [02-technical-feasibility.md](02-technical-feasibility.md): DSP/ML challenges, model choices, risks, Python-first prototyping order
- [03-platform-architecture.md](03-platform-architecture.md): framework decision, host behaviour, plugin formats, build/CI, architecture
- [04-product-concept.md](04-product-concept.md): users, workflow, controls, editor, sound engine, UI, open questions

---

## 1. The one-paragraph verdict

This is buildable, the market has a real gap, and the gap is exactly the thing you described: a **capture-first, offline-refined** voice instrument rather than a live controller. Every incumbent that converts voice to MIDI in real time (Dubler 2, Imitone) pays for it with double triggers, note jitter and "every result needs editing". Every offline converter (Ableton, Logic, Melodyne) gives one answer with no controls and is locked to its host. Nobody covers melody + drums + harmony in one tool that ships as both standalone and plugin, and nobody treats interpretation, randomness and subjectivity as a feature. The hard parts are perceptual (what counts as a note, which of your mouth sounds is a snare, whether "random" feels musical), not algorithmic. All core pieces exist as permissively licensed, CPU-friendly models.

**Positioning:** "The most accurate and musical voice-to-stem instrument." Hum it, beatbox it, gesture it, and get finished-sounding synth and drum parts plus clean MIDI, with three takes on what you meant.

---

## 2. What is out there (short form)

| Segment | Who | What they prove | What they miss |
|---|---|---|---|
| Live voice-to-MIDI | Vochlea Dubler 2 ($99), Imitone ($29–99, stagnant) | Demand exists; beatbox triggers work for 3–4 classes; "stickiness" and slide modes are the right mental models | Causal by design, so accuracy is capped; no AAX, no CLAP; Imitone is a 10-year beta |
| Beatbox capture loop | **Vochlea DubBox ($69, July 2026)**, Ala Shiban's open-source Beatbox | The capture → BPM detect → quantize-keeping-feel → overdub → export-stems loop is validated | Drums only, standalone only, sample triggering only. Vochlea will likely add pitch, so this is the clock you are racing |
| DAW built-ins | Ableton, Logic, Cubase, Studio Pro (Melodyne), FL Newtone | Offline conversion is table stakes | One answer, no controls, DAW-locked, bounce-and-convert workflow. Bitwig has nothing |
| Neural timbre transfer | Neutone Morpho ($99), RAVE, DDSP-VST (abandoned 2023), Combobulator | Users want "my voice becomes an instrument" | Audio only, no MIDI, ~46 ms model delay, quirky timbres, non-commercial weights |
| Offline AI pipelines | ACE Studio 2 ($398+), Kits.ai, Suno Studio | Vocal → MIDI → re-render is a workflow people pay for | Cloud or slow, weak instruments, messy MIDI. Udio losing downloads shows the risk of cloud dependence |
| Research to track | TRIA (beatbox → drum audio, ISMIR 2025), Sketch2Sound (vocal imitation conditioning), PESTO (sub-10 ms f0), AVP user embeddings | The "subjective mapping" idea is now feasible | None productized |

Six gaps nobody fills: capture-first across pitch and drums with plugin builds; accuracy from being non-causal; semantic mapping of vocal gesture; rendering beyond sample triggering; AAX and CLAP coverage; a maintained, local, commercially clean product.

---

## 3. Technical challenges, ranked

1. **Note segmentation that matches intent.** Scoops, glides, vibrato and same-pitch re-articulation ("da-da-da") are the hard cases. Use a pYIN-style HMM over a neural f0 contour; its parameters map directly to user-facing Sensitivity and Glide controls. Expect onset+pitch F1 around 0.8–0.9 on clean singing and lower on humming and scat. The editor and the "three interpretations" design exist because this will never be perfect.
2. **Cross-user drum classification.** Accuracy falls past 3–4 trained classes in every product. Plan: a frozen embedding model trained in-house on AVP plus your own data, a few-shot prototype head per user (5–15 examples per class), confidence gating, a refractory window for doubled sounds, and a UI that tells users 3–5 roles is the sweet spot.
3. **Monitoring latency.** 12–20 ms for sustained pitch above ~150 Hz is realistic. Low male voices are worse. Trigger on onset at a provisional pitch, then correct. The offline render is authoritative; the preview is a sketch.
4. **Licence contamination.** JUCE is AGPL or paid. PESTO is LGPL. madmom, RAVE, MuScriptor and Essentia models are non-commercial. GPL synth engines (Surge, Vital source) cannot be linked. Mitigation: a licence register from day one, sfizz (BSD) for samples, an in-house synth, retrained small models, JUCE Indie licence.
5. **Octave and voicing errors** on closed-mouth humming, falsetto and noisy rooms. Ensemble offline, Viterbi with an octave-jump penalty, per-user range calibration.
6. **ML runtime in a plugin.** Static, operator-reduced ONNX Runtime with hidden symbols plus RTNeural for tiny real-time models. Shared inference thread pool across instances. Weights as versioned hashed data files.
7. **Tempo and loop inference** on short rubato clips. Host tempo first; top-N candidates with half/double; user-editable loop points; quantize strength rather than hard snap.
8. **Making randomness feel musical.** Seeded, reproducible variation; sampling from model uncertainty rather than spraying noise; curated interpretation presets.

Two **patents** need a freedom-to-operate review before the drum classifier is designed: US11978473B1 (Bace Technologies, CNN classification of beatbox into kick/snare, to 2042) and US10861427B2 (Harman, vocal percussion to drum pattern, to ~2038). The embedding-plus-prototype design may be a design-around but a lawyer has to say so.

---

## 4. Decisions the research supports

**Framework: JUCE 9, C++20, over a JUCE-free core library.** JUCE is the only option that covers standalone + VST3 + AU + AAX from one codebase today, with CLAP via clap-juce-extensions until JUCE's native CLAP lands. The core (analysis, rendering, take model, serialisation) never includes JUCE, so it stays licence-clean, is testable from Python via nanobind, and can be re-shelled behind CLAP-native plus clap-wrapper if JUCE ever disappoints. Rust was considered seriously and rejected for now: no AU/AAX without a wrapper, immature plugin GUI toolkits, nih-plug in maintenance mode, and the ML runtime is C++ underneath anyway. Buy JUCE Indie ($800 perpetual, under $300k revenue).

**Two facts that changed since you last looked at this space:** VST3 became MIT-licensed in October 2025 (no Steinberg agreement needed), and ASIO went the other way (GPLv3 or proprietary agreement). AAX still needs Avid approval plus PACE/iLok signing.

**Plugin shape: one instrument plugin with an audio sidechain input, capture-based.** It records the sidechain into a take buffer, analyses offline, renders, and offers drag-out. Live MIDI output from a plugin is inconsistent across hosts (Ableton merges channels and AU has no MIDI out; Pro Tools routing is clunky), so **drag-and-drop of .mid and .wav is the universal path** and the standalone's virtual MIDI port is the second. CLAP hosts are Bitwig, Reaper, Studio One and FL; the big four DAWs still need VST3 and AU.

**Sound engine: in-house.** One excellent mono/poly synth (2 VA/wavetable oscillators, sub, noise, ladder/SVF/comb filters, mod matrix where voice features are first-class sources, drive only) plus an 8-slot drum engine (sample layer via sfizz plus a small synth layer per slot). No reverb, delay or chorus. Dry stems are the point. About 60 synth presets and 20 kits, organised by Idiom.

**Pitch stack:** PESTO-class streaming model for the live preview (retrained in-house or SwiftF0 to avoid LGPL), an ensemble of FCPE/RMVPE + PESTO with Viterbi for the offline pass, pYIN as DSP fallback. Decide after the Python bake-off, not before.

**Neural timbre transfer: not in v1.** Three of four passes agreed. Users report quality and setup friction, the weights are mostly non-commercial, and a conventional synth driven by voice features sounds better and stays editable. Leave an architectural slot (a "render backend" interface) so a DDSP-style engine trained in-house can be added later.

**OS order: Windows x64 first, macOS CI compiling from day one.** You develop on Windows and it is the largest hobbyist base. macOS, AU and notarization are v0.2.

---

## 5. The product in one screen

Six macro knobs, each with one job: **Fidelity** (literal ↔ interpretive), **Idiom** (the "producer's ear": Sub/808, Acid, Funk Bass, Pluck, Lead, Pad; Live Kit, Four-on-Floor, Boom-Bap, Trap, Breaks), **Variance** with Seed and Reroll, **Confidence** threshold, **Glide ↔ Retrigger**, **Timbre Follow**. A utility row covers Key/Scale lock, Octave, Tightness, Velocity curve, Drum Vocabulary and A/B.

Three ideas that make it feel different rather than "another audio-to-MIDI":
- **Interpretations as first-class objects.** Every result is settings + seed + edits, reproducible, nameable, and breedable (rhythm of B with pitches of C). Reroll re-imagines only the uncertain regions and keeps the confident notes.
- **"Say it again" consensus.** Hum the phrase two or three times; agreement raises confidence, disagreement becomes the pool the alternates draw from. The randomness is literally yours.
- **Your mouth is the patch.** Brightness → cutoff, breathiness → noise, vowel (F1/F2) → wavetable position via a per-preset Vowel X/Y pad. "oo" is dark, "ah" is open, "ee" is bright.

The editor draws your actual pitch curve faintly behind the transcribed notes. That is the single most important UX choice.

Modes: Melodic, Percussive, Gesture (control curves only in v1), Auto (default; splits one take into Notes and Hits lanes by event, not by source separation).

Explicitly **not in v1:** live zero-latency effect, any effects chains, DAW features beyond 8 layers, polyphonic voice input, neural timbre transfer, granular/texture mode, mobile, cloud.

---

## 6. Roadmap

**Phase 0: Python research spike (weeks 1–6).** No product code yet. Answers the three risks that decide everything else.
- Pitch + segmentation bake-off on Vocadito, VocalSet, MIR-1K and a small in-house hum/scat set, scored with mir_eval. Picks the shipping f0 model and tunes the HMM segmenter.
- Few-shot vocal-percussion classifier: onset detection options, MFCC+kNN vs frozen embedding + prototype head, 5/10/15 shots, cross-user, on AVP plus in-house recordings.
- Streaming simulation of the chosen models at real hop and buffer sizes to settle the preview latency/accuracy trade.
- A throwaway feature-to-synth mapping (Python or MIDI into an existing synth) to tune Timbre Follow and the randomness controls by ear.
- In parallel, paperwork: Avid/PACE developer registration, Apple Developer Program, Windows code signing, JUCE Indie, licence register, freedom-to-operate on the two patents.

**Phase 1: v0.1, Windows (weeks 6–20).** Standalone + VST3 + CLAP from the Pamplejuce template.
- Record a take (count-in, metronome, host sync in plugin; WASAPI shared/exclusive in standalone), offline analysis, Notes and Hits lanes, one Idiom set, three interpretations, Fidelity/Variance/Confidence/Glide knobs.
- Basic piano roll and drum grid with undo, quantize with strength and swing, key detection and lock.
- One synth, one kit, your own content. WAV and SMF export, drag-out to DAW.
- Plugin: instrument with sidechain input, capture-based. pluginval and clap-validator in CI.

**Phase 2: v0.2, macOS + expression.** Universal build, AU, notarization, CoreMIDI virtual port. Low-latency monitoring mode. Plugin MIDI output where hosts allow it. Timbre Follow, Teach screen with separability matrix, layering/overdub, Harmonize (rule-based: interval, chord tone, pad voicing, counter-bass), groove extract.

**Phase 3: v0.3.** MPE export and live MPE out. Windows virtual MIDI (Windows MIDI Services loopback, licensed virtualMIDI fallback). More Idioms, presets, model updater. Linux beta.

**Phase 4: v1.0.** AAX (PACE-signed, Pro Tools QA). Native JUCE CLAP when available. "Say it again" consensus takes. Accessibility pass, installers. ARA 2 and a DDSP-style render backend are v1.5 candidates.

---

## 7. Open questions for you

Resolving these before the coding thread saves rework. One-line answers are enough.

1. **Primary launch buyer:** electronic/hip-hop producer who can't play keys (recommended) or songwriter/topliner?
2. **Business model:** one-time licence around $99–149, or subscription? (Vochlea sits at $69–99, so pricing above that needs the plugin formats and accuracy to justify it.)
3. **All analysis on-device, no cloud dependency?** Recommended yes; it is a differentiator given Udio and Suno.
4. **Framework confirmation:** JUCE 9 C++ over a JUCE-free core, or do you have a strong Rust preference worth the extra risk?
5. **Windows first, macOS v0.2?** Or must both ship together?
6. **Content:** can v1 ship licensed third-party drum samples, or must every sound be recorded or synthesised in-house?
7. **Gesture mode outputting control curves only in v1:** acceptable?
8. **Layers cap of 8 per project:** enough for v1?
9. **Data policy:** opt-in sharing of corrections and audio to improve models, or a hard "audio never leaves the machine" promise?
10. **Team:** solo with AI assistance, or will you hire? This sets whether Phase 0 and Phase 1 overlap.
11. **Working name:** "Murmur" was a placeholder from the product pass and is not trademark-checked. Pick something before the repo is created.

---

## 8. Decisions from the founder discussion (2026-10-20)

These supersede the recommendations in sections 4 and 6 and in the companion documents where they conflict.

| Topic | Decision | Consequence |
|---|---|---|
| **Licensing** | Open source, as free/libre as possible. Proposed split: core crates MIT OR Apache-2.0, app and plugin GPLv3-or-later (pending confirmation) | JUCE AGPL cost and GPL synth-engine worries vanish. AAX is dropped (PACE signing is incompatible with open-source builds). Non-commercial model weights are still excluded: all shipped models are trained in-house on permissive data |
| **Host and formats** | Reaper on Windows and macOS. VST3 + CLAP + standalone. AU nice-to-have via clap-wrapper later | Both OSes are in v0.1. Reaper's unrestricted MIDI routing and scriptability simplify integration and CI. ReaTune's MIDI output is the day-one baseline to beat |
| **Language** | Rust, founder's preference, research now agrees | nice-plug shell, egui via egui-baseview and eframe, cpal, midir, tract for CPU inference, burn/candle with wgpu for GPU later, PyO3/maturin for research bindings. JUCE C++ recommendation in doc 03 is withdrawn |
| **Audience** | Built for the founder. No buyers, no pricing. Efficient, clean, effective first; polish later. Public on GitHub if others want it | No preset catalog, onboarding, or Teach-screen polish in v0.1. The product doc is a backlog, not a spec |
| **Voice-to-audio first, MIDI second** | Continuous feature curves drive the synth directly. Notes are an optional lens | Removes segmentation from the audio path's critical risk. Fidelity 0 = continuous resynthesis, 100 = quantized discrete notes. "Gesture mode" is no longer a separate mode |
| **Neural rendering** | Founder has training experience. DDSP-class renderer is a primary v1 backend, not an experiment | Two render backends behind one trait: deterministic synth and neural. PESTO-style tracker and drum embedding retrained in-house |
| **Hardware acceleration** | CPU for everything real-time. GPU later, for offline render and training, via burn/candle wgpu | No CUDA. No cloud, ever |
| **Drums** | No sampler. Synthesized classics plus an exciter/resonator architecture where the user's own mouth transient can be the exciter. Neural drum synthesis later for realism | Acoustic-kit realism deferred. Electronic kits are native to this design |
| **Layers** | Per-instrument stacking, materialized to disk for real editing in the DAW. No design cap | |
| **Community data** | Roadmap: local "export my corrections as a dataset" command plus a public dataset repo. No telemetry | |
| **Team** | Founder plus Claude. Outside contributions possible later | Phase 0 and Phase 1 run serially; the bake-off harness is written by Claude to compress Phase 0 |
| **Name** | TBD. Shortlist: Lilt (recommended), Takedown, Fenby, Copyist, Dictée | Trademark search before the repo goes public |

Revised workspace:

```
crates/
  evidence/    f0, loudness, brightness, breathiness, formants, onsets  (MIT OR Apache-2.0)
  notes/       HMM segmentation, quantize, key, harmonize              (MIT OR Apache-2.0)
  drums/       onset + few-shot classifier head                        (MIT OR Apache-2.0)
  render/      synth, exciter/resonator drums, DDSP-style neural; one RenderBackend trait
  model/       Take, Evidence, Interpretation, Layer; serde project format
  infer/       tract today, burn/candle later, behind one trait; hashed weights
  ui/          shared egui widgets: waveform + f0 overlay, piano roll, hit grid
  app/         standalone: eframe + cpal + midir                       (GPLv3)
  plugin/      nice-plug VST3 + CLAP shell                              (GPLv3)
  py/          PyO3 bindings
research/      PyTorch training and bake-off notebooks
models/        versioned weights with hashes
```

Revised v0.1 milestone: one honest end-to-end path. Record a take in the standalone, extract the evidence curves, render through one synth under continuous voice control, drag the WAV into Reaper. Notes, drums, interpretations and the plugin shell follow in that order.

## 9. Kickoff brief for the coding thread

Paste this at the top of the next session once the license split and name are confirmed.

> Building **<NAME>**, an open-source voice-to-synth/drum/MIDI desktop tool in Rust. Capture-first, offline-refined, not a live effect. Voice-to-audio is primary: continuous feature curves (f0, loudness, brightness, breathiness, formants) drive the synth directly, and discrete notes are an optional lens. Targets: standalone + VST3 + CLAP on Windows and macOS, Reaper as the primary host. Stack: Cargo workspace with permissive core crates (evidence, notes, drums, render, model, infer) and GPLv3 app/plugin crates; nice-plug for the plugin shell; egui via egui-baseview and eframe; cpal, midir, rtrb, arc-swap; tract for CPU inference with burn/candle wgpu planned for GPU; PyO3/maturin bindings for research. Render backends behind one trait: an in-house synth (VA + wavetable, ladder/SVF/comb, mod matrix with voice features as sources) and a DDSP-class neural renderer trained in-house. Drums are synthesized, with an exciter/resonator design where the mouth transient can be the exciter. No sampler, no effects chains, no cloud, no telemetry, no non-commercial model weights. Phase 0 is a Python research spike with a Rust-backed harness: pitch/segmentation bake-off (pYIN, PESTO, FCPE, SwiftF0, RMVPE, Basic Pitch on Vocadito/VocalSet/MIR-1K with mir_eval), few-shot beatbox classifier on AVP, streaming latency simulation, and a throwaway feature-to-synth mapping. v0.1 is one end-to-end path: record, extract evidence, render through one synth, drag the WAV into Reaper. Full research is in `docs/00` through `docs/04`; `docs/00` section 8 holds the decisions that supersede the rest.

**Verification caveats carried over from the research passes:** licences for RMVPE, FCPE, SwiftF0, SOME, BeatNet+, anira, PANNs/BEATs/LAION-CLAP weights and the RAVE checkpoints were not confirmed from primary sources. Vochlea's plugin formats came from a retailer listing. The note-level F1 ranges are literature estimates. Patent status is per Google Patents, not a legal opinion.
