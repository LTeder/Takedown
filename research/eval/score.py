"""Score `takedown analyze` CSV output against reference annotations with mir_eval.

Single files:
    uv run python eval/score.py pitch  FRAMES.csv REF_F0.csv
    uv run python eval/score.py onsets ONSETS.csv REF_ONSETS.csv [--window 0.025]

Whole datasets (runs the CLI on every file, then scores; needs fetch_data.py first):
    uv run python eval/score.py vocadito [--bin ../target/release/takedown]
    uv run python eval/score.py avp      [--bin ../target/release/takedown]

The batch modes write per-file CSVs and a scoreboard (JSON + Markdown) under
research/out/<dataset>/ and print the Markdown table.

CSV contract (written by `takedown analyze IN.wav --frames F.csv --onsets O.csv`):
  frames: header row; columns used here are time_s, f0_hz (decoded pitch, > 0 on
          every frame where the decoder has one) and voiced (0 or 1).
  onsets: header row; columns time_s, strength, picked (0 or 1). Candidates with
          picked == 1 are the CLI's default decision; --threshold re-picks on
          strength instead, so thresholds can be swept without re-analysis.
Reference CSVs have no header: Vocadito f0 is (time_s, hz with 0 = unvoiced),
AVP onsets are (time_s, label).
"""

from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
from pathlib import Path

import mir_eval
import numpy as np

RESEARCH = Path(__file__).resolve().parent.parent
REPO = RESEARCH.parent
DATA = RESEARCH / "data"
OUT = RESEARCH / "out"
DEFAULT_BIN = REPO / "target" / "release" / ("takedown.exe" if sys.platform == "win32" else "takedown")


# ---------------------------------------------------------------- readers


def read_header_csv(path: Path) -> dict[str, np.ndarray]:
    with path.open(newline="") as f:
        rows = list(csv.reader(f))
    if not rows:
        raise SystemExit(f"{path}: empty")
    header, body = rows[0], [r for r in rows[1:] if r]
    cols = list(zip(*body)) if body else [() for _ in header]
    # Empty fields are undefined values (NaN), e.g. brightness on silent frames.
    return {name: np.asarray([float(v) if v else np.nan for v in col]) for name, col in zip(header, cols)}


def read_plain_csv(path: Path) -> list[list[str]]:
    """Headerless reference CSV; tolerates CRLF, stray spaces and blank lines."""
    with path.open(newline="") as f:
        return [[c.strip() for c in row] for row in csv.reader(f) if row and row[0].strip()]


def est_melody(frames: dict[str, np.ndarray]) -> tuple[np.ndarray, np.ndarray]:
    """mir_eval convention: negative frequency = unvoiced frame with a pitch guess."""
    f0 = frames["f0_hz"]
    voiced = frames["voiced"] > 0.5
    return frames["time_s"], np.where(voiced, f0, -np.abs(f0))


def est_onsets(onsets: dict[str, np.ndarray], threshold: float | None) -> np.ndarray:
    if "time_s" not in onsets or onsets["time_s"].size == 0:
        return np.zeros(0)
    keep = onsets["picked"] > 0.5 if threshold is None else onsets["strength"] >= threshold
    return np.sort(onsets["time_s"][keep])


# ---------------------------------------------------------------- metrics


def pitch_scores(frames_csv: Path, ref_csv: Path) -> dict[str, float]:
    ref = np.asarray(read_plain_csv(ref_csv), dtype=float)
    est_t, est_f = est_melody(read_header_csv(frames_csv))
    s = mir_eval.melody.evaluate(ref[:, 0], ref[:, 1], est_t, est_f)
    return {
        "RPA": s["Raw Pitch Accuracy"],
        "RCA": s["Raw Chroma Accuracy"],
        "VR": s["Voicing Recall"],
        "VFA": s["Voicing False Alarm"],
        "OA": s["Overall Accuracy"],
    }


def onset_counts(est: np.ndarray, ref: np.ndarray, window: float) -> tuple[int, int, int]:
    """(true positives, estimates, references) under mir_eval's one-to-one matching."""
    if est.size == 0 or ref.size == 0:
        return 0, est.size, ref.size
    return len(mir_eval.util.match_events(ref, est, window)), est.size, ref.size


