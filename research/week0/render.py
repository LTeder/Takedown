"""Week 0 listening test: play a take through the sketch palettes.

    uv run python week0/render.py TAKE.wav [--rolls 3] [--palettes rubber crushed struck]

Runs `takedown analyze`, turns the picked onsets into drum hits and the pitch
track into a phrase, synthesises both through each palette in `sketch.py`
with a few dice rolls, and writes to `out/week0/<take>/`:

- `<palette>-<roll>.wav`: the renders, at the take's rate, loudness-matched.
- `original.wav`: the take at the same loudness, for comparison.
- `index.html`: a listening page. One transport, instant switching between
  the take and every render at the same position, a blind mode, and verdict
  buttons whose result can be copied out as text. Open it from disk.
- `control.wav`: the first palette with every hit's features swapped for
  another hit's. Same rhythm, scrambled expression: if it sounds as good as
  the real render, the performance is not audible in the result.
- `events.csv`: what each onset became, and `report.txt`: levels and
  spectral balance of each render.

How a take becomes events (all of it lens work; none of it changes evidence):

- An onset is a **phrase accent** when a pitched stretch of 300 ms or more
  starts within 60 ms of it (a sung note starting), or when it falls inside
  a pitched stretch (a note re-articulated). It is **dropped** when its peak
  is under 18 dB above the take's floor, or it is weak on both counts
  (strength under 1.5 and rise under 22 dB). Anything else is a **hit**.
  On AVP's improvisations this keeps 97% of the annotated hits as hits (the
  rest are mostly pitched "bmm" kicks, which become bass notes) and drops 30
  unannotated onsets for 2 real ones.
- A hit is a kick when it lifts the 60 to 200 Hz band well above that
  band's floor in the take (the upper of two clusters, if they sit 6 dB
  apart): low punch, however breathy the rest of the sound. The others
  split into snare and hat by brightness (two clusters of log centroid).
  Against AVP's labels this is right 61% of the time, with 92% of kicks
  found; brightness alone gets 53%. Velocity is peak loudness over a 40 dB
  range; a hit's brightness is its rank within its role.
- The phrase is eager: a frame is tonal if the decoder called it voiced or
  gave it a voiced probability of 0.25 or more, as long as it is 15 dB above
  the floor. Gaps under 60 ms are bridged, runs under 80 ms dropped. Pitch
  is the decoder's path; amplitude follows loudness over 40 dB; brightness
  follows the centroid in harmonic units.

The listening files are normalised to -16 LUFS and soft-clipped so palettes
compare at equal loudness. That is for listening only: the sketch itself has
no bus effects.
"""

from __future__ import annotations

import argparse
import csv
import json
import warnings
from pathlib import Path

import numpy as np
from scipy import signal
from scipy.io import wavfile

import precheck
import sketch

OVER = 2  # synthesis runs at twice the take's rate, so drive and fold alias less
FRAME_HZ = 200
ROLES = ("kick", "snare", "hat")
TARGET_LUFS = -16.0
FLOOR_PCT = 10
RANGE_DB = 40.0


def read_mono(path: Path) -> tuple[int, np.ndarray]:
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


# ------------------------------------------------------------------ events


def runs_of(mask: np.ndarray) -> list[tuple[int, int]]:
    edges = np.flatnonzero(np.diff(np.r_[0, mask.astype(int), 0]))
    return list(zip(edges[::2], edges[1::2]))


def tonal_mask(fr: np.ndarray, floor: float) -> np.ndarray:
    """Eager voicing for sound: see the module docstring."""
    m = ((fr["voiced"] == 1) | (fr["voiced_p"] >= 0.25)) & (fr["loudness_db"] > floor + 15) & (fr["f0_hz"] > 0)
    merged: list[list[int]] = []
    for s, e in runs_of(m):
        if merged and s - merged[-1][1] < 12:
            merged[-1][1] = e
        else:
            merged.append([s, e])
    out = np.zeros(len(m), bool)
    for s, e in merged:
        if e - s >= 16:
            out[s:e] = True
    return out


