# Status

*2026-10-21, after the first unattended session (steps 1 to 6 of the kickoff list). Newest first in "Next"; numbers are reproducible with `research/README.md`.*

## Done

| Step | What landed | Where |
|---|---|---|
| 1 | Cargo workspace, seven crates, clippy pedantic and rustfmt configured | `Cargo.toml`, `clippy.toml`, `rustfmt.toml` |
| 2 | uv-pinned Python 3.13 eval env (mir_eval 0.8.2); `fetch_data.py` downloads Vocadito and AVP with size, MD5 and SHA-256 checks | `research/` |
| 3 | Core types from a judge panel (3 designs, 3 adversarial judges, synthesis), then implemented: time base, keys, seed tree, ControlTrack, views, edits, Take, Evidence and `.tkev`, interpretations, layers, project and migrations in `core`; the render contract (backend trait, segment plan, driver, offline render, testkit) in `render::contract` | `docs/design/core-types.md`, `crates/core`, `crates/render` |
| 4 | DSP evidence: pYIN candidates, separate pitch and voicing Viterbi with an octave-jump penalty, A-weighted loudness, centroid (Hz and harmonic units), tilt, smoothed CPP, SuperFlux onsets with energy backtracking; conversion to core `Evidence` | `crates/evidence` |
| 5 | `takedown analyze` (per-frame CSV plus onset candidates); `research/eval/score.py` scoring with mir_eval | `crates/cli`, `research/eval` |
| 6 | Not yet: CI is the next commit | |

Tests: 147 test functions, most of them property tests at 128 to 256 cases (core 89, render contract 21, evidence 35, cli 2), including the four CLAUDE.md requires: exact sample-to-control mapping, block-size invariance, chunked equals monolithic, seed-tree independence. The render suite was mutation-checked against seven deliberate bugs; each was caught.

## Numbers

All from `takedown analyze` with default settings, scored by `research/eval/score.py`. Both datasets are 44.1 kHz mono.

**Pitch, Vocadito** (40 solo singing excerpts, 13.6 min, mean over tracks):

| RPA | RCA | Voicing recall | Voicing false alarm | OA |
|---|---|---|---|---|
| 98.8 | 99.5 | 90.3 | 6.7 | 90.9 |

- Octave errors (RCA − RPA) 0.7 points. Worst track: vocadito_34 at 83.6 RPA, about 500 frames an octave low where the subharmonic trough wins.
- With white noise added (SNR against the voiced RMS): at 20 dB, RPA 97.9, VR 82.2, VFA 1.6; at 10 dB, RPA 92.8 but **VR 21.7**. pYIN's voiced probability collapses in noise and the fixed 0.5 voicing threshold rejects it. A per-take relative threshold recovers only half (VR 45).
- Against gate E1 (RPA ≥ 95, octave errors ≤ 2 points, VFA ≤ 10 on the golden set): passes on clean Vocadito. The golden set does not exist yet, and its fan-noise half will fail voicing as things stand.

**Onsets, AVP** (280 files, 9,869 annotated hits, 66 min):

| Subset | F1 ±25 ms | P | R | F1 ±50 ms |
|---|---|---|---|---|
| all | 90.0 | 86.8 | 93.5 | 92.3 |
| improvisations | 90.8 | 87.7 | 94.1 | 92.7 |
| isolated hits | 89.7 | 86.4 | 93.2 | 92.1 |

- Recall at ±25 ms by label: kick 94.6, snare 95.5, closed hat 92.6, open hat 90.6.
- Timing of onsets matched within ±50 ms: mean absolute error 3.2 ms, median 1.0 ms. docs/research/08 cites 11 ms mean deviation after refinement for the AVP paper's method; protocols were not compared in detail.
- Caveat: the post-max window, the backtracking rule and the default pick (60 ms refractory, a 25 dB rise needed within 100 ms of a kept onset) were chosen while looking at AVP, so these numbers are optimistic. The first pass (plain SuperFlux settings, 15%-of-peak backtracking) scored 61.0; most of the gain came from fixing onsets that landed 20 to 40 ms late on two-part sounds.
- Against gate E1 (onset F1 ≥ 0.9 on the golden set): on the line, on a dataset it was tuned on.

**Cost:** a 10-minute 48 kHz take analyses in 8.3 s single-threaded with 306 MB peak RSS (budget 600 MB).

## Assumptions

Every assumption is a dated entry in `docs/DECISIONS.md`. The ones most worth a look:
- **Core design.** Attempt A (renderer lens) won the panel with B's track shape and edits and C's persistence grafted on; the design's 33 assumptions are listed in one entry. Notable: no gate curve (the gate is spans with Detached, Retrigger and Legato links); hits and spans carry an anchor separate from their firing time; the render contract lives in `render`, not `core`; every persisted field is written and unknown fields are refused.
- **Deferred.** The MIDI projection (`quantise`, the MPE lattice, `reattach_anchors`) waits for the midi crate; its types exist because interpretations persist a `MidiBinding`.
- **Evidence.** Pitch candidates are stored and decoding is a pure function the lens calls; pitch and voicing are decoded separately (pYIN's joint model gave 27.6% voicing false alarms); f0 runs at capture rate rather than on a 16 kHz copy; evidence may hold NaN in the four columns where a value can be undefined.
- **Fixes made on the way.** The counter noise had a fixed zero at index 0 of stream 0 for every seed; two f32 values did not survive a JSON round trip through serde's enum paths, so all JSON now goes through `core::project::json`.

## Next

1. **Record the golden set** (docs/RECORDING-PROTOCOL.md). Every number above is a proxy until the founder's own hums, fills and noisy-room takes are scored; gate E1 is defined on them.
2. **Voicing in noise.** Fuse voiced probability with loudness relative to the take's floor and with CPP, or make the threshold adaptive per take; test on the fan-noise recordings. This is the one evidence curve that clearly fails today.
3. **Octave-down errors.** A decoder rule or prior for subharmonic troughs that are only marginally deeper than the fundamental's (vocadito_34).
4. **drums crate.** Onset refinement into `Hit` events using the stored strength and rise, the "b + oom" merge, per-user kNN roles; score per-user accuracy at 5 and 10 shots on AVP.
5. **lens and the first backend** in `render`, to reach the week-4 gate; then the midi crate with the deferred projection and its round-trip tests.
6. **Smaller items.** Pin the CI toolchain to a version if stable releases start failing clippy; parallelise or downsample f0 if analysis time matters in the app; persist the scoreboard baseline so CI can fail on regressions (docs/research/08 §7).
