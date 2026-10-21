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