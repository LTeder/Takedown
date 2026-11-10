"""Measure loop renders, since Claude cannot listen to them.

    uv run python week0/measure.py LOOP_DIR [NAME ...] [--png OUT.png]

Reads `loop.pkl` and `<NAME>.wav` from LOOP_DIR (every WAV there if no
names are given) and prints one row per render. None of these numbers say
whether a render sounds good; they catch renders that are broken, smeared or
deaf to the performance before the founder spends time on them. Compare
against `mouth-kit` (the founder's own sounds in the same loop).

  peak       sample peak of the listening file (they are matched to -16 LUFS,
             then soft-clipped above 0.7; a peak near 0.98 means heavy clipping)
  clip%      samples above 0.95
  <100 ...   share of power below 100 Hz, 100 Hz-1 kHz, 1-8 kHz, above 8 kHz
  punch      median over hits of the 5 ms RMS peak in the 25 ms after the hit
             minus the mean level in the 30 ms before it, dB. Low means hits
             do not stand out from what is already sounding (smear, drone).
  depth      10th to 95th percentile of the 10 ms RMS level, dB. Low means
             a wall of sound with no space between hits.
  drone%     share of time the 30-150 Hz band is within 10 dB of its loudest.
             High means the low end never lets go.
  ongrid%    share of spectral-flux energy within 20 ms of a hit or bass note
             in the loop. Low means transients the pattern did not ask for.
  vel~       Spearman correlation between each hit's performed velocity and
             its rendered peak, within role. Near 0: dynamics are lost.
  brt~       Spearman correlation between each hit's performed brightness and
             its rendered centroid (0-40 ms), within role. Near 0: tone is
             not following the mouth.
  roles      how far apart kick, snare and hat sit in spectral shape compared
             with the spread inside each role. Under about 1 they blur.

With --png, also draws the first two bars of each render as a spectrogram
with the 16th grid, for looking at.
"""

from __future__ import annotations

import argparse
import pickle
import sys
from pathlib import Path

import numpy as np
from scipy import signal, stats
from scipy.io import wavfile

sys.path.insert(0, str(Path(__file__).parent))
import loop as loop_mod  # noqa: E402,F401  (needed to unpickle Loop)


def db(x):
    return 10 * np.log10(np.asarray(x, float) + 1e-12)


def rms_env(y: np.ndarray, sr: int, win: float) -> np.ndarray:
    k = max(1, int(win * sr))
    return np.sqrt(np.convolve(y**2, np.ones(k) / k, "same"))


def measure(y: np.ndarray, sr: int, lp) -> dict:
    out = {"peak": float(np.abs(y).max()), "clip%": 100 * float(np.mean(np.abs(y) > 0.95))}
    f, pw = signal.welch(y, sr, nperseg=8192)
    tot = pw.sum() + 1e-30
    for name, lo, hi in (("<100", 0, 100), ("100-1k", 100, 1000), ("1k-8k", 1000, 8000), (">8k", 8000, sr / 2)):
        out[name] = 100 * float(pw[(f >= lo) & (f < hi)].sum() / tot)

    e5 = db(rms_env(y, sr, 0.005) ** 2)
    e10 = db(rms_env(y, sr, 0.010) ** 2)
    hits = [h for h in lp.hits if h.sample + int(0.03 * sr) < len(y)]
    punch = []
    for h in hits:
        a = h.sample
        pre = e5[max(0, a - int(0.03 * sr)) : max(1, a)]
        post = e5[a : a + int(0.025 * sr)]
        if len(pre) and len(post):
            punch.append(post.max() - 10 * np.log10(np.mean(10 ** (pre / 10))))
    out["punch"] = float(np.median(punch)) if punch else float("nan")
    loud = e10[e10 > e10.max() - 80]
    out["depth"] = float(np.percentile(loud, 95) - np.percentile(loud, 10)) if loud.size else 0.0

    sos = signal.butter(4, [30, 150], "bandpass", fs=sr, output="sos")
    low = db(rms_env(signal.sosfilt(sos, y), sr, 0.01) ** 2)
    out["drone%"] = 100 * float(np.mean(low > np.percentile(low, 99) - 10))

    hop = int(0.005 * sr)
    fr, tt, Z = signal.stft(y, sr, nperseg=1024, noverlap=1024 - hop)
    mag = np.log1p(np.abs(Z) * 100)
    flux = np.maximum(0, np.diff(mag, axis=1)).sum(axis=0)
    tt = tt[1:]
    marks = np.array(sorted([h.sample / sr for h in lp.hits] + [b.start / sr for b in lp.bass]))
    near = np.min(np.abs(tt[:, None] - marks[None, :]), axis=1) <= 0.02 if marks.size else np.zeros(len(tt), bool)
    out["ongrid%"] = 100 * float(flux[near].sum() / (flux.sum() + 1e-12))

    feats, peaks, cents, roles, vels, brts = [], [], [], [], [], []
    edges = np.geomspace(40, min(16000, sr / 2 - 1), 9)
    for h in hits:
        seg = y[h.sample : h.sample + int(0.06 * sr)]
        if len(seg) < 64:
            continue
        sp = np.abs(np.fft.rfft(seg * np.hanning(len(seg)))) ** 2
        fq = np.fft.rfftfreq(len(seg), 1 / sr)
        bands = np.array([sp[(fq >= lo) & (fq < hi)].sum() for lo, hi in zip(edges[:-1], edges[1:])])
        feats.append(db(bands / (bands.sum() + 1e-20)))
        s40 = seg[: int(0.04 * sr)]
        sp40 = np.abs(np.fft.rfft(s40 * np.hanning(len(s40)))) ** 2
        fq40 = np.fft.rfftfreq(len(s40), 1 / sr)
        cents.append(float((fq40 * sp40).sum() / (sp40.sum() + 1e-20)))
        peaks.append(float(e5[h.sample : h.sample + int(0.03 * sr)].max()))
        roles.append(h.role)
        vels.append(h.vel)
        brts.append(h.bright)
    roles = np.array(roles)

    def within_corr(a, b):
        za, zb = [], []
        for r in set(roles):
            m = roles == r
            if m.sum() >= 4 and np.std(np.asarray(a)[m]) > 0 and np.std(np.asarray(b)[m]) > 0:
                za += list(stats.rankdata(np.asarray(a)[m]) / m.sum())
                zb += list(stats.rankdata(np.asarray(b)[m]) / m.sum())
        return float(stats.spearmanr(za, zb)[0]) if len(za) >= 6 else float("nan")

    out["vel~"] = within_corr(vels, peaks)
    out["brt~"] = within_corr(brts, cents)
    F = np.array(feats)
    present = [r for r in ("kick", "snare", "hat") if (roles == r).sum() >= 2]
    if len(present) >= 2:
        means = {r: F[roles == r].mean(axis=0) for r in present}
        spread = np.mean([np.linalg.norm(F[roles == r] - means[r], axis=1).mean() for r in present])
        between = np.mean([np.linalg.norm(means[a] - means[b]) for i, a in enumerate(present) for b in present[i + 1 :]])
        out["roles"] = float(between / (spread + 1e-9))
    else:
        out["roles"] = float("nan")
    return out


