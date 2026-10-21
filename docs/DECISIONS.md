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
