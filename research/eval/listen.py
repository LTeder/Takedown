"""Listening check for one take: a plot and two stems to play against it.

    uv run python eval/listen.py TAKE.wav [--zoom 55:61 --zoom 118:128 ...] [--out DIR]

Runs `takedown analyze` on the take and writes, under `out/listen/<take>/`
by default:

- `frames.csv`, `onsets.csv`: the analyzer's output.
- `check.png`: the whole take with the picked onsets over the waveform and
  the decoded pitch, plus a waveform panel and a pitch-over-spectrogram panel
  for each `--zoom START:END` (seconds).
- `onset-clicks.wav`: a 6 ms blip at every picked onset.
- `pitch-sine.wav`: a sine following the decoded f0 where it is voiced, at
  the take's loudness.

Both stems have the take's length and rate, so dropped at the start of a
Reaper project under the take they line up with it. Mismatches are easier to
hear than to see: a click with no hit under it, a hit with no click, or a
sine that jumps an octave or sounds where nothing is sung.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import warnings
from pathlib import Path

import matplotlib
import numpy as np
from scipy.io import wavfile
from scipy.signal import spectrogram

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "research" / "out" / "listen"
DEFAULT_BIN = REPO / "target" / "release" / ("takedown.exe" if sys.platform == "win32" else "takedown")

SURFACE, INK2, GRID = "#fcfcfb", "#52514e", "#e4e3df"
WAVE, ONSET, F0, UNVOICED = "#8a8984", "#eb6834", "#2a78d6", "#c9c8c2"


def read_mono(path: Path) -> tuple[int, np.ndarray]:
    """Samples in [-1, 1], channels averaged (as `takedown analyze` does)."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", wavfile.WavFileWarning)
        sr, x = wavfile.read(path)
    if x.dtype == np.uint8:
        x = (x.astype(np.float64) - 128.0) / 128.0
    elif np.issubdtype(x.dtype, np.integer):
        x = x.astype(np.float64) / -float(np.iinfo(x.dtype).min)
    else:
        x = x.astype(np.float64)
    return sr, (x.mean(axis=1) if x.ndim == 2 else x)


def write_stems(out: Path, sr: int, n: int, fr: np.ndarray, picked: np.ndarray) -> None:
    click = np.zeros(n)
    blip_len = int(0.006 * sr)
    blip = 0.5 * np.sin(2 * np.pi * 2000 * np.arange(blip_len) / sr) * np.hanning(blip_len)
    for s in picked["sample"].astype(int):
        e = min(n, s + blip_len)
        click[s:e] += blip[: e - s]

    voiced = fr["voiced"] == 1
    f0 = np.where(voiced, fr["f0_hz"], 0.0)
    amp = np.where(voiced, 10 ** ((np.clip(fr["loudness_db"], -60, 0) + 6) / 20), 0.0)
    idx = np.arange(n)
    f0_s = np.interp(idx, fr["sample"], f0)
    box = max(1, int(0.005 * sr))
    amp_s = np.convolve(np.interp(idx, fr["sample"], amp), np.ones(box) / box, mode="same")
    sine = np.sin(np.cumsum(2 * np.pi * f0_s / sr)) * np.clip(amp_s, 0, 0.5)

    for name, sig in (("onset-clicks", click), ("pitch-sine", sine)):
        wavfile.write(out / f"{name}.wav", sr, (np.clip(sig, -1, 1) * 32767).astype(np.int16))