def prf(tp: int, n_est: int, n_ref: int) -> tuple[float, float, float]:
    p = tp / n_est if n_est else 0.0
    r = tp / n_ref if n_ref else 0.0
    return (2 * p * r / (p + r) if p + r else 0.0), p, r


def onset_scores(onsets_csv: Path, ref_csv: Path, window: float, threshold: float | None) -> dict[str, float]:
    ref = np.sort(np.asarray([float(r[0]) for r in read_plain_csv(ref_csv)]))
    est = est_onsets(read_header_csv(onsets_csv), threshold)
    tp, n_est, n_ref = onset_counts(est, ref, window)
    f, p, r = prf(tp, n_est, n_ref)
    return {"F": f, "P": p, "R": r, "tp": tp, "n_est": n_est, "n_ref": n_ref}


# ---------------------------------------------------------------- batch


def analyze(binary: Path, wav: Path, frames: Path, onsets: Path) -> None:
    frames.parent.mkdir(parents=True, exist_ok=True)
    cmd = [str(binary), "analyze", str(wav), "--frames", str(frames), "--onsets", str(onsets)]
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)


def write_board(name: str, board: dict, table: str) -> None:
    out = OUT / name
    out.mkdir(parents=True, exist_ok=True)
    (out / "scoreboard.json").write_text(json.dumps(board, indent=2) + "\n")
    (out / "scoreboard.md").write_text(table + "\n")
    print(table)
    print(f"\nwrote {out.relative_to(RESEARCH)}/scoreboard.{{json,md}}")


def run_vocadito(binary: Path) -> None:
    root = DATA / "vocadito" / "zenodo_data"
    if not root.is_dir():
        raise SystemExit("Vocadito missing: run `uv run python fetch_data.py vocadito`")
    wavs = sorted((root / "Audio").glob("vocadito_*.wav"), key=lambda p: int(p.stem.split("_")[1]))
    per_track = {}
    for wav in wavs:
        frames = OUT / "vocadito" / "csv" / f"{wav.stem}.frames.csv"
        onsets = frames.with_name(f"{wav.stem}.onsets.csv")
        analyze(binary, wav, frames, onsets)
        per_track[wav.stem] = pitch_scores(frames, root / "Annotations" / "F0" / f"{wav.stem}_f0.csv")
    keys = ["RPA", "RCA", "VR", "VFA", "OA"]
    mean = {k: float(np.mean([t[k] for t in per_track.values()])) for k in keys}
    worst = sorted(per_track, key=lambda t: per_track[t]["RPA"])[:5]
    lines = [
        f"Vocadito, {len(wavs)} tracks, mean over tracks",
        "",
        "| " + " | ".join(keys) + " |",
        "|" + "---|" * len(keys),
        "| " + " | ".join(f"{100 * mean[k]:.1f}" for k in keys) + " |",
        "",
        "Lowest RPA: " + ", ".join(f"{t} {100 * per_track[t]['RPA']:.1f}" for t in worst),
    ]
    write_board("vocadito", {"mean": mean, "tracks": per_track}, "\n".join(lines))


