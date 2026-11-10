# Status

*2026-10-21, after the first unattended session (steps 1 to 6 of the kickoff list); updated 2026-11-09 with the founder's first take and review, and 2026-11-10 with the sound sketch and the re-evaluation. Newest first in "Next"; numbers are reproducible with `research/README.md`.*

## Done

| Step | What landed | Where |
|---|---|---|
| 1 | Cargo workspace, seven crates, clippy pedantic and rustfmt configured | `Cargo.toml`, `clippy.toml`, `rustfmt.toml` |
| 2 | uv-pinned Python 3.13 eval env (mir_eval 0.8.2); `fetch_data.py` downloads Vocadito and AVP with size, MD5 and SHA-256 checks | `research/` |
| 3 | Core types from a judge panel (3 designs, 3 adversarial judges, synthesis), then implemented: time base, keys, seed tree, ControlTrack, views, edits, Take, Evidence and `.tkev`, interpretations, layers, project and migrations in `core`; the render contract (backend trait, segment plan, driver, offline render, testkit) in `render::contract` | `docs/design/core-types.md`, `crates/core`, `crates/render` |
| 4 | DSP evidence: pYIN candidates, separate pitch and voicing Viterbi with an octave-jump penalty, A-weighted loudness, centroid (Hz and harmonic units), tilt, smoothed CPP, SuperFlux onsets with energy backtracking; conversion to core `Evidence` | `crates/evidence` |
| 5 | `takedown analyze` (per-frame CSV plus onset candidates); `research/eval/score.py` scoring with mir_eval | `crates/cli`, `research/eval` |
| 6 | GitHub Actions: fmt, clippy, build, test on macOS and Windows; weekly `cargo update` canary; toolchain pinned to 1.99.1 after the first run showed CI's newer clippy | `.github/workflows/ci.yml`, `rust-toolchain.toml` |

Tests: 164 test functions, most of them property tests at 128 to 256 cases (core 101, render contract 26, evidence 35, cli 2), including the four CLAUDE.md requires: exact sample-to-control mapping, block-size invariance, chunked equals monolithic, seed-tree independence. The render suite was mutation-checked against seven deliberate bugs; each was caught, and a 5,000-case soak passes. Two independent reviews followed the build, one of the evidence crate and one of the merged core and render code; their findings are fixed (a layered hit's seed depended on its sibling's timing, newer document versions were not refused, MIDI bindings were not validated, duplicate JSON keys collapsed silently, and smaller items), each recorded in DECISIONS.md. CI is green on macOS and Windows.

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

**Founder's first take** (2026-11-09; 179 s at 48 kHz on the MacBook mic in a small room with a ceiling fan running and a window open: beatbox, hums, a whistle and a passage of noise). Not scored, since there is no annotation; the founder listened to it against the click and pitch stems that `research/eval/listen.py` now makes.
- **Hums: accurate by ear.** The pitch sine sounded like a heavily filtered copy of the voice; f0 follows glides with no octave errors in the hummed sections.
- **Onsets: accurate in the beatbox passages,** with false positives in chaotic stretches (noise, irregular sounds, hums mixed in) and in very quiet ones. The founder's verdict: a good job that could be reined in a little, depending on use. Measured: 13 picked onsets peak less than 20 dB above the take's floor (39 under 25 dB). The 14 s of noise at the end got 90 onsets that peak almost as high as real hits (28 dB above the floor against 32) but rise less (mean 25 dB against 32) and are weaker (7% against 43% have SuperFlux strength over 5). A level floor would catch the quiet ones; the noise needs rise and strength, which evidence already stores.
- **Voicing on a room tone.** A steady tone near 59 Hz, most likely the ceiling fan, is called voiced in pauses: 1,453 frames (7 s) below 62 Hz at a median loudness of -71 dB. Same root cause as voicing in noise above.
- **Whistle:** tracked where it stays under the decoder's 1,100 Hz ceiling, unvoiced above it.

## Sound sketch (2026-11-10)

The first thing in the repository that makes sound. `research/week0/render.py` plays a take through three palettes synthesised in `research/week0/sketch.py` (rubber, crushed, struck), with three dice rolls each, and writes a listening page that switches between the take and every render in place. It is throwaway Python, built to find out whether sounds designed by Claude and driven by the founder's mouth are exciting before any Rust engine exists.

