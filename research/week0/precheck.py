"""Week 0 pre-check (MASTER-PLAN §5): beatbox fills to drum MIDI, hums to MPE.

    uv run python week0/precheck.py fills FILL1.wav FILL2.wav ... [--kick K.wav --snare S.wav --hat H.wav]
    uv run python week0/precheck.py hums HUM1.wav HUM2.wav ...

Throwaway: it exists to answer "does driving designed sounds from my own
performance excite me?" before the real lens, drums crate and midi crate
exist. It runs `takedown analyze` on each take and writes, under
`out/week0/` by default:

fills  `<take>.fills.mid`: one note per picked onset on channel 10 with
       General MIDI drum notes, one track per role (kick 36, snare 38,
       closed hat 42, open hat 46; a hat that rings past 180 ms is open).
       With `--kick K.wav --snare S.wav --hat H.wav` (about ten of each
       sound, each on its own) a hit takes the majority role of its three
       nearest taught hits on brightness, spectral tilt and decay; without
       them, brightness (median spectral centroid over the attack) is
       clustered into three groups across all the fills given together,
       darkest to brightest. On AVP's 28 improvisations the roles agree
       with the labels about 73% of the time taught from 10 to 30 hits per role and 54%
       by brightness alone (always guessing the commonest role: 41%).
       Velocity follows peak loudness (2.5 steps per dB below the loudest
       hit), note length follows the decay (until 25 dB below the peak or
       the next hit), and CC74 before each note carries the hit's
       brightness rank. `<take>.hits.csv` lists the same per hit.
hums   `<take>.hums.mid`: MPE, lower zone with 15 member channels and a
       48-semitone bend range. One note per voiced run (gaps under 50 ms
       bridged; split at a picked onset when both halves last 60 ms), on
       its own channel, keyed to the run's median pitch; pitch bend
       follows f0 every 5 ms, channel pressure follows loudness over the
       40 dB below the take's loudest voiced frame, CC74 follows the
       spectral centroid from 200 Hz to 4 kHz.

Files are SMF type 1 at 120 BPM and 960 PPQ, Reaper's default project tempo,
so a take and its MIDI dropped at the start of a default project line up.
"""

from __future__ import annotations

import argparse
import csv
import subprocess
import sys
import warnings
from dataclasses import dataclass
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "research" / "out" / "week0"
DEFAULT_BIN = REPO / "target" / "release" / ("takedown.exe" if sys.platform == "win32" else "takedown")

FRAME_HZ = 200
BPM, PPQ = 120, 960
TICKS_PER_S = PPQ * BPM / 60

# fills
ROLES = ("kick", "snare", "hat")
GM_NOTE = {"kick": 36, "snare": 38, "hat": 42, "open hat": 46}
DRUM_CH = 9  # channel 10
DECAY_DB = 25.0
MAX_HIT_S = 1.5
OPEN_HAT_S = 0.18
VEL_PER_DB = 2.5

# hums
BEND_RANGE = 48
GAP_FRAMES = 10  # 50 ms
MIN_FRAMES = 12  # 60 ms
PRESSURE_DB = 40.0
CC74_LO_HZ, CC74_HI_HZ = 200.0, 4000.0


# ----------------------------------------------------------- MIDI writing


def vlq(n: int) -> bytes:
    out = [n & 0x7F]
    n >>= 7
    while n:
        out.append((n & 0x7F) | 0x80)
        n >>= 7
    return bytes(reversed(out))


def tick(seconds: float) -> int:
    return max(0, round(seconds * TICKS_PER_S))


# Events are (tick, order, bytes). At one tick, note-offs (0) go before
# controller set-up (1), which goes before note-ons (2).
Event = tuple[int, int, bytes]


def meta(kind: int, data: bytes) -> bytes:
    return bytes([0xFF, kind]) + vlq(len(data)) + data


def cc(ch: int, num: int, val: int) -> bytes:
    return bytes([0xB0 | ch, num, int(np.clip(val, 0, 127))])


def bend(ch: int, value: int) -> bytes:
    v = int(np.clip(value, 0, 16383))
    return bytes([0xE0 | ch, v & 0x7F, v >> 7])


def track_chunk(name: str, events: list[Event]) -> bytes:
    data = bytearray(vlq(0) + meta(0x03, name.encode()))
    last = 0
    for t, _, msg in sorted(events, key=lambda e: (e[0], e[1])):
        data += vlq(t - last) + msg
        last = t
    data += vlq(0) + meta(0x2F, b"")
    return b"MTrk" + len(data).to_bytes(4, "big") + bytes(data)


