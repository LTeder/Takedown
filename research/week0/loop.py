"""Week 0, round 2: turn a take into a loop, and play the loop through palettes.

    uv run python week0/loop.py TAKE.wav [--palettes NAME ...] [--rolls 2] [--bars 8] [--out DIR]

Round 1 (`render.py`) replayed the whole take one hit at a time and the
founder found it unmusical and not rhythmic enough. This round takes the
performance as a source of material and feel rather than a score: it finds
the tempo and groove of the beatboxed stretches, picks the tightest two bars
as a pattern and another two as a fill, and turns the most melodic hummed
phrase into a bassline in the hum's key that plays on the beat's kicks. The
result is an 8-bar loop at the performed tempo, with the performed swing and
accents, rendered through each palette.

How (all lens-like decisions; evidence is untouched):

- Hits and phrase accents as in `render.py`. Kicks as there (low-band
  punch). The other hits of the beatboxed stretch the pattern comes from
  split into snare and hat by how much of their 1-16 kHz energy sits above
  8 kHz, with the boundary halfway between the pattern's backbeat hits and
  its off-beat 8ths: the snare is whatever sound lands on beats 2 and 4. In the founder's take the
  backbeat sound is loud between 1 and 8 kHz and round 1's brightness
  split had called it a hat, which left the loop with no backbeat.
- **Tempo.** Hits become a velocity-weighted pulse train at 200 Hz over the
  beatboxed stretches (where hits are dense and pitch is rare). Its
  autocorrelation, scored at the beat, half-beat and quarter-beat lags with
  a broad prior around 100 BPM, gives the beat period; dynamic-programming
  beat tracking (Ellis 2007) follows drift; each beat splits into four
  16ths.
- **Pattern.** Every 2-bar window starting on a beat is scored by how well
  its hits sit on the 16th grid, weighted by velocity, plus kicks on the
  downbeats and snares on beats 2 and 4. The best is the pattern, the best
  window that does not overlap it is the fill. Hits snap to the nearest
  16th; the swing (how late the odd 16ths sit) is kept, and half of each
  hit's remaining timing deviation.
- **Bassline.** The 10 s with the most pitched sound is split into notes
  (pitch steps over 0.8 semitones lasting 80 ms), quantised to the key
  that best fits their pitch classes (Krumhansl profiles), and spread over
  the 8 bars in proportion to their sung lengths. The bass plucks the
  current note on every kick and at every note change (at most a quarter
  note long, decaying), centred on the octave from C2 to B2.

Writes `<palette>-<roll>.wav` (loudness-matched), `loop.pkl` (the extracted
loop, reusable with `--from-loop` to skip analysis), `mouth-kit.wav` (your own
hit sounds re-sequenced into the loop, with a sine bass, for reference),
`loop.json` (tempo, grid, pattern, notes) and `index.html`.

A palette in `sketch.PALETTES` or `week0/palettes/<name>.py` (a module that
defines `PALETTE`) renders with its hit and phrase voices; a palette with a
`render_loop(loop, params, roll)` function renders the whole loop itself
(for example through a plugin host).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pickle
import zlib
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from scipy import signal
from scipy.io import wavfile

import precheck
import render
import sketch

FRAME_HZ = render.FRAME_HZ
OVER = render.OVER
PALETTE_DIR = Path(__file__).with_name("palettes")
BASS_GAIN = 0.4  # round-1 phrase voices were levelled for a hum on its own; under a beat they sit lower
KRUMHANSL_MAJOR = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
KRUMHANSL_MINOR = np.array([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])
MAJOR, MINOR = (0, 2, 4, 5, 7, 9, 11), (0, 2, 3, 5, 7, 8, 10)
NAMES = ("C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B")


# ------------------------------------------------------------------ types


@dataclass
class LoopHit:
    sample: int  # where it plays in the loop, at the take's rate
    src: int  # where it came from in the take
    role: str
    vel: float  # 0..1
    bright: float  # 0..1, rank within role
    dur: float  # seconds, as performed (capped at a beat)
    slot: int  # 16th index in the loop
    mouth: np.ndarray = field(repr=False, default_factory=lambda: np.zeros(0))  # the take's transient, take rate, peak 1


@dataclass
class BassNote:
    start: int  # samples, take rate
    end: int
    midi: int
    vel: float
    brt: float  # 0..1


@dataclass
class Loop:
    sr: int
    n: int  # length in samples
    bpm: float
    swing: float  # how late odd 16ths sit, in 16ths
    key: str
    hits: list
    bass: list
    take: np.ndarray = field(repr=False, default_factory=lambda: np.zeros(0))

    @property
    def sixteenth(self) -> float:
        return 60.0 / self.bpm / 4


# ------------------------------------------------------------------ analysis


def beat_regions(fr: np.ndarray, hits: list[dict], tonal: np.ndarray) -> list[tuple[int, int]]:
    """Frames where hits are dense (2 per second or more over 2 s) and pitch is rare."""
    n = len(fr)
    rate = np.zeros(n)
    for h in hits:
        rate[min(n - 1, int(round(h["t"] * FRAME_HZ)))] += 1
    win = np.ones(2 * FRAME_HZ) / 2.0
    dense = np.convolve(rate, win, "same") >= 2.0
    pitched = np.convolve(tonal.astype(float), np.ones(FRAME_HZ) / FRAME_HZ, "same") > 0.4
    return [(s, e) for s, e in render.runs_of(dense & ~pitched) if e - s >= 4 * FRAME_HZ]


def pulse(hits: list[dict], n: int) -> np.ndarray:
    env = np.zeros(n)
    for h in hits:
        k = int(round(h["t"] * FRAME_HZ))
        if 0 <= k < n:
            env[k] += h["vel"]
    g = signal.windows.gaussian(13, 2.0)
    return np.convolve(env, g / g.sum(), "same")


def beat_period(env: np.ndarray) -> float:
    """Beat period in frames: autocorrelation at beat, half and quarter lags, prior around 100 BPM."""
    e = env - env.mean()
    ac = np.correlate(e, e, "full")[len(e) - 1 :]
    ac /= ac[0] + 1e-12
    best, best_p = -np.inf, 120.0
    for bpm in np.arange(70.0, 160.0, 0.25):
        p = 60.0 * FRAME_HZ / bpm
        val = lambda lag: np.interp(lag, np.arange(len(ac)), ac)
        score = val(p) + 0.5 * val(p / 2) + 0.25 * val(p / 4) + 0.25 * val(2 * p)
        score *= np.exp(-0.5 * (np.log2(bpm / 100.0) / 0.5) ** 2)
        if score > best:
            best, best_p = score, p
    return best_p


def track_beats(env: np.ndarray, period: float, tightness: float = 100.0) -> np.ndarray:
    """Ellis (2007) dynamic-programming beat tracker on a pulse train; returns beat frames."""
    n = len(env)
    e = env / (env.std() + 1e-12)
    score, back = e.copy(), -np.ones(n, int)
    lo, hi = int(round(period / 2)), int(round(2 * period))
    for t in range(lo, n):
        prev = np.arange(max(0, t - hi), t - lo + 1)
        if prev.size == 0:
            continue
        cand = score[prev] - tightness * np.log((t - prev) / period) ** 2
        k = int(np.argmax(cand))
        score[t], back[t] = e[t] + cand[k], prev[k]
    tail = slice(max(0, n - int(period)), n)
    t = tail.start + int(np.argmax(score[tail]))
    beats = [t]
    while back[beats[-1]] >= 0:
        beats.append(int(back[beats[-1]]))
    return np.array(beats[::-1], float)


def sixteenths(beats: np.ndarray) -> np.ndarray:
    """16th-note grid in frames: each beat interval split in four."""
    pts = [beats[i] + (beats[i + 1] - beats[i]) * q / 4 for i in range(len(beats) - 1) for q in range(4)]
    return np.array(pts + [beats[-1]])


def window_score(hits: list[dict], grid: np.ndarray, b: int) -> float:
    """2 bars from 16th index 4*b: on-grid hit weight, kicks on downbeats, snares on 2 and 4."""
    lo, hi = grid[4 * b], grid[4 * b + 32]
    total = 0.0
    roles = set()
    for h in hits:
        f = h["t"] * FRAME_HZ
        if not lo - 2 <= f < hi - 2:
            continue
        pos = np.interp(f, grid[4 * b : 4 * b + 33], np.arange(33))
        slot, dev = int(round(pos)), pos - round(pos)
        total += h["vel"] * np.exp(-((dev / 0.25) ** 2))
        if h["role"] == "kick" and slot % 16 == 0:
            total += 1.5 * h["vel"]
        if h["role"] == "snare" and slot % 8 == 4:
            total += 1.0 * h["vel"]
        roles.add(h["role"])
    return total * (1.0 if len(roles) >= 2 else 0.3)


def take_pattern(hits: list[dict], grid: np.ndarray, b: int) -> list[dict]:
    """The window's hits snapped to 32 16th slots: slot, deviation (16ths), and the hit."""
    out: dict[tuple[int, str], dict] = {}
    seg = grid[4 * b : 4 * b + 33]
    for h in hits:
        f = h["t"] * FRAME_HZ
        if not seg[0] - 2 <= f < seg[-1] - 2:
            continue
        pos = float(np.interp(f, seg, np.arange(33)))
        slot = int(round(pos)) % 32
        dev = pos - round(pos)
        if abs(dev) > 0.45:
            continue
        key = (slot, h["role"])
        if key not in out or h["vel"] > out[key]["hit"]["vel"]:
            out[key] = dict(slot=slot, dev=dev, hit=h)
    return sorted(out.values(), key=lambda p: (p["slot"], p["hit"]["role"]))


