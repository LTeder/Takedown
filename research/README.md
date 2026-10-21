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

The local-only sets from `docs/research/08` §7 (MIR-1K, MDB-stem-synth, MedleyDB-Pitch) are not fetched: their licences rule out redistribution and CI.