def onset_events(fr: np.ndarray, picked: np.ndarray, tonal: np.ndarray, floor: float) -> list[dict]:
    loud, cent, tilt = fr["loudness_db"], fr["centroid_hz"], fr["tilt_db_oct"]
    n = len(loud)
    # The frame of the refined onset, not the detection frame (`picked["frame"]` is the flux peak, a few ms later).
    starts = np.clip(np.round(picked["time_s"] * FRAME_HZ).astype(int), 0, n - 1)
    events = []
    for i, f in enumerate(starts):
        nxt = int(starts[i + 1]) if i + 1 < len(starts) else n
        lim = max(f + 1, min(nxt, f + 300, n))
        head = loud[f : min(f + 10, lim)]
        pk, peak = f + int(np.argmax(head)), float(head.max())
        below = np.nonzero(loud[pk:lim] < peak - 25.0)[0]
        dur = max(0.02, ((pk + int(below[0]) if below.size else lim) - f) / FRAME_HZ)
        att = slice(f, min(f + 8, lim))
        near = loud[att] > peak - 20
        c, tl = cent[att][near], tilt[att][near]
        c, tl = c[np.isfinite(c) & (c > 0)], tl[np.isfinite(tl)]
        ev = dict(t=float(picked["time_s"][i]), sample=int(picked["sample"][i]), dur=dur, peak_db=peak,
                  bright_hz=float(np.median(c)) if c.size else np.nan, tilt=float(np.median(tl)) if tl.size else np.nan,
                  strength=float(picked["strength"][i]), rise_db=float(picked["rise_db"][i]))
        k = f  # the tonal run that starts within 60 ms of the onset, and how long it lasts
        while k < min(n, f + 12) and not tonal[k]:
            k += 1
        j = k
        while j < n and tonal[j]:
            j += 1
        note_start = k < min(n, f + 12) and (j - k) / FRAME_HZ >= 0.3
        mid_phrase = f >= 4 and tonal[f - 4 : f].mean() >= 0.5 and tonal[f : f + 20].mean() >= 0.5
        if note_start or mid_phrase:
            ev["kind"] = "accent"
        elif peak < floor + 18 or (ev["strength"] < 1.5 and ev["rise_db"] < 22):
            ev["kind"] = "dropped"
        else:
            ev["kind"] = "hit"
        events.append(ev)
    return events