def run_avp(binary: Path, threshold: float | None) -> None:
    root = DATA / "avp" / "AVP_Dataset"
    if not root.is_dir():
        raise SystemExit("AVP missing: run `uv run python fetch_data.py avp`")
    windows = [0.025, 0.050]
    files = {}
    by_label: dict[str, list[int]] = {}  # label -> [found within 25 ms, total]
    deviations = []  # |est - ref| in ms for pairs matched within 50 ms
    for wav in sorted(root.glob("*/Participant_*/*.wav")):
        rel = wav.relative_to(root)
        frames = OUT / "avp" / "csv" / rel.with_suffix(".frames.csv")
        onsets = OUT / "avp" / "csv" / rel.with_suffix(".onsets.csv")
        analyze(binary, wav, frames, onsets)
        rows = sorted((float(r[0]), r[1] if len(r) > 1 else "?") for r in read_plain_csv(wav.with_suffix(".csv")))
        ref = np.asarray([t for t, _ in rows])
        est = est_onsets(read_header_csv(onsets), threshold)
        files[str(rel)] = {f"{int(w * 1000)}ms": onset_counts(est, ref, w) for w in windows}
        found = {i for i, _ in mir_eval.util.match_events(ref, est, 0.025)} if est.size and ref.size else set()
        for i, (_, label) in enumerate(rows):
            counts = by_label.setdefault(label, [0, 0])
            counts[0] += i in found
            counts[1] += 1
        if est.size and ref.size:
            deviations += [abs(est[j] - ref[i]) * 1000 for i, j in mir_eval.util.match_events(ref, est, 0.050)]

    def summary(select) -> dict[str, dict[str, float]]:
        chosen = [v for k, v in files.items() if select(k)]
        res = {}
        for w in windows:
            key = f"{int(w * 1000)}ms"
            tp, ne, nr = (sum(c[key][i] for c in chosen) for i in range(3))
            f, p, r = prf(tp, ne, nr)
            macro = float(np.mean([prf(*c[key])[0] for c in chosen])) if chosen else 0.0
            res[key] = {"F": f, "P": p, "R": r, "F_macro": macro, "n_files": len(chosen), "n_ref": nr}
        return res

    groups = {
        "all": lambda k: True,
        "improvisation": lambda k: "Improvisation" in k,
        "isolated hits": lambda k: "Improvisation" not in k,
    }
    board = {
        "threshold": threshold,
        "groups": {g: summary(s) for g, s in groups.items()},
        "recall_25ms_by_label": {k: v[0] / v[1] for k, v in sorted(by_label.items()) if v[1] >= 20},
        "deviation_ms": {"mean": float(np.mean(deviations)), "median": float(np.median(deviations))},
        "files": files,
    }
    lines = [
        f"AVP onsets, {len(files)} files, pooled counts (F_macro = mean of per-file F)",
        "",
        "| Subset | Window | F | P | R | F_macro | Refs |",
        "|---|---|---|---|---|---|---|",
    ]
    for g, res in board["groups"].items():
        for key, s in res.items():
            lines.append(
                f"| {g} | ±{key} | {100 * s['F']:.1f} | {100 * s['P']:.1f} | {100 * s['R']:.1f} "
                f"| {100 * s['F_macro']:.1f} | {s['n_ref']} |"
            )
    lines += [
        "",
        "Recall at ±25ms by label: "
        + ", ".join(f"{k} {100 * v:.1f}" for k, v in board["recall_25ms_by_label"].items()),
        f"Timing of onsets matched within ±50ms: mean |error| {board['deviation_ms']['mean']:.1f} ms, "
        f"median {board['deviation_ms']['median']:.1f} ms",
    ]
    write_board("avp", board, "\n".join(lines))


# ---------------------------------------------------------------- cli


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("pitch", help="RPA, RCA, voicing recall and false alarm for one file")
    p.add_argument("frames", type=Path)
    p.add_argument("ref", type=Path)
    o = sub.add_parser("onsets", help="onset F-measure for one file")
    o.add_argument("onsets", type=Path)
    o.add_argument("ref", type=Path)
    o.add_argument("--window", type=float, default=0.025)
    o.add_argument("--threshold", type=float, default=None)
    v = sub.add_parser("vocadito", help="run the CLI over Vocadito and score pitch")
    v.add_argument("--bin", type=Path, default=DEFAULT_BIN)
    a = sub.add_parser("avp", help="run the CLI over AVP and score onsets")
    a.add_argument("--bin", type=Path, default=DEFAULT_BIN)
    a.add_argument("--threshold", type=float, default=None)
    args = ap.parse_args()

    if args.cmd == "pitch":
        print(json.dumps(pitch_scores(args.frames, args.ref), indent=2))
    elif args.cmd == "onsets":
        print(json.dumps(onset_scores(args.onsets, args.ref, args.window, args.threshold), indent=2))
    else:
        if not args.bin.is_file():
            raise SystemExit(f"{args.bin} not found: run `cargo build --release -p takedown-cli` first")
        if args.cmd == "vocadito":
            run_vocadito(args.bin)
        else:
            run_avp(args.bin, args.threshold)


if __name__ == "__main__":
    main()