def hum_notes(fr: np.ndarray, tonal: np.ndarray) -> list[tuple[float, float, float, float]]:
    """(start s, end s, midi float, brightness) for the 10 s with the most pitched sound."""
    n = len(fr)
    w = 10 * FRAME_HZ
    if n <= w:
        a = 0
    else:
        cs = np.r_[0, np.cumsum(tonal)]
        a = int(np.argmax(cs[w:] - cs[:-w]))
    b = min(n, a + w)
    st = 69 + 12 * np.log2(np.clip(fr["f0_hz"], 20, None) / 440)
    st = signal.medfilt(st, 11)
    ch = fr["centroid_harm"]
    notes = []
    for s, e in render.runs_of(tonal[a:b]):
        s, e = s + a, e + a
        cut = [s]
        ref = st[s]
        k = s
        while k < e:
            if abs(st[k] - ref) > 0.8 and k + 16 <= e and np.all(np.abs(st[k : k + 16] - st[k]) < 0.8):
                cut.append(k)
                ref = st[k]
            k += 1
        cut.append(e)
        for p, q in zip(cut[:-1], cut[1:]):
            if q - p >= 16:
                c = ch[p:q]
                c = c[np.isfinite(c) & (c > 0)]
                brt = float(np.clip((np.log2(np.median(c)) - 1) / 3, 0, 1)) if c.size else 0.5
                notes.append((p / FRAME_HZ, q / FRAME_HZ, float(np.median(st[p:q])), brt))
    return notes