def plot(out: Path, sr: int, x: np.ndarray, fr: np.ndarray, picked: np.ndarray, zooms: list[tuple[float, float]]) -> None:
    n = len(x)
    t_on = picked["sample"] / sr
    plt.rcParams.update({
        "font.size": 9, "axes.edgecolor": GRID, "axes.labelcolor": INK2, "xtick.color": INK2,
        "ytick.color": INK2, "axes.titlesize": 10, "axes.titleweight": "bold", "axes.titlelocation": "left",
    })

    def waveform(ax, a: float, b: float) -> None:
        seg = x[int(a * sr) : min(n, int(b * sr))]
        k = max(1, len(seg) // 1600)
        blk = seg[: len(seg) // k * k].reshape(-1, k)
        tt = a + np.arange(blk.shape[0]) * k / sr
        ax.fill_between(tt, blk.min(1), blk.max(1), color=WAVE, lw=0)
        sel = (t_on >= a) & (t_on < b)
        ax.vlines(t_on[sel], -1, 1, color=ONSET, lw=0.8)
        lim = max(1e-3, float(np.abs(seg).max())) * 1.05
        ax.set_ylim(-lim, lim)
        ax.set_xlim(a, b)
        ax.set_yticks([])
        ax.grid(axis="x", color=GRID, lw=0.6)
        ax.set_axisbelow(True)
        span = "Whole take" if a == 0 and b == n / sr else f"{a:g}–{b:g} s"
        ax.set_title(f"{span}: waveform and {int(sel.sum())} picked onsets (orange)")

    def pitch(ax, a: float, b: float, spec: bool) -> None:
        m = (fr["time_s"] >= a) & (fr["time_s"] < b)
        if spec:
            seg = x[int(a * sr) : min(n, int(b * sr))]
            nper = 1 << int(np.log2(0.085 * sr))
            f, t, s = spectrogram(seg, sr, nperseg=nper, noverlap=nper - int(0.01 * sr))
            keep = (f >= 40) & (f <= 2000)
            db = 10 * np.log10(s[keep] + 1e-14)
            ax.pcolormesh(a + t, f[keep], db, cmap="Greys", shading="auto", vmin=np.percentile(db, 40))
        v, u = m & (fr["voiced"] == 1), m & (fr["voiced"] == 0)
        if not spec:
            ax.scatter(fr["time_s"][u], np.clip(fr["cand1_hz"][u], 40, None), s=1, color=UNVOICED, lw=0,
                       label="best candidate, unvoiced")
        ax.scatter(fr["time_s"][v], fr["f0_hz"][v], s=2.5, color=F0, lw=0, label="f0, voiced")
        ax.set_yscale("log")
        ax.set_ylim(40, 1300)
        ax.set_xlim(a, b)
        ax.set_yticks([55, 110, 220, 440, 880], ["A1", "A2", "A3", "A4", "A5"])
        ax.minorticks_off()
        ax.grid(color=GRID, lw=0.6)
        ax.set_axisbelow(True)
        if spec:
            ax.set_title(f"{a:g}–{b:g} s: voiced f0 (blue) over the spectrogram")
        else:
            ax.set_title("Decoded pitch (blue where voiced; grey is the best candidate where unvoiced)")
            ax.legend(loc="upper left", frameon=False, markerscale=4, fontsize=8)

    rows = 2 + 2 * len(zooms)
    fig, axes = plt.subplots(rows, 1, figsize=(14, 2.3 * rows), facecolor=SURFACE, squeeze=False)
    axes = axes[:, 0]
    waveform(axes[0], 0.0, n / sr)
    pitch(axes[1], 0.0, n / sr, spec=False)
    for i, (a, b) in enumerate(zooms):
        waveform(axes[2 + 2 * i], a, b)
        pitch(axes[3 + 2 * i], a, b, spec=True)
    axes[-1].set_xlabel("seconds")
    fig.tight_layout(h_pad=1.6)
    fig.savefig(out / "check.png", dpi=110, facecolor=SURFACE)
    plt.close(fig)


def parse_zoom(text: str) -> tuple[float, float]:
    try:
        a, b = (float(v) for v in text.split(":"))
    except ValueError:
        raise argparse.ArgumentTypeError(f"expected START:END in seconds, got {text!r}") from None
    if not 0 <= a < b:
        raise argparse.ArgumentTypeError(f"need 0 <= START < END, got {text!r}")
    return a, b


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("wav", type=Path)
    ap.add_argument("--zoom", type=parse_zoom, action="append", default=[], metavar="START:END")
    ap.add_argument("--out", type=Path, help="default: research/out/listen/<take name>/")
    ap.add_argument("--bin", type=Path, default=DEFAULT_BIN)
    args = ap.parse_args()
    if not args.bin.exists():
        raise SystemExit(f"{args.bin} not found: run `cargo build --release -p takedown-cli` first")
    if not args.wav.is_file():
        raise SystemExit(f"{args.wav}: no such file")

    out = args.out or OUT / args.wav.stem
    out.mkdir(parents=True, exist_ok=True)
    frames, onsets = out / "frames.csv", out / "onsets.csv"
    cmd = [str(args.bin), "analyze", str(args.wav), "--frames", str(frames), "--onsets", str(onsets)]
    if subprocess.run(cmd, stdout=subprocess.DEVNULL).returncode != 0:
        raise SystemExit("takedown analyze failed (its error is above)")

    sr, x = read_mono(args.wav)
    fr = np.genfromtxt(frames, delimiter=",", names=True)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)  # a header-only onsets file is "empty input"
        on = np.atleast_1d(np.genfromtxt(onsets, delimiter=",", names=True))
    picked = on[on["picked"] == 1] if on.size and on.dtype.names else np.zeros(0, dtype=[("sample", float)])
    end = len(x) / sr
    zooms = [(a, min(b, end)) for a, b in args.zoom if a < end]

    write_stems(out, sr, len(x), fr, picked)
    plot(out, sr, x, fr, picked, zooms)
    voiced = float(np.mean(fr["voiced"] == 1)) * 100
    print(f"{args.wav.name}: {end:.1f} s, {len(picked)} picked onsets, {voiced:.0f}% of frames voiced")
    print(f"wrote {out}/{{check.png, onset-clicks.wav, pitch-sine.wav, frames.csv, onsets.csv}}")


if __name__ == "__main__":
    main()