def write_smf(path: Path, tracks: list[tuple[str, list[Event]]]) -> None:
    conductor = [(0, 1, meta(0x51, (60_000_000 // BPM).to_bytes(3, "big"))),
                 (0, 1, meta(0x58, bytes([4, 2, 24, 8])))]
    chunks = [track_chunk(path.stem, conductor)] + [track_chunk(n, e) for n, e in tracks]
    head = b"MThd" + (6).to_bytes(4, "big") + (1).to_bytes(2, "big") + len(chunks).to_bytes(2, "big") + PPQ.to_bytes(2, "big")
    path.write_bytes(head + b"".join(chunks))


# ------------------------------------------------------------- analysis


def analyze(binary: Path, wav: Path, out: Path) -> tuple[np.ndarray, np.ndarray]:
    frames, onsets = out / f"{wav.stem}.frames.csv", out / f"{wav.stem}.onsets.csv"
    cmd = [str(binary), "analyze", str(wav), "--frames", str(frames), "--onsets", str(onsets)]
    if subprocess.run(cmd, stdout=subprocess.DEVNULL).returncode != 0:
        raise SystemExit(f"takedown analyze failed on {wav} (its error is above)")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)  # header-only onsets file
        fr = np.atleast_1d(np.genfromtxt(frames, delimiter=",", names=True))
        on = np.atleast_1d(np.genfromtxt(onsets, delimiter=",", names=True))
    picked = on[on["picked"] == 1] if on.size and on.dtype.names else np.zeros(0, dtype=[("time_s", float), ("frame", float)])
    return fr, picked


# ---------------------------------------------------------------- fills


@dataclass
class Hit:
    t: float
    dur: float
    peak_db: float
    bright_hz: float
    tilt: float
    role: str = ""
    note: int = 0
    vel: int = 0
    cc74: int = 0

    def features(self) -> list[float]:
        """What the teach classifier compares: brightness, tilt and decay."""
        b = np.log2(self.bright_hz) if self.bright_hz > 0 else np.nan
        return [b, self.tilt, np.log2(self.dur)]


def hits_of(fr: np.ndarray, picked: np.ndarray) -> list[Hit]:
    loud, cent, tilt = fr["loudness_db"], fr["centroid_hz"], fr["tilt_db_oct"]
    n = len(loud)
    starts = picked["frame"].astype(int)
    hits = []
    for i, f in enumerate(starts):
        nxt = int(starts[i + 1]) if i + 1 < len(starts) else n
        lim = min(nxt, f + int(MAX_HIT_S * FRAME_HZ), n)
        if lim <= f:
            continue
        head = loud[f : min(f + 10, lim)]  # peak within 50 ms
        pk = f + int(np.argmax(head))
        peak = float(head.max())
        below = np.nonzero(loud[pk:lim] < peak - DECAY_DB)[0]
        end = pk + int(below[0]) if below.size else lim
        att = slice(f, min(f + 8, lim))  # timbre over the first 40 ms, near the peak
        near = loud[att] > peak - 20
        c, tl = cent[att][near], tilt[att][near]
        c, tl = c[np.isfinite(c)], tl[np.isfinite(tl)]
        hits.append(Hit(t=float(picked["time_s"][i]), dur=max(0.02, (end - f) / FRAME_HZ), peak_db=peak,
                        bright_hz=float(np.median(c)) if c.size else float("nan"),
                        tilt=float(np.median(tl)) if tl.size else float("nan")))
    return hits


def kmeans_1d(x: np.ndarray, k: int, iters: int = 100) -> np.ndarray:
    """Sorted cluster centres."""
    centres = np.quantile(x, (np.arange(k) + 0.5) / k)
    for _ in range(iters):
        label = np.argmin(np.abs(x[:, None] - centres[None, :]), axis=1)
        new = np.array([x[label == j].mean() if np.any(label == j) else centres[j] for j in range(k)])
        if np.allclose(new, centres):
            break
        centres = new
    return np.sort(centres)


def roles_by_brightness(hits: list[Hit]) -> str:
    """Three brightness clusters, darkest to brightest, become kick, snare, hat."""
    bright = np.array([h.bright_hz for h in hits])
    ok = np.isfinite(bright) & (bright > 0)
    for h in hits:
        h.role = "snare"
    if not ok.any():
        return "no hit has a brightness; every hit is a snare"
    logb = np.log2(bright[ok])
    k = min(len(ROLES), len(np.unique(logb)))
    centres = kmeans_1d(logb, k)
    names = ROLES if k == 3 else (("kick", "hat") if k == 2 else ("snare",))
    for h, b in zip((h for h, good in zip(hits, ok) if good), logb):
        h.role = names[int(np.argmin(np.abs(b - centres)))]
    return "roles by brightness, centres " + ", ".join(f"{r} {2**c:.0f} Hz" for r, c in zip(names, centres))


def roles_by_teach(hits: list[Hit], teach: dict[str, list[Hit]], k: int = 3) -> str:
    """k nearest teach hits, on z-scored brightness, tilt and decay."""
    x = np.array([h.features() for r in ROLES for h in teach[r]], dtype=float)
    y = np.array([r for r in ROLES for _ in teach[r]])
    keep = np.all(np.isfinite(x), axis=1)
    x, y = x[keep], y[keep]
    if len(x) == 0:
        raise SystemExit("no usable hits in the teach takes")
    mu, sd = x.mean(axis=0), x.std(axis=0) + 1e-9
    a = (x - mu) / sd
    for h in hits:
        f = np.array(h.features(), dtype=float)
        f = np.where(np.isfinite(f), f, mu)
        nearest = list(y[np.argsort(((a - (f - mu) / sd) ** 2).sum(axis=1))[:k]])
        h.role = max(nearest, key=lambda r: (nearest.count(r), -nearest.index(r)))  # ties: the closest
    return "roles taught from " + ", ".join(f"{int(np.sum(y == r))} {r}" for r in ROLES)


def finish(hits: list[Hit]) -> None:
    """Note, velocity and CC74 from role, loudness and brightness rank."""
    if not hits:
        return
    peak_max = max(h.peak_db for h in hits)
    bright = np.array([h.bright_hz for h in hits])
    ok = np.isfinite(bright) & (bright > 0)
    rank = np.full(len(hits), 64)
    if ok.sum() > 1:
        r = np.argsort(np.argsort(bright[ok]))
        rank[ok] = np.round(127 * r / (ok.sum() - 1)).astype(int)
    for h, c in zip(hits, rank):
        h.note = GM_NOTE["open hat" if h.role == "hat" and h.dur > OPEN_HAT_S else h.role]
        h.vel = int(np.clip(round(127 - (peak_max - h.peak_db) * VEL_PER_DB), 1, 127))
        h.cc74 = int(c)


def write_fills(path: Path, hits: list[Hit]) -> None:
    tracks = []
    for role in ROLES:
        ev: list[Event] = []
        for h in (h for h in hits if h.role == role):
            on, off = tick(h.t), max(tick(h.t) + 1, tick(h.t + h.dur))
            ev += [(on, 1, cc(DRUM_CH, 74, h.cc74)),
                   (on, 2, bytes([0x90 | DRUM_CH, h.note, h.vel])),
                   (off, 0, bytes([0x80 | DRUM_CH, h.note, 64]))]
        tracks.append((role, ev))
    write_smf(path, tracks)
    with path.with_name(path.name.replace(".fills.mid", ".hits.csv")).open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["time_s", "dur_s", "peak_db", "bright_hz", "role", "note", "vel", "cc74"])
        for h in hits:
            w.writerow([f"{h.t:.4f}", f"{h.dur:.3f}", f"{h.peak_db:.1f}", f"{h.bright_hz:.0f}", h.role, h.note, h.vel, h.cc74])


# ----------------------------------------------------------------- hums


def voiced_runs(fr: np.ndarray, picked: np.ndarray) -> list[tuple[int, int]]:
    v = fr["voiced"] == 1
    n = len(v)
    # bridge short unvoiced gaps between voiced frames
    edges = np.flatnonzero(np.diff(np.r_[0, v.astype(int), 0]))
    runs = list(zip(edges[::2], edges[1::2]))
    merged: list[list[int]] = []
    for s, e in runs:
        if merged and s - merged[-1][1] < GAP_FRAMES:
            merged[-1][1] = e
        else:
            merged.append([s, e])
    onset_frames = np.sort(picked["frame"].astype(int)) if len(picked) else np.zeros(0, int)
    out = []
    for s, e in merged:
        cuts = [s]
        for f in onset_frames[(onset_frames > s) & (onset_frames < e)]:
            if f - cuts[-1] >= MIN_FRAMES and e - f >= MIN_FRAMES:
                cuts.append(int(f))
        cuts.append(e)
        out += [(a, b) for a, b in zip(cuts[:-1], cuts[1:]) if b - a >= MIN_FRAMES and b <= n]
    return out


def write_hums(path: Path, fr: np.ndarray, picked: np.ndarray) -> int:
    runs = voiced_runs(fr, picked)
    f0 = np.clip(fr["f0_hz"], 20.0, None)
    st = 69 + 12 * np.log2(f0 / 440.0)
    loud = fr["loudness_db"]
    cent = fr["centroid_hz"]
    top = max((loud[s:e].max() for s, e in runs), default=0.0)
    pressure = np.clip((loud - (top - PRESSURE_DB)) / PRESSURE_DB * 127, 0, 127).round().astype(int)
    c = np.where(np.isfinite(cent) & (cent > 0), cent, np.nan)
    cc74 = np.clip(np.log2(c / CC74_LO_HZ) / np.log2(CC74_HI_HZ / CC74_LO_HZ) * 127, 0, 127)
    t = fr["time_s"]

    ev: list[Event] = [(0, 1, m) for m in (cc(0, 101, 0), cc(0, 100, 6), cc(0, 6, 15), cc(0, 101, 127), cc(0, 100, 127))]
    for ch in range(1, 16):
        ev += [(0, 1, m) for m in (cc(ch, 101, 0), cc(ch, 100, 0), cc(ch, 6, BEND_RANGE), cc(ch, 38, 0),
                                   cc(ch, 101, 127), cc(ch, 100, 127))]
    free_at = {ch: -1 for ch in range(1, 16)}
    for s, e in runs:
        ch = min(free_at, key=lambda k: free_at[k])
        on, off = tick(t[s]), max(tick(t[s]) + 1, tick(t[e - 1] + 1 / FRAME_HZ))
        free_at[ch] = off
        base = int(np.clip(round(float(np.median(st[s:e]))), 0, 127))
        vel = int(np.clip(round(127 - (top - loud[s : min(s + 6, e)].max()) * 127 / PRESSURE_DB), 1, 127))
        last_b = last_p = last_c = None
        for i in range(s, e):
            order = 1 if i == s else 3
            tt = on if i == s else tick(t[i])
            b = 8192 + round((st[i] - base) / BEND_RANGE * 8192)
            if last_b is None or abs(b - last_b) >= 2:
                ev.append((tt, order, bend(ch, b)))
                last_b = b
            if last_p is None or pressure[i] != last_p:
                ev.append((tt, order, bytes([0xD0 | ch, int(pressure[i])])))
                last_p = pressure[i]
            if np.isfinite(cc74[i]) and (last_c is None or round(cc74[i]) != last_c):
                last_c = round(cc74[i])
                ev.append((tt, order, cc(ch, 74, last_c)))
        ev += [(on, 2, bytes([0x90 | ch, base, vel])), (off, 0, bytes([0x80 | ch, base, 64]))]
    write_smf(path, [("hum (MPE)", ev)])
    return len(runs)


# ----------------------------------------------------------------- main


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("mode", choices=("fills", "hums"))
    ap.add_argument("wavs", type=Path, nargs="+")
    for r in ROLES:
        ap.add_argument(f"--{r}", type=Path, metavar="WAV",
                        help=f"fills only: a teach take of about ten {r}s, each on its own")
    ap.add_argument("--out", type=Path, default=OUT, help="default: research/out/week0/")
    ap.add_argument("--bin", type=Path, default=DEFAULT_BIN)
    args = ap.parse_args()
    if not args.bin.exists():
        raise SystemExit(f"{args.bin} not found: run `cargo build --release -p takedown-cli` first")
    teach_wavs = {r: getattr(args, r) for r in ROLES if getattr(args, r)}
    if teach_wavs and (args.mode == "hums" or len(teach_wavs) != len(ROLES)):
        raise SystemExit("teach takes are for fills, and need all three: --kick, --snare and --hat")
    for w in [*args.wavs, *teach_wavs.values()]:
        if not w.is_file():
            raise SystemExit(f"{w}: no such file")
    args.out.mkdir(parents=True, exist_ok=True)

    if args.mode == "hums":
        for w in args.wavs:
            fr, picked = analyze(args.bin, w, args.out)
            path = args.out / f"{w.stem}.hums.mid"
            print(f"{w.name}: {write_hums(path, fr, picked)} notes -> {path}")
        return

    per_take = []
    for w in args.wavs:
        fr, picked = analyze(args.bin, w, args.out)
        per_take.append((w, hits_of(fr, picked)))
    all_hits = [h for _, hs in per_take for h in hs]
    if teach_wavs:
        teach = {r: hits_of(*analyze(args.bin, w, args.out)) for r, w in teach_wavs.items()}
        for r, hs in teach.items():
            if len(hs) < 3:
                print(f"warning: the {r} teach take has {len(hs)} hits; record about ten, each on its own")
        print(roles_by_teach(all_hits, teach))
    else:
        print(roles_by_brightness(all_hits))
    finish(all_hits)
    for w, hs in per_take:
        path = args.out / f"{w.stem}.fills.mid"
        write_fills(path, hs)
        counts = ", ".join(f"{sum(h.role == r for h in hs)} {r}" for r in ROLES)
        print(f"{w.name}: {len(hs)} hits ({counts}) -> {path}")


if __name__ == "__main__":
    main()