def best_key(notes: list[tuple[float, float, float, float]]) -> tuple[int, tuple]:
    hist = np.zeros(12)
    for s, e, m, _ in notes:
        hist[int(round(m)) % 12] += e - s
    best, key = -np.inf, (0, MAJOR)
    for root in range(12):
        for prof, scale in ((KRUMHANSL_MAJOR, MAJOR), (KRUMHANSL_MINOR, MINOR)):
            r = np.corrcoef(hist, np.roll(prof, root))[0, 1] if hist.any() else 0.0
            if r > best:
                best, key = r, (root, scale)
    return key


def snap(m: float, root: int, scale: tuple) -> int:
    cands = [o * 12 + root + d for o in range(0, 11) for d in scale]
    return int(min(cands, key=lambda c: abs(c - m)))


# ------------------------------------------------------------------ build


def hf_ratio(x: np.ndarray, sr: int, sample: int) -> float:
    """dB of 8-16 kHz energy over 1-8 kHz energy in the 80 ms after a hit."""
    seg = x[sample : sample + int(0.08 * sr)]
    if len(seg) < 64:
        return 0.0
    sp = np.abs(np.fft.rfft(seg * np.hanning(len(seg)))) ** 2
    fq = np.fft.rfftfreq(len(seg), 1 / sr)
    hi, mid = sp[(fq >= 8000) & (fq < 16000)].sum(), sp[(fq >= 1000) & (fq < 8000)].sum()
    return float(10 * np.log10((hi + 1e-12) / (mid + 1e-12)))