def draw(d: Path, names: list[str], lp, png: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    six = 60 / lp.bpm / 4
    fig, axes = plt.subplots(len(names), 1, figsize=(14, 2.3 * len(names)), squeeze=False)
    for ax, nm in zip(axes[:, 0], names):
        sr, y = wavfile.read(d / f"{nm}.wav")
        y = (y / 32768.0 if y.dtype == np.int16 else y.astype(float))[: int(32 * six * sr)]
        f, t, s = signal.spectrogram(y, sr, nperseg=1024, noverlap=1024 - 128)
        ax.pcolormesh(t, f, 10 * np.log10(s + 1e-12), shading="auto", cmap="magma", vmin=-100, vmax=-25)
        ax.set_yscale("symlog", linthresh=200)
        ax.set_ylim(30, 20000)
        ax.set_yticks([50, 100, 200, 1000, 5000], ["50", "100", "200", "1k", "5k"])
        for k in range(33):
            ax.axvline(k * six, color="w" if k % 4 == 0 else "#777", lw=0.6 if k % 4 == 0 else 0.3)
        for h in lp.hits:
            if h.sample / sr < 32 * six:
                ax.plot(h.sample / sr, 35, marker="^", ms=4, color={"kick": "#eb6834", "snare": "#1baf7a", "hat": "#eda100"}[h.role])
        ax.set_title(f"{nm}: first 2 bars (triangles: kick orange, snare green, hat yellow)", fontsize=9, loc="left")
    fig.tight_layout()
    fig.savefig(png, dpi=70)
    plt.close(fig)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dir", type=Path)
    ap.add_argument("names", nargs="*")
    ap.add_argument("--loop", type=Path, help="default: DIR/loop.pkl")
    ap.add_argument("--png", type=Path)
    args = ap.parse_args()
    with (args.loop or args.dir / "loop.pkl").open("rb") as f:
        lp, _ = pickle.load(f)
    names = args.names or sorted(p.stem for p in args.dir.glob("*.wav"))
    cols = ["peak", "clip%", "<100", "100-1k", "1k-8k", ">8k", "punch", "depth", "drone%", "ongrid%", "vel~", "brt~", "roles"]
    print(f"{'render':22s}" + "".join(f"{c:>8s}" for c in cols))
    for nm in names:
        sr, y = wavfile.read(args.dir / f"{nm}.wav")
        y = y / 32768.0 if y.dtype == np.int16 else y.astype(float)
        if y.ndim == 2:
            y = y.mean(axis=1)
        m = measure(y, sr, lp)
        print(f"{nm:22s}" + "".join(f"{m[c]:8.2f}" for c in cols))
    if args.png:
        draw(args.dir, names, lp, args.png)
        print(f"wrote {args.png}")


if __name__ == "__main__":
    main()