def low_rise(x: np.ndarray, sr: int, samples: list[int]) -> np.ndarray:
    """How far each hit lifts the 60 to 200 Hz band above that band's own floor in the take, in dB."""
    f, t, s = signal.stft(x, sr, nperseg=2048, noverlap=2048 - sr // 100, boundary=None, padded=False)
    level = 10 * np.log10((np.abs(s[(f >= 60) & (f < 200)]) ** 2).sum(axis=0) + 1e-14)
    floor = np.percentile(level, FLOOR_PCT)
    out = np.zeros(len(samples))
    for i, n in enumerate(samples):
        k0 = int(np.searchsorted(t, n / sr))
        k1 = max(k0 + 1, int(np.searchsorted(t, n / sr + 0.05)))
        if k0 < len(level):
            out[i] = level[k0:k1].max() - floor
    return out


def two_means(v: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Labels (0 low, 1 high) and the two centres."""
    c = np.array([np.percentile(v, 25), np.percentile(v, 75)], float)
    for _ in range(100):
        label = np.abs(v[:, None] - c[None, :]).argmin(axis=1)
        new = np.array([v[label == j].mean() if np.any(label == j) else c[j] for j in range(2)])
        if np.allclose(new, c):
            break
        c = new
    return label, c


def assign_roles(hits: list[dict], x: np.ndarray, sr: int) -> None:
    """Kick first, by low-band punch; then snare against hat, by brightness."""
    if not hits:
        return
    low = low_rise(x, sr, [h["sample"] for h in hits])
    label, c = two_means(low)
    kick = (label == 1) & (c[1] - c[0] >= 6.0)
    cent = np.array([np.log2(h["bright_hz"]) if h["bright_hz"] > 0 else np.nan for h in hits])
    cent = np.where(np.isfinite(cent), cent, np.nanmedian(cent) if np.isfinite(cent).any() else 0.0)
    rest = np.flatnonzero(~kick)
    bright = np.zeros(len(hits), bool)
    if len(rest) >= 2:
        bright[rest] = two_means(cent[rest])[0] == 1
    peak_max = max(h["peak_db"] for h in hits)
    for i, h in enumerate(hits):
        h["role"] = "kick" if kick[i] else ("hat" if bright[i] else "snare")
        h["low_rise"] = float(low[i])
        h["vel"] = float(np.clip(1 - (peak_max - h["peak_db"]) / RANGE_DB, 0.05, 1.0))
    for role in ROLES:  # brightness is the hit's rank within its role
        group = [(cent[i], h) for i, h in enumerate(hits) if h["role"] == role]
        for r, j in enumerate(np.argsort([g[0] for g in group])):
            group[j][1]["bright"] = r / (len(group) - 1) if len(group) > 1 else 0.5


# ------------------------------------------------------------------ render


def region_controls(fr: np.ndarray, s: int, e: int, accents: list[dict], top: float, x: np.ndarray, sr: int):
    """Audio-rate controls for tonal frames s..e, and where the region starts (synthesis rate)."""
    sr_s = sr * OVER
    a, b = max(0, s - 2), min(len(fr), e + 2)
    ft = fr["time_s"][a:b]
    gate = np.zeros(b - a)
    gate[s - a : e - a] = 1.0
    amp_f = np.clip((fr["loudness_db"][a:b] - (top - RANGE_DB)) / RANGE_DB, 0, 1) * gate
    ch = fr["centroid_harm"][a:b]
    brt_f = np.clip((np.log2(np.where(np.isfinite(ch) & (ch > 0), ch, 2.0)) - 1) / 3, 0, 1)
    acc_f = np.zeros(b - a)
    for ev in accents:
        k = int(round(ev["t"] * FRAME_HZ)) - a
        if 0 <= k < b - a:
            acc_f[k:] += min(1.0, 0.3 + ev["strength"] / 8) * np.exp(-np.arange(b - a - k) / (0.06 * FRAME_HZ))
    n0 = int(round(ft[0] * sr_s))
    n = int(round((ft[-1] - ft[0]) * sr_s)) + 1
    ts = ft[0] + np.arange(n) / sr_s
    f = 2 ** np.interp(ts, ft, np.log2(np.clip(fr["f0_hz"][a:b], 20, None)))
    i0 = int(round(ft[0] * sr))
    seg = x[i0 : i0 + n // OVER + 2]
    mouth = sketch.pad_to(signal.resample_poly(seg, OVER, 1) if len(seg) > 8 else np.zeros(n), n)
    return n0, sketch.Controls(f=f, amp=np.interp(ts, ft, amp_f), brt=np.interp(ts, ft, brt_f),
                               acc=np.clip(np.interp(ts, ft, acc_f), 0, 1), mouth=mouth)


def ratchets(hits: list[dict], prob: float, roll: int, sr: int) -> list[dict]:
    """The dice may turn a hit with room after it into a roll of 2 to 4, fading. Seeded per hit."""
    if prob <= 0:
        return hits
    times = np.array(sorted(h["sample"] for h in hits))
    out = []
    for h in hits:
        rng = np.random.default_rng([roll, 77, h["sample"]])
        later = times[times > h["sample"]]
        gap = min(0.5, (later[0] - h["sample"]) / sr) if later.size else 0.5
        if rng.random() < prob and gap >= 0.16:
            k = int(rng.choice([2, 3, 4]))
            out += [dict(h, sample=h["sample"] + int(i * gap / k * sr), vel=h["vel"] * (1 - 0.18 * i),
                         dur=min(h["dur"], gap / k)) for i in range(k)]
        else:
            out.append(h)
    return out


def render(pal: sketch.Palette, roll: int, hits: list[dict], regions: list, x: np.ndarray, sr: int) -> np.ndarray:
    sr_s = sr * OVER
    n_s = len(x) * OVER
    out = np.zeros(n_s)
    params = pal.params(None if roll == 1 else np.random.default_rng([sorted(sketch.PALETTES).index(pal.name), roll]))
    fade = int(0.003 * sr_s)
    hits = ratchets(hits, params.get("ratchet", 0.0), roll, sr)
    for ri, role in enumerate(ROLES):
        group = sorted((h for h in hits if h["role"] == role), key=lambda h: h["sample"])
        for i, h in enumerate(group):
            n0 = h["sample"] * OVER
            m = x[h["sample"] : h["sample"] + int(min(h["dur"], 0.12) * sr)]
            mouth = signal.resample_poly(m, OVER, 1) if len(m) > 8 else np.zeros(8)
            nf = min(len(mouth) // 2, int(0.0005 * sr_s)) or 1
            mouth[:nf] *= np.linspace(0, 1, nf)
            mouth[-len(mouth) // 3 :] *= np.cos(np.linspace(0, np.pi / 2, len(mouth[-len(mouth) // 3 :]))) ** 2
            hit = sketch.Hit(n0=n0, role=role, vel=h["vel"], bright=h["bright"], dur=h["dur"], mouth=sketch.norm(mouth), pre=0)
            y = pal.hits[role](hit, params, np.random.default_rng([roll, ri, n0]), sr_s)
            if i + 1 < len(group):  # a voice is monophonic: the next hit of the role chokes this one
                room = group[i + 1]["sample"] * OVER - n0 + fade
                if room < len(y):
                    y = y[:room].copy()
                    y[-fade:] *= np.cos(np.linspace(0, np.pi / 2, fade)) ** 2
            y = y[: n_s - n0]
            out[n0 : n0 + len(y)] += y * pal.levels[role] * 10 ** ((h["vel"] - 1) * 24 / 20)
    for j, (n0, c) in enumerate(regions):
        y = pal.phrase(c, params, np.random.default_rng([roll, 9, j]), sr_s)[: n_s - n0]
        out[n0 : n0 + len(y)] += y * pal.levels["phrase"]
    return signal.resample_poly(out, 1, OVER)


# ------------------------------------------------------------------ levels


def lufs(x: np.ndarray, sr: int) -> float:
    """Integrated loudness, ITU-R BS.1770 with gating."""
    def shelf(f0, gain_db, q):
        a_ = 10 ** (gain_db / 40)
        w = 2 * np.pi * f0 / sr
        al, c = np.sin(w) / (2 * q), np.cos(w)
        b = np.array([a_ * ((a_ + 1) + (a_ - 1) * c + 2 * np.sqrt(a_) * al), -2 * a_ * ((a_ - 1) + (a_ + 1) * c),
                      a_ * ((a_ + 1) + (a_ - 1) * c - 2 * np.sqrt(a_) * al)])
        a = np.array([(a_ + 1) - (a_ - 1) * c + 2 * np.sqrt(a_) * al, 2 * ((a_ - 1) - (a_ + 1) * c),
                      (a_ + 1) - (a_ - 1) * c - 2 * np.sqrt(a_) * al])
        return b / a[0], a / a[0]
    y = signal.lfilter(*shelf(1681.97, 4.0, 0.7072), x)
    y = signal.lfilter(*sketch.biquad("hp", 38.135, 0.5003, sr), y)
    blk, hop = int(0.4 * sr), int(0.1 * sr)
    if len(y) < blk:
        return -70.0
    z = np.array([np.mean(y[i : i + blk] ** 2) for i in range(0, len(y) - blk + 1, hop)])
    z = z[z > 10 ** ((-70 + 0.691) / 10)]
    if not z.size:
        return -70.0
    z = z[z > np.mean(z) * 10 ** (-10 / 10)]
    return float(-0.691 + 10 * np.log10(np.mean(z))) if z.size else -70.0


def for_listening(x: np.ndarray, sr: int) -> tuple[np.ndarray, float]:
    """Gain to the target loudness, then a soft clip above 0.7. Returns the audio and the gain in dB."""
    gain = 10 ** ((TARGET_LUFS - lufs(x, sr)) / 20)
    y = x * gain
    t = 0.7
    over = np.abs(y) > t
    y[over] = np.sign(y[over]) * (t + (0.98 - t) * np.tanh((np.abs(y[over]) - t) / (0.98 - t)))
    return y, float(20 * np.log10(gain))


def balance(x: np.ndarray, sr: int) -> str:
    f, p = signal.welch(x, sr, nperseg=8192)
    tot = p.sum() + 1e-30
    bands = [(0, 100), (100, 1000), (1000, 8000), (8000, sr / 2)]
    cent = float((f * p).sum() / tot)
    crest = 20 * np.log10(np.abs(x).max() / (np.sqrt(np.mean(x**2)) + 1e-12))
    return "  ".join(f"{100 * p[(f >= lo) & (f < hi)].sum() / tot:4.1f}%" for lo, hi in bands) + f"  centroid {cent:5.0f} Hz  crest {crest:4.1f} dB"


# -------------------------------------------------------------------- main


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("wav", type=Path)
    ap.add_argument("--rolls", type=int, default=3)
    ap.add_argument("--palettes", nargs="+", default=list(sketch.PALETTES), choices=list(sketch.PALETTES))
    ap.add_argument("--out", type=Path, help="default: research/out/week0/<take name>/")
    ap.add_argument("--bin", type=Path, default=precheck.DEFAULT_BIN)
    args = ap.parse_args()
    if not args.bin.exists():
        raise SystemExit(f"{args.bin} not found: run `cargo build --release -p takedown-cli` first")
    if not args.wav.is_file():
        raise SystemExit(f"{args.wav}: no such file")
    out = args.out or precheck.OUT / args.wav.stem
    out.mkdir(parents=True, exist_ok=True)

    fr, picked = precheck.analyze(args.bin, args.wav, out)
    sr, x = read_mono(args.wav)
    floor = float(np.percentile(fr["loudness_db"], FLOOR_PCT))
    tonal = tonal_mask(fr, floor)
    events = onset_events(fr, picked, tonal, floor)
    hits = [e for e in events if e["kind"] == "hit"]
    accents = [e for e in events if e["kind"] == "accent"]
    assign_roles(hits, x, sr)
    runs = runs_of(tonal)
    top = max((fr["loudness_db"][s:e].max() for s, e in runs), default=0.0)
    regions = [region_controls(fr, s, e, accents, top, x, sr) for s, e in runs]

    with (out / "events.csv").open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["time_s", "kind", "role", "vel", "bright", "dur_s", "peak_db", "bright_hz", "strength", "rise_db"])
        for e in events:
            w.writerow([f"{e['t']:.4f}", e["kind"], e.get("role", ""), f"{e.get('vel', 0):.2f}", f"{e.get('bright', 0):.2f}",
                        f"{e['dur']:.3f}", f"{e['peak_db']:.1f}", f"{e['bright_hz']:.0f}", f"{e['strength']:.2f}", f"{e['rise_db']:.1f}"])

    counts = ", ".join(f"{sum(h['role'] == r for h in hits)} {r}" for r in ROLES)
    summary = (f"{args.wav.name}: {len(x) / sr:.1f} s; {len(hits)} hits ({counts}), {len(accents)} phrase accents, "
               f"{sum(e['kind'] == 'dropped' for e in events)} onsets dropped; {len(runs)} tonal stretches, "
               f"{sum(e - s for s, e in runs) / FRAME_HZ:.1f} s")
    print(summary)
    report = [summary, "", f"{'file':16s} gain dB   <100 Hz  100-1k   1k-8k   >8k"]

    listen, gain = for_listening(x.copy(), sr)
    wavfile.write(out / "original.wav", sr, (listen * 32767).astype(np.int16))
    report.append(f"{'original':16s} {gain:+6.1f}   {balance(x, sr)}")
    sources = [dict(id="original", label="Your take", palette="", roll=0, file="original.wav", blurb="The recording itself.")]
    for name in args.palettes:
        pal = sketch.PALETTES[name]
        for roll in range(1, args.rolls + 1):
            y = render(pal, roll, hits, regions, x, sr)
            if not np.all(np.isfinite(y)):
                raise SystemExit(f"{name} roll {roll} produced non-finite samples")
            listen, gain = for_listening(y, sr)
            wavfile.write(out / f"{name}-{roll}.wav", sr, (listen * 32767).astype(np.int16))
            line = f"{name + '-' + str(roll):16s} {gain:+6.1f}   {balance(y, sr)}"
            report.append(line)
            print(line)
            sources.append(dict(id=f"{name}-{roll}", label=f"{name.capitalize()} {roll}", palette=name, roll=roll,
                                file=f"{name}-{roll}.wav", blurb=pal.blurb))
    if hits and args.palettes:  # the control: same hit times, every hit wearing another hit's features
        pal = sketch.PALETTES[args.palettes[0]]
        perm = np.random.default_rng(20261110).permutation(len(hits))
        swapped = [dict(h, **{k: hits[j][k] for k in ("role", "vel", "bright", "dur")}) for h, j in zip(hits, perm)]
        listen, gain = for_listening(render(pal, 1, swapped, regions, x, sr), sr)
        wavfile.write(out / "control.wav", sr, (listen * 32767).astype(np.int16))
        sources.append(dict(id="control", label=f"{pal.name.capitalize()} 1, shuffled", palette=pal.name, roll=1, file="control.wav",
                            blurb="A control: the same hit times, but each hit gets another hit's loudness, brightness, "
                                  "length and role. If this sounds as good as the real one, your performance is not coming through."))
    (out / "report.txt").write_text("\n".join(report) + "\n")

    k = max(1, len(x) // 1600)
    blk = x[: len(x) // k * k].reshape(-1, k)
    scale = float(np.abs(x).max()) or 1.0
    data = dict(take=args.wav.stem, duration=len(x) / sr, sources=sources,
                peaks=[[round(float(a) / scale, 3), round(float(b) / scale, 3)] for a, b in zip(blk.min(1), blk.max(1))],
                tonal=[[round(s / FRAME_HZ, 3), round(e / FRAME_HZ, 3)] for s, e in runs],
                hits=[[round(h["t"], 3), h["role"]] for h in hits])
    page = (Path(__file__).with_name("page.html")).read_text().replace("/*DATA*/null", json.dumps(data))
    (out / "index.html").write_text(page)
    print(f"open {out / 'index.html'}")


if __name__ == "__main__":
    main()