def split_snare_hat(hits: list[dict], pattern: list[dict], x: np.ndarray, sr: int) -> str:
    """Re-split the non-kick `hits` into snare and hat by their high-band ratio.

    The snare is the sound on beats 2 and 4: the boundary sits halfway
    between the median ratio of the pattern's backbeat hits and that of its
    off-beat 8ths, on whichever side the backbeat lies. If the two do not
    differ by 3 dB, falls back to two clusters, the darker one the snare.
    Returns a note on what it did.
    """
    rest = [h for h in hits if h["role"] != "kick"]
    if len(rest) < 2:
        return "too few hits to split"
    ratio = {id(h): hf_ratio(x, sr, h["sample"]) for h in rest}
    back = [ratio[id(p["hit"])] for p in pattern if p["slot"] % 8 == 4 and id(p["hit"]) in ratio]
    off = [ratio[id(p["hit"])] for p in pattern if p["slot"] % 4 == 2 and id(p["hit"]) in ratio]
    if back and off and abs(np.median(back) - np.median(off)) >= 3:
        mb, mo = float(np.median(back)), float(np.median(off))
        mid = (mb + mo) / 2
        for h in rest:
            h["role"] = "snare" if (ratio[id(h)] < mid) == (mb < mid) else "hat"
        return f"snare by backbeat: backbeat {mb:.1f} dB, off-beats {mo:.1f} dB, boundary {mid:.1f} dB"
    label, _ = render.two_means(np.array([ratio[id(h)] for h in rest]))
    for h, lab in zip(rest, label):
        h["role"] = "hat" if lab == 1 else "snare"
    return "snare by two clusters (backbeat and off-beats too alike)"


def mouth_of(x: np.ndarray, sr: int, src: int, dur: float) -> np.ndarray:
    m = x[src : src + int(min(dur, 0.12) * sr)].copy()
    if len(m) < 16:
        return np.zeros(16)
    nf = max(1, int(0.0005 * sr))
    m[:nf] *= np.linspace(0, 1, nf)
    tail = len(m) // 3
    m[-tail:] *= np.cos(np.linspace(0, np.pi / 2, tail)) ** 2
    return sketch.norm(m)


