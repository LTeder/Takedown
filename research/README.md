# research/

Python evaluation harness for the Rust analyzers. Python never runs inside Takedown; it only scores what the `takedown` CLI writes, so the numbers stay comparable with the MIR literature (mir_eval).

## Setup

```bash
cd research
uv sync
uv run python fetch_data.py
```

`uv sync` builds `.venv/` from `uv.lock` (Python 3.13, mir_eval 0.8.2). `fetch_data.py` downloads the datasets below into `data/` (gitignored), checks size, MD5 and SHA-256, and unpacks each into its own directory. It is idempotent; `--force` re-downloads.

| Dataset | Zenodo | Licence | Contents | Use |
|---|---|---|---|---|
| Vocadito | [5578259](https://zenodo.org/records/5578259) | CC BY 4.0 | 40 solo vocal excerpts, 44.1 kHz mono, f0 at ~5.8 ms hop, notes from two annotators | f0 and voicing (RPA, RCA, VR, VFA) |
| AVP | [3245959](https://zenodo.org/records/3245959) | CC BY 4.0 | 28 participants, beatbox hits and improvisations, 44.1 kHz mono, onset times with labels | Onset F1 at ±25 ms; later, hit classification |

Layout after fetching:

```
data/archives/         the verified zips
data/vocadito/zenodo_data/{Audio,Annotations/F0,Annotations/Notes}
data/avp/AVP_Dataset/{Personal,Fixed}/Participant_N/PN_<Kind>_<Set>.{wav,csv}
```

## Scoring

```bash
cargo build --release -p takedown-cli
cd research
uv run python eval/score.py vocadito   # pitch: RPA, RCA, voicing recall, false alarm, OA
uv run python eval/score.py avp        # onsets: F1/P/R at ±25 and ±50 ms, recall by hit label
```

Each batch mode runs `takedown analyze` on every file, writes the CSVs under `out/<dataset>/csv/` (gitignored) and a scoreboard (`scoreboard.json`, `scoreboard.md`) under `out/<dataset>/`. Single files:

```bash
uv run python eval/score.py pitch  FRAMES.csv REF_F0.csv
uv run python eval/score.py onsets ONSETS.csv REF.csv --window 0.025 [--threshold 1.5]
```

Pitch follows the MIREX convention: frames the decoder calls unvoiced still report their held pitch as a negative frequency, so RPA and RCA measure the pitch track independently of the voicing decision. Onset scores pool true positives, estimates and references over all files (`F`) and also average per-file F (`F_macro`). `--threshold` re-picks candidates by strength alone, without the refractory and continuation rules of the default pick, so threshold sweeps need no re-analysis.

## Week 0 listening test

```bash
cargo build --release -p takedown-cli
cd research
uv run python week0/render.py TAKE.wav
open out/week0/TAKE/index.html
```

`week0/render.py` plays a take through three palettes synthesised in `week0/sketch.py` (rubber, crushed, struck), three dice rolls each, and writes them with a listening page to `out/week0/<take>/`. It is throwaway Python: the first sound in the project, built to find out whether sounds designed here and driven by a mouth are exciting before the Rust engine exists. Both files' docstrings say how a take becomes hits and a phrase, and what each voice is.

**Listening.** Open `index.html` from disk. One transport plays the take or any render; click a row or press its number to switch without losing your place, space to play and pause, arrows to skip. "Blind" hides the names and shuffles the order. Mark each render (would use, close, no), tick the ones you could not have made another way, write notes, and "Copy verdict" gives text to paste back. One row is a control: the same hit times with every hit's loudness, brightness, length and role swapped for another hit's. If the control sounds as good as the real render, the performance is not coming through.

**What it is not.** No bus effects, mono, and levels matched to -16 LUFS for fair comparison; judge it in a session with your usual master chain as well. Roles (kick, snare, hat) are guessed without teaching and are right about 61% of the time on AVP, though 92% of kicks are found.

**The decision.** If nothing excites you, the plan says stop. If something does, say what: which stretch, which palette, what made it work.

**Round 2: loops.** The founder found round 1 unmusical and not rhythmic enough, so round 2 renders a loop instead of the whole take:

```bash
uv run python week0/loop.py TAKE.wav                 # every palette, 2 rolls each
uv run python week0/loop.py TAKE.wav --palettes crushed westcoast --rolls 3
uv run python week0/measure.py out/week0/TAKE-loop   # numbers per render, since Claude cannot listen
```

`week0/loop.py` finds the tempo and swing of the beatboxed stretches, takes the tightest two bars as the pattern and another two as a fill, names the snare by what lands on beats 2 and 4, turns the most melodic hummed phrase into a bassline in its key that plucks on the kicks, and renders 8 bars through each palette, next to your own sounds re-sequenced into the same loop. Palettes are the three round-1 ones in `week0/sketch.py` plus one module per palette in `week0/palettes/`; a palette can also render the whole loop itself (pattern transformations, plugin hosting). `week0/measure.py` explains its columns in its docstring: they catch broken, smeared or performance-deaf renders, and say nothing about whether a render sounds good.

`week0/precheck.py` is the earlier version of this test: it writes drum MIDI and MPE for your own synths instead of audio (`fills` and `hums` subcommands; teach takes with `--kick --snare --hat` raise role accuracy to about 73%). It still works and nothing depends on it.

## Listening check

```bash
uv run python eval/listen.py TAKE.wav --zoom 55:61 --zoom 118:128
```

For takes with no annotation, such as the founder's own. Runs `takedown analyze` and writes to `out/listen/<take>/` a plot (`check.png`: picked onsets over the waveform and the decoded pitch, for the whole take and each `--zoom START:END` in seconds) and two stems with the take's length and rate: `onset-clicks.wav` (a blip per picked onset) and `pitch-sine.wav` (a sine on the voiced f0). Put both under the take at the start of a Reaper project and listen for clicks with no hit, hits with no click, and a sine that jumps or sounds where nothing is sung.

The local-only sets from `docs/research/08` §7 (MIR-1K, MDB-stem-synth, MedleyDB-Pitch) are not fetched: their licences rule out redistribution and CI.
