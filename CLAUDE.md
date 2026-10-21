# Takedown

Open-source (GPL-3.0-or-later) Rust tool: beatbox, hum or breathe into a mic and get designed electronic drum and texture sounds in the SOPHIE / Death Grips / JPEGMAFIA / Aphex Twin palette, plus MPE MIDI describing the same performance. Built by Logan (founder, plays piano, uses Reaper on Windows and macOS) with Claude.

## Read first

- `docs/MASTER-PLAN.md` is authoritative. Where any other doc conflicts with it, the master plan wins.
- `docs/DECISIONS.md` is the append-only decisions log; add an entry whenever you make a call the code does not explain.
- `docs/research/05`–`08` are adversarial reviews; `06` (engine), `07` (types and threading) and `08` (DSP recipes, licences) hold detail the plan refers to. Open sections on demand. `docs/research/00`–`04` are the original survey and are superseded. Research docs are expected to rot; the maintained rules are CLAUDE.md, MASTER-PLAN.md, LICENSES.md and DECISIONS.md.

## Hard rules

- The renderer never sees notes. Backends take a `ControlTrack` only; the lens compiles everything.
- One object, two outputs: audio and MPE `.mid` describe the same performance and round-trip.
- `process()` is block-size invariant. Smoothing is a function of absolute sample index.
- Evidence is parameter-free observation; anything a slider changes is lens work.
- Seeds are a tree; noise is counter-based. Edits are time-anchored, never index-anchored.
- Nothing allocates on the audio thread.
- Mangle (drive, fold, crush, filter scream, pitch envelope, transient shaping, ring-mod) is synthesis and lives per voice. Reverb, delay, compression, OTT and EQ belong to the DAW. No global effects bus.
- Licence register in `LICENSES.md` from commit one. No code copied from Surge XT (reference only). Prefer models trained on clean, redistributable data and record provenance; pretrained checkpoints are allowed when the founder judges them fine (see MASTER-PLAN §7).
- No sample library, no cloud, no telemetry, no looper/layers/piano roll in the app, never AAX.

## Conventions

- Cargo workspace; crates under `crates/` (v0.1: core, evidence, drums, render, midi, cli, app). Research and eval in `research/` (uv-pinned Python, mir_eval). Models, when any exist, in `models/` with blake3 hashes.
- CI builds and tests on Windows and macOS from day one. Weekly `cargo update` canary.
- Git-pinned dependencies for mi-plaits-dsp, nice-plug and egui-baseview. Pin to commits, not branches.
- Control rate is 200 Hz, fixed. Audio time is integer samples at capture rate.
- Property tests for: sample↔control mapping, block-size invariance, chunked-equals-monolithic render, seed-tree independence.
- Reaper 7.x is the only test host until v0.2. Plugin (nice-plug VST3 + CLAP) is v0.2; v0.1 is the eframe standalone plus CLI.
- Clippy clean, `cargo fmt`, no `unsafe` without a comment saying why.

## Do not propose

JUCE or C++ shells, samplers, cloud or subscription features, note-transcription accuracy as a product goal, DAW-like features, commercial licensing, GPU on the real-time path.