def extract(wav: Path, out: Path, bars: int = 8, binary: Path = precheck.DEFAULT_BIN) -> tuple[Loop, dict]:
    fr, picked = precheck.analyze(binary, wav, out)
    sr, x = render.read_mono(wav)
    floor = float(np.percentile(fr["loudness_db"], render.FLOOR_PCT))
    tonal = render.tonal_mask(fr, floor)
    events = render.onset_events(fr, picked, tonal, floor)
    hits = [e for e in events if e["kind"] == "hit"]
    render.assign_roles(hits, x, sr)
    n = len(fr)

    regions = beat_regions(fr, hits, tonal)
    if not regions:
        raise SystemExit("no beatboxed stretch found: need 4 s or more with 2 or more hits per second and little pitch")
    best = None
    for s, e in regions:
        rh = [h for h in hits if s <= h["t"] * FRAME_HZ < e]
        env = pulse(rh, n)[s:e]
        period = beat_period(env)
        beats = track_beats(env, period) + s
        if len(beats) < 10:
            continue
        grid = sixteenths(beats)
        for b in range(0, len(beats) - 9):
            sc = window_score(rh, grid, b)
            if best is None or sc > best[0]:
                best = (sc, b, grid, rh, period, (s, e))
    if best is None:
        raise SystemExit("beat tracking found fewer than 10 beats in every beatboxed stretch")
    _, b, grid, rh, period, region = best
    fills = [(window_score(rh, grid, c), c) for c in range(0, (len(grid) - 1) // 4 - 8) if abs(c - b) >= 8]
    fill_b = max(fills)[1] if fills else b
    split_note = split_snare_hat(rh, take_pattern(rh, grid, b), x, sr)  # within the stretch the loop comes from
    pat = take_pattern(rh, grid, b)
    fill = take_pattern(rh, grid, fill_b)
    bpm = 60.0 * FRAME_HZ / float(np.mean(np.diff(grid[4 * b : 4 * b + 33])) * 4)

    odd = [p["dev"] for p in pat if p["slot"] % 2 == 1]
    even = [p["dev"] for p in pat if p["slot"] % 2 == 0]
    swing = float(np.clip((np.median(odd) if odd else 0.0) - (np.median(even) if even else 0.0), -0.1, 0.4))

    six = 60.0 / bpm / 4
    nloop = int(round(bars * 16 * six * sr))
    loop_hits = []
    for bar in range(0, bars, 2):
        src_pat = fill if bar == bars - 2 else pat
        for p in src_pat:
            slot = bar * 16 + p["slot"]
            dev = (swing if p["slot"] % 2 else 0.0) + 0.5 * (p["dev"] - (swing if p["slot"] % 2 else 0.0))
            t = (slot + dev) * six
            h = p["hit"]
            src = int(h["sample"])
            dur = float(min(h["dur"], 4 * six))
            loop_hits.append(LoopHit(sample=max(0, int(round(t * sr))), src=src, role=h["role"], vel=h["vel"],
                                     bright=h["bright"], dur=dur, slot=slot, mouth=mouth_of(x, sr, src, dur)))

    notes = hum_notes(fr, tonal)
    bass: list[BassNote] = []
    key_name = "none"
    if len(notes) >= 2:
        root, scale = best_key(notes)
        key_name = f"{NAMES[root]} {'major' if scale == MAJOR else 'minor'}"
        mids = [snap(m, root, scale) for _, _, m, _ in notes]
        shift = 12 * round((np.median(mids) - 41.5) / 12)  # median into C2..B2
        mids = [int(m - shift) for m in mids]
        lens = np.array([e - s for s, e, _, _ in notes])
        bounds = np.r_[0, np.cumsum(lens) / lens.sum()] * bars * 16  # note k covers 16ths bounds[k]..bounds[k+1]
        kick_slots = sorted({h.slot for h in loop_hits if h.role == "kick"})
        change_slots = [int(round(v)) for v in bounds[:-1]]
        trig = sorted(set(kick_slots) | set(change_slots))
        for i, s in enumerate(trig):
            k = int(np.clip(np.searchsorted(bounds, s, side="right") - 1, 0, len(notes) - 1))
            nxt = trig[i + 1] if i + 1 < len(trig) else bars * 16
            length = min(nxt - s, 4)
            start = int(round(s * six * sr))
            end = int(round((s + length) * six * sr - 0.012 * sr))
            kick = [h.vel for h in loop_hits if h.slot == s and h.role == "kick"]
            bass.append(BassNote(start=start, end=max(start + int(0.03 * sr), end), midi=mids[k],
                                 vel=float(kick[0]) if kick else 0.75, brt=notes[k][3]))

    loop = Loop(sr=sr, n=nloop, bpm=bpm, swing=swing, key=key_name, hits=loop_hits, bass=bass, take=x)
    grid_txt = {}
    for role, ch in (("kick", "K"), ("snare", "S"), ("hat", "h")):
        for name, p in (("pattern", pat), ("fill", fill)):
            row = ["."] * 32
            for q in p:
                if q["hit"]["role"] == role:
                    row[q["slot"]] = ch
            grid_txt[f"{name} {role}"] = " ".join("".join(row[i : i + 4]) for i in range(0, 32, 4))
    info = dict(bpm=round(bpm, 2), swing_16ths=round(swing, 3), key=key_name, bars=bars, roles=split_note,
                region_s=[round(region[0] / FRAME_HZ, 2), round(region[1] / FRAME_HZ, 2)],
                pattern_from_s=round(grid[4 * b] / FRAME_HZ, 2), fill_from_s=round(grid[4 * fill_b] / FRAME_HZ, 2),
                grid=grid_txt, hits=len(loop_hits), bass_notes=[[n.start / sr, n.end / sr, n.midi] for n in bass],
                hum_notes=[[round(s, 2), round(e, 2), round(m, 2)] for s, e, m, _ in notes])
    return loop, info


# ------------------------------------------------------------------ render


def bass_controls(loop: Loop, sr_s: int) -> sketch.Controls:
    """One phrase region over the whole loop at the synthesis rate: pitch glides 10 ms between notes."""
    n = loop.n * OVER
    f = np.full(n, np.nan)
    amp, brt, acc = np.zeros(n), np.full(n, 0.5), np.zeros(n)
    for nt in loop.bass:
        a, b = nt.start * OVER, min(n, nt.end * OVER)
        hz = 440.0 * 2 ** ((nt.midi - 69) / 12)
        f[a:b] = hz
        t = np.arange(b - a) / sr_s
        env = np.minimum(1, t / 0.002) * (0.25 + 0.75 * np.exp(-t / 0.09))  # a pluck, so the bass pulses with the kicks
        rel = min(len(env), int(0.025 * sr_s))
        env[-rel:] *= np.linspace(1, 0, rel)
        amp[a:b] = np.maximum(amp[a:b], env * (0.5 + 0.5 * nt.vel))
        brt[a:b] = nt.brt
        acc[a:b] = np.maximum(acc[a:b], np.exp(-t / 0.06))
    if np.isnan(f).all():
        f[:] = 55.0
    idx = np.flatnonzero(~np.isnan(f))
    f = np.interp(np.arange(n), idx, f[idx])  # hold pitch through gaps
    a = np.exp(-1 / (0.010 * sr_s))
    f = np.exp(signal.lfilter([1 - a], [1, -a], np.log(f), zi=[np.log(f[0]) * a])[0])
    return sketch.Controls(f=f, amp=amp, brt=brt, acc=acc, mouth=np.zeros(n))


def render_generic(pal: sketch.Palette, roll: int, loop: Loop) -> np.ndarray:
    sr, sr_s = loop.sr, loop.sr * OVER
    n_s = loop.n * OVER
    out = np.zeros(n_s)
    params = pal.params(None if roll == 1 else np.random.default_rng([zlib.crc32(pal.name.encode()), roll]))
    fade = int(0.003 * sr_s)
    for ri, role in enumerate(render.ROLES):
        group = sorted((h for h in loop.hits if h.role == role), key=lambda h: h.sample)
        for i, h in enumerate(group):
            n0 = h.sample * OVER
            mouth = signal.resample_poly(h.mouth, OVER, 1) if len(h.mouth) > 8 else np.zeros(16)
            hit = sketch.Hit(n0=n0, role=role, vel=h.vel, bright=h.bright, dur=h.dur, mouth=sketch.norm(mouth), pre=0)
            y = pal.hits[role](hit, params, np.random.default_rng([roll, ri, h.src]), sr_s)
            if i + 1 < len(group):
                room = group[i + 1].sample * OVER - n0 + fade
                if room < len(y):
                    y = y[:room].copy()
                    y[-fade:] *= np.cos(np.linspace(0, np.pi / 2, fade)) ** 2
            y = y[: max(0, n_s - n0)]
            out[n0 : n0 + len(y)] += y * pal.levels[role] * 10 ** ((h.vel - 1) * 24 / 20)
    if loop.bass:
        out += BASS_GAIN * pal.phrase(bass_controls(loop, sr_s), params, np.random.default_rng([roll, 9]), sr_s)[:n_s] * pal.levels["phrase"]
    return signal.resample_poly(out, 1, OVER)


def render_loop(pal: sketch.Palette, roll: int, loop: Loop) -> np.ndarray:
    custom = getattr(pal, "render_loop", None)
    if custom is not None:
        params = pal.params(None if roll == 1 else np.random.default_rng([zlib.crc32(pal.name.encode()), roll]))
        y = np.asarray(custom(loop, params, roll), float)
        y = y.mean(axis=0) if y.ndim == 2 and y.shape[0] <= 2 else (y.mean(axis=1) if y.ndim == 2 else y)
        return sketch.pad_to(y, loop.n)
    return render_generic(pal, roll, loop)


def mouth_kit(loop: Loop) -> np.ndarray:
    """Your own hit sounds re-sequenced into the loop, plus a sine bass."""
    sr = loop.sr
    out = np.zeros(loop.n)
    for h in loop.hits:
        m = loop.take[h.src : h.src + int(min(h.dur, 0.3) * sr)].copy()
        tail = max(1, len(m) // 4)
        m[-tail:] *= np.linspace(1, 0, tail)
        m = m[: max(0, loop.n - h.sample)]
        out[h.sample : h.sample + len(m)] += m
    c = bass_controls(loop, sr * OVER)
    sine = np.sin(2 * np.pi * np.cumsum(c.f) / (sr * OVER)) * c.amp
    out += 0.3 * np.abs(out).max() * signal.resample_poly(sine, 1, OVER)[: loop.n]
    return out


def load_palettes(names: list[str] | None) -> dict[str, sketch.Palette]:
    """Round-1 palettes plus `palettes/<name>.py`; with `names`, only those modules are imported."""
    pals = dict(sketch.PALETTES)
    for path in sorted(PALETTE_DIR.glob("*.py")) if PALETTE_DIR.is_dir() else []:
        if path.name.startswith("_") or (names and path.stem not in names):
            continue
        spec = importlib.util.spec_from_file_location(f"palettes.{path.stem}", path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        pals[mod.PALETTE.name] = mod.PALETTE
    if names:
        missing = [n for n in names if n not in pals]
        if missing:
            raise SystemExit(f"unknown palettes {missing}; have {sorted(pals)}")
        pals = {n: pals[n] for n in names}
    return pals


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("wav", type=Path)
    ap.add_argument("--palettes", nargs="+", metavar="NAME[:ROLLS]")
    ap.add_argument("--rolls", type=int, default=2)
    ap.add_argument("--bars", type=int, default=8, choices=(4, 8, 16))
    ap.add_argument("--out", type=Path, help="default: research/out/week0/<take>-loop/")
    ap.add_argument("--bin", type=Path, default=precheck.DEFAULT_BIN)
    ap.add_argument("--from-loop", type=Path, metavar="LOOP.pkl", help="reuse an extracted loop (written as loop.pkl by a previous run)")
    args = ap.parse_args()
    if not args.from_loop and not args.bin.exists():
        raise SystemExit(f"{args.bin} not found: run `cargo build --release -p takedown-cli` first")
    if not args.from_loop and not args.wav.is_file():
        raise SystemExit(f"{args.wav}: no such file")
    out = args.out or precheck.OUT / f"{args.wav.stem}-loop"
    out.mkdir(parents=True, exist_ok=True)
    rolls = {n.split(":")[0]: int(n.split(":")[1]) if ":" in n else args.rolls for n in args.palettes or []}
    pals = load_palettes(list(rolls) or None)

    if args.from_loop:
        with args.from_loop.open("rb") as f:
            loop, info = pickle.load(f)
    else:
        loop, info = extract(args.wav, out, args.bars, args.bin)
        with (out / "loop.pkl").open("wb") as f:
            pickle.dump((loop, info), f)
    (out / "loop.json").write_text(json.dumps(info, indent=1))
    print(f"{args.wav.name}: {info['bpm']} BPM, swing {info['swing_16ths']} 16ths, key {info['key']}, "
          f"pattern from {info['pattern_from_s']} s, fill from {info['fill_from_s']} s, "
          f"{len(loop.hits)} hits, {len(loop.bass)} bass notes")
    for k, v in info["grid"].items():
        print(f"  {k:14s} {v}")

    sr = loop.sr
    ref, gain = render.for_listening(mouth_kit(loop), sr)
    wavfile.write(out / "mouth-kit.wav", sr, (ref * 32767).astype(np.int16))
    sources = [dict(id="mouth-kit", label="Your sounds, re-sequenced", palette="", roll=0, file="mouth-kit.wav",
                    blurb="Your own hits snapped into the loop, with a sine playing the bassline from your hum.")]
    report = [f"{'file':16s} gain dB   <100 Hz  100-1k   1k-8k   >8k"]
    for name, pal in pals.items():
        for roll in range(1, rolls.get(name, args.rolls) + 1):
            y = render_loop(pal, roll, loop)
            if not np.all(np.isfinite(y)) or np.abs(y).max() == 0:
                print(f"{name}-{roll}: silent or non-finite, skipped")
                continue
            listen, gain = render.for_listening(y, sr)
            wavfile.write(out / f"{name}-{roll}.wav", sr, (listen * 32767).astype(np.int16))
            line = f"{name + '-' + str(roll):16s} {gain:+6.1f}   {render.balance(y, sr)}"
            print(line)
            report.append(line)
            sources.append(dict(id=f"{name}-{roll}", label=f"{name.capitalize()} {roll}", palette=name, roll=roll,
                                file=f"{name}-{roll}.wav", blurb=pal.blurb))
    (out / "report.txt").write_text("\n".join(report) + "\n")

    k = max(1, len(ref) // 1600)
    blk = ref[: len(ref) // k * k].reshape(-1, k)
    scale = float(np.abs(ref).max()) or 1.0
    data = dict(take=f"{args.wav.stem} loop ({info['bpm']:.0f} BPM, {info['key']})", duration=loop.n / sr, sources=sources,
                peaks=[[round(float(a) / scale, 3), round(float(b) / scale, 3)] for a, b in zip(blk.min(1), blk.max(1))],
                tonal=[[round(nt.start / sr, 3), round(nt.end / sr, 3)] for nt in loop.bass],
                hits=[[round(h.sample / sr, 3), h.role] for h in loop.hits])
    page = Path(__file__).with_name("page.html").read_text().replace("/*DATA*/null", json.dumps(data))
    page = page.replace("pitched stretch (drives the phrase voice)", "bass note (from your hum)")
    (out / "index.html").write_text(page)
    print(f"open {out / 'index.html'}")


if __name__ == "__main__":
    import loop  # run as the importable module, so a pickled Loop is `loop.Loop` and loads anywhere

    loop.main()
