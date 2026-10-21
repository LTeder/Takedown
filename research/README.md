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

The local-only sets from `docs/research/08` §7 (MIR-1K, MDB-stem-synth, MedleyDB-Pitch) are not fetched: their licences rule out redistribution and CI.