- **Rendered:** the founder's first take, 179 s: 355 hits (80 kick, 135 snare, 140 hat), 38 phrase accents, 43 onsets dropped, 60 s of pitched phrase. Nine renders plus a control take about half a minute.
- **Checked by measurement, not by ear** (Claude cannot hear): attacks reach half peak 0.4 ms after the onset in rubber and crushed; all files at -16 LUFS; under 0.5% of samples in the soft-clip zone; spectrograms show the kicks' pitch drops, the phrase voice following the hum's glides, and each palette's character. Whether any of it sounds good is the founder's call and has not been made.
- **Roles:** a hit is a kick if it lifts the 60 to 200 Hz band above that band's floor; the rest split into snare and hat by brightness. Against AVP's labels: 61% overall, 92% of kicks found (brightness alone: 53%). The founder's kicks are breathy, so brightness alone had filed most of them under hat.
- **The control render** gives each hit another hit's loudness, brightness, length and role. If it sounds as good as the real render, the performance is not audible in the result.

## Assumptions

Every assumption is a dated entry in `docs/DECISIONS.md`. The ones most worth a look:
- **Core design.** Attempt A (renderer lens) won the panel with B's track shape and edits and C's persistence grafted on; the design's 33 assumptions are listed in one entry. Notable: no gate curve (the gate is spans with Detached, Retrigger and Legato links); hits and spans carry an anchor separate from their firing time; the render contract lives in `render`, not `core`; every persisted field is written and unknown fields are refused.
- **Deferred.** The MIDI projection (`quantise`, the MPE lattice, `reattach_anchors`) waits for the midi crate; its types exist because interpretations persist a `MidiBinding`.
- **Evidence.** Pitch candidates are stored and decoding is a pure function the lens calls; pitch and voicing are decoded separately (pYIN's joint model gave 27.6% voicing false alarms); f0 runs at capture rate rather than on a 16 kHz copy; evidence may hold NaN in the four columns where a value can be undefined.
- **Fixes made on the way.** The counter noise had a fixed zero at index 0 of stream 0 for every seed; two f32 values did not survive a JSON round trip through serde's enum paths, so all JSON now goes through `core::project::json`.

## Next

Reordered on 2026-11-10 after the re-evaluation (`docs/research/09-re-evaluation.md`). Sound first; `core` is frozen until something uses it.

1. **The founder listens to the sketch** and gives a verdict per render: open `research/out/week0/<take>/index.html` after running `research/week0/render.py` on a take. This is the Week 0 check from MASTER-PLAN §5, with Claude's sounds in place of the founder's synths. If nothing excites, the plan says stop.
2. **Fix the gain-dependent onset detector** (recall falls from 0.93 to 0.42 when the audio is 20 dB quieter) and add one primitive to evidence: level above the take's own noise floor. Tests that vary gain and that start in room tone. The floor also fixes the fan tone called voiced and the false onsets in quiet stretches.
3. **Iterate the sketch by ear:** a texture voice for breath and noise (the noise passage is material, not error), continuous mapping in place of hard roles, more of the mouth as exciter. Compare Plaits blind against purpose-built voices before depending on it.
4. **Port what wins to Rust** behind the render contract, with a thin lens and `cli render`. Let that slice reshape `HitEvent` and the sliders; add the session glue (mix, play, stems per role) then. The drums crate grows out of this, not ahead of it.
5. **Re-baseline MASTER-PLAN** after the verdict: dates, a week-4 gate with a control condition and recorded reasons, a risk register that names the founder's listening time.
6. **Golden set, reordered.** Record the raw material when convenient (three input levels, a throat-bass item, two seconds of room tone first); annotate only what a decision needs.
7. **Smaller items.** The confirmed defects in 09 (edit edge cases, hit-cache validation, onset at 0 in room tone, evidence cache key, `score.py --threshold`); patch `core-types.md` where the code departs from it; decide the live monitor's causal path before the lens depends on offline shapes; bump the toolchain when 1.100 ships.
