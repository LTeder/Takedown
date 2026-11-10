"""Glitch: the loop's groove as a skeleton, edited like a drill-and-bass producer would.

Voices are tight and crushed (short clipped kicks tuned to the key, trash
snares with a tuned comb, aliased hats with a metallic squeak, an acid saw
bass that plucks on the kicks). `render_loop` then transforms the pattern:
ratchets and rolls, half-time and double-time beats, beat slices re-ordered,
buffer-repeat stutters (some stepping in pitch) and abrupt cuts to silence.
Roll 1 is restrained, rolls 2 and 3 go further. Every edit lands on the
swung 16th grid or a clean subdivision of it (halves, thirds, quarters).
"""

from __future__ import annotations

import sys
import zlib
from copy import copy
from pathlib import Path

import numpy as np
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import sketch  # noqa: E402
from sketch import TAU, crush, env_ad, filt, hold, norm, pad_to, phase_of, sinefold, sweep, t_axis  # noqa: E402

OVER = 2
LEVEL = {"kick": 0.95, "snare": 0.85, "hat": 0.75, "bass": 0.32}
ROLE_IDX = {"kick": 0, "snare": 1, "hat": 2, "bass": 3}


def vel_gain(v: float) -> float:
    return 10 ** ((v - 1) * 24 / 20)


def fade_out(y: np.ndarray, n: int) -> np.ndarray:
    n = min(n, len(y))
    if n > 0:
        y[-n:] *= np.cos(np.linspace(0, np.pi / 2, n)) ** 2
    return y


def gate(y: np.ndarray, length: int, tail: int) -> np.ndarray:
    """Hard gate: hold to `length` samples, then a very short fade."""
    y = y[:length].copy()
    return fade_out(y, tail)


def hp(x, fc, sr, q=0.7):
    return filt(x, "hp", fc, q, sr)


# ------------------------------------------------------------------ voices


def kick(h, p: dict, rng, sr: int) -> np.ndarray:
    pit = getattr(h, "pitch", 1.0)
    v, b = h.vel, h.bright
    L = float(np.clip(0.10 + 0.5 * h.dur, 0.10, 0.17)) * p["kick_len"]
    n = int(L * sr)
    root = p["kick_root"] * pit
    f = sweep(n, sr, root * (3.0 + 2.2 * b), root, p["kick_tau"])
    x = np.sin(phase_of(f, sr))
    x = np.clip(x * (3.0 + 6.0 * v), -1, 1)
    x = sinefold(x, 1.0 + p["kick_fold"] * (0.4 + 0.6 * v))
    x = filt(x, "lp", 1400 + 3600 * b, 0.7, sr) * env_ad(n, sr, 0.0004, 0.045)
    m = pad_to(h.mouth, n)[: int(0.012 * sr)]
    click = np.tanh(6 * hp(pad_to(m, n), 1200, sr)) * env_ad(n, sr, 0.0002, 0.004)
    top = crush(hold(norm(x + (0.25 + 0.9 * b) * p["mouth"] * click), p["hold"]), p["bits"])
    sub = np.sin(phase_of(sweep(n, sr, root * 1.7, root, 0.018), sr)) * env_ad(n, sr, 0.0015, 0.06)
    y = 0.75 * top + 0.85 * sub
    y = hp(y, 32.0, sr, 0.7)
    return norm(gate(y, n, int(0.012 * sr)))


def snare(h, p: dict, rng, sr: int) -> np.ndarray:
    pit = getattr(h, "pitch", 1.0)
    v, b = h.vel, h.bright
    L = float(np.clip(0.07 + 0.55 * h.dur, 0.08, 0.15)) * p["snare_len"]
    n = int(L * sr)
    t = t_axis(n, sr)
    noise = hp(rng.standard_normal(n), 700 + 3800 * b, sr) * env_ad(n, sr, 0.0003, 0.026 + 0.04 * h.dur)
    tone = np.sin(phase_of(sweep(n, sr, p["snare_hz"] * 1.5 * pit, p["snare_hz"] * pit, 0.012), sr)) * env_ad(n, sr, 0.0003, 0.035)
    rm = np.sin(TAU * p["rm_hz"] * (0.6 + 0.8 * b) * t)
    ring = noise * (1 - p["rm_mix"] + p["rm_mix"] * rm)
    tuned = sketch.comb(noise, p["snare_hz"] * 2 * pit, 0.09, sr, 0.3)
    m = pad_to(h.mouth, n)[: int(0.03 * sr)]
    mt = np.tanh(6 * hp(pad_to(m, n), 500, sr)) * env_ad(n, sr, 0.0002, 0.012)
    x = ring + 0.45 * tuned + 1.4 * tone + p["mouth"] * 1.5 * mt
    x = np.clip(norm(x) * (2.0 + 4.0 * v), -1, 1)
    y = crush(hold(x, p["hold"]), p["bits"] + 1)
    y = hp(y, 110.0, sr, 0.7)
    return norm(gate(y, int(n * 0.8), int(0.004 * sr)))


def hat(h, p: dict, rng, sr: int) -> np.ndarray:
    pit = getattr(h, "pitch", 1.0)
    v, b = h.vel, h.bright
    L = float(np.clip(0.014 + 0.22 * max(h.dur - 0.04, 0.0), 0.014, 0.05)) * p["hat_len"]
    n = int(L * sr)
    t = t_axis(n, sr)
    noise = hp(rng.standard_normal(n), 3500 + 5500 * b, sr) * np.sin(TAU * p["hat_rm_hz"] * pit * t)
    noise = hold(noise, p["hat_hold"] + int(6 * (1 - b)))
    fund = 440.0 * pit
    metal = sum(np.sign(np.sin(TAU * fund * r * t)) for r in (2.0, 3.0, 4.16, 5.43, 6.79, 8.21)) / 6
    metal = hp(metal, 4200 + 2500 * b, sr)
    x = noise + p["metal"] * metal
    if b > 0.9 and v > 0.7:  # the brightest accents get an FM squeak tuned to A
        c = 3520.0 * pit
        mod = np.sin(TAU * c * 1.41 * t) * 5.0 * np.exp(-t / 0.006)
        x = x + 0.7 * np.sin(TAU * c * t + mod) * env_ad(n, sr, 0.0003, 0.008)
    m = pad_to(h.mouth, n)[: int(0.01 * sr)]
    x = x + 0.5 * p["mouth"] * np.tanh(5 * hp(pad_to(m, n), 4000, sr))
    x = np.tanh(2.0 * norm(x)) * env_ad(n, sr, 0.0002, L / 2.5)
    return norm(gate(x, n, int(0.003 * sr)))


def tv_lowpass(x: np.ndarray, fc: np.ndarray, q: float, sr: int, block: int = 128) -> np.ndarray:
    y = np.empty_like(x)
    zi = np.zeros(2)
    for i in range(0, len(x), block):
        b, a = sketch.biquad("lp", float(fc[i]), q, sr)
        y[i : i + block], zi = signal.lfilter(b, a, x[i : i + block], zi=zi)
    return y


def bass_note(hz: float, L: float, vel: float, brt: float, p: dict, sr: int) -> np.ndarray:
    """An acid saw pluck: folded, through a resonant low-pass that closes fast, gated, high-passed."""
    delay = int(0.014 * sr)  # lets the kick's click go first
    n = int(L * sr)
    t = t_axis(n, sr)
    f = hz * 2 ** ((3.0 * np.exp(-t / 0.012)) / 12)
    ph = phase_of(f, sr)
    saw = sum(2 * ((ph * 2 ** (d / 1200) / TAU) % 1.0) - 1 for d in (-9.0, 0.0, 9.0)) / 3
    x = 0.65 * saw + 0.7 * np.sin(ph)
    x = sinefold(x, 1.0 + p["bass_fold"] * (0.3 + 0.7 * vel))
    top = np.minimum(hz * (4 + 9 * brt) * (0.6 + 0.6 * vel), 3500.0)
    fc = hz * 1.4 + (top - hz * 1.4) * np.exp(-t / 0.07)
    x = tv_lowpass(x, fc, p["bass_q"], sr)
    x = np.tanh(2.0 * x)
    x = crush(hold(x, p["bass_hold"]), 9.0)
    x = hp(x, 48.0, sr, 0.7)
    env = np.minimum(1.0, t / 0.010) * np.exp(-t / 0.065)
    y = x * env
    y = fade_out(y, int(0.012 * sr))
    return np.concatenate([np.zeros(delay), norm(y)])


def bass(h, p: dict, rng, sr: int) -> np.ndarray:
    hz = 440.0 * 2 ** ((h.midi - 69) / 12) * p["bass_oct"]
    return bass_note(hz, min(h.length, p["bass_max"]), h.vel, h.brt, p, sr)


VOICES = {"kick": kick, "snare": snare, "hat": hat, "bass": bass}


# ------------------------------------------------------------------ params


def params(rng) -> dict:
    d = dict(kick_root=55.0, kick_tau=0.03, kick_fold=2.0, kick_len=1.0, mouth=0.35, bits=7.0, hold=2,
             snare_hz=220.0, snare_len=1.0, rm_hz=1500.0, rm_mix=0.45,
             hat_rm_hz=7300.0, hat_hold=4, hat_len=1.0, metal=0.5,
             bass_fold=1.6, bass_q=5.0, bass_hold=2, bass_oct=1.0, bass_max=0.16)
    if rng is None:
        return d
    d.update(kick_root=float(rng.choice([49.0, 55.0, 55.0, 61.7])), kick_tau=rng.uniform(0.02, 0.045),
             kick_fold=rng.uniform(1.0, 3.5), kick_len=rng.uniform(0.85, 1.2), mouth=rng.uniform(0.2, 0.6),
             bits=float(rng.choice([5.0, 6.0, 7.0, 8.0])), hold=int(rng.integers(1, 5)),
             snare_hz=float(rng.choice([196.0, 220.0, 220.0, 247.0])), snare_len=rng.uniform(0.8, 1.2),
             rm_hz=rng.uniform(1000, 2400), rm_mix=rng.uniform(0.3, 0.7),
             hat_rm_hz=rng.uniform(6000, 10000), hat_hold=int(rng.integers(2, 6)), hat_len=rng.uniform(0.8, 1.3),
             metal=rng.uniform(0.2, 0.9), bass_fold=rng.uniform(0.8, 3.0), bass_q=rng.uniform(3.0, 9.0),
             bass_hold=int(rng.integers(1, 4)), bass_oct=float(rng.choice([1.0, 1.0, 2.0])),
             bass_max=rng.uniform(0.12, 0.2))
    return d


# ------------------------------------------------------------------ events


class Ev:
    """One thing to play: a hit or a bass note, at a position in 16ths (not necessarily an integer)."""

    def __init__(self, role, pos, vel, bright=0.5, dur=0.1, mouth=None, src=0, midi=45, length=0.2, brt=0.5):
        self.role, self.pos, self.vel, self.bright, self.dur = role, float(pos), vel, bright, dur
        self.mouth = mouth if mouth is not None else np.zeros(16)
        self.src, self.pitch, self.amp = src, 1.0, 1.0
        self.midi, self.length, self.brt = midi, length, brt


class Clock:
    """Position in 16ths to samples, through the swung grid (odd 16ths late); halves, thirds etc. are exact fractions."""

    def __init__(self, loop, sr_s):
        self.six, self.sw, self.sr = loop.sixteenth, loop.swing, sr_s

    def slot(self, i: int) -> float:
        return (i + self.sw * (i % 2)) * self.six * self.sr

    def __call__(self, pos: float) -> int:
        i = int(np.floor(pos + 1e-9))
        fr = pos - i
        a, b = self.slot(i), self.slot(i + 1)
        return int(round(a + (b - a) * fr))


def base_events(loop) -> list:
    evs = []
    for h in loop.hits:
        m = signal.resample_poly(h.mouth, OVER, 1) if len(h.mouth) > 8 else np.zeros(16)
        evs.append(Ev(h.role, h.slot, h.vel, h.bright, h.dur, sketch.norm(m), h.sample))
    six = loop.sixteenth
    for nt in loop.bass:
        slot = nt.start / (six * loop.sr)
        evs.append(Ev("bass", round(slot), nt.vel, midi=nt.midi, length=(nt.end - nt.start) / loop.sr, brt=nt.brt,
                      src=nt.start))
    return evs


# ratchet shapes: offsets in 16ths, velocity ramp, pitch ramp (semitones)
SHAPES = {
    "r2": ([0, 0.5], [1.0, 0.65], [0, 0]),
    "r3": ([0, 1 / 3, 2 / 3], [1.0, 0.6, 0.8], [0, 0, 0]),
    "r4": ([0, 0.25, 0.5, 0.75], [1.0, 0.55, 0.8, 0.6], [0, 0, 0, 0]),
    "acc": ([0, 1, 1.5, 2, 2.25, 2.5, 2.75, 3], [0.55, 0.6, 0.65, 0.7, 0.8, 0.85, 0.95, 1.0], [0] * 8),
    "dec": ([0, 0.25, 0.5, 0.75, 1, 1.5, 2, 3], [1.0, 0.95, 0.9, 0.8, 0.7, 0.65, 0.6, 0.55], [0] * 8),
    "snr": ([0, 1, 2, 2.5, 3, 3.5], [0.6, 0.65, 0.75, 0.8, 0.9, 1.0], [0] * 6),
    "rise": ([0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5], [0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.95, 1.0], [0, 1, 2, 3, 4, 5, 6, 7]),
    "drop": ([0, 0.5, 1, 1.5], [1.0, 0.85, 0.7, 0.6], [0, -1, -3, -5]),
}


def ratchet(e: Ev, shape: str, span: float = 4.0) -> list:
    offs, vels, pits = SHAPES[shape]
    out = []
    for k, (o, vv, pp) in enumerate(zip(offs, vels, pits)):
        if o >= span:
            continue
        c = copy(e)
        c.pos = e.pos + o
        c.vel = float(np.clip(e.vel * vv + (0.05 if k == 0 else 0.0), 0.05, 1.0))
        c.pitch = e.pitch * 2 ** (pp / 12)
        c.src = e.src + 101 * k
        if k > 0:
            c.mouth = np.zeros(16)
        out.append(c)
    return out


def half_time(evs: list, a: float) -> list:
    """One beat (4 sixteenths from a) stretched over two; the second beat's own events are dropped."""
    out = []
    for e in evs:
        if a <= e.pos < a + 4:
            e = copy(e)
            e.pos = a + (e.pos - a) * 2
            e.length *= 2
        elif a + 4 <= e.pos < a + 8:
            continue
        out.append(e)
    return out


def double_time(evs: list, a: float) -> list:
    """Two beats from a squeezed into one, played twice."""
    out, add = [], []
    for e in evs:
        if a <= e.pos < a + 8:
            e = copy(e)
            e.pos = a + (e.pos - a) / 2
            e.length /= 2
            c = copy(e)
            c.pos += 4
            add.append(c)
        out.append(e)
    return out + add


def dedupe(evs: list) -> list:
    seen = {}
    for e in sorted(evs, key=lambda e: (e.role, e.pos)):
        seen[(e.role, round(e.pos * 96))] = e
    return sorted(seen.values(), key=lambda e: (e.pos, e.role))


# ------------------------------------------------------------------ plan


def transform(loop, roll: int, evs: list) -> tuple[list, list]:
    """Event-level edits, and a list of stream-level edits ("slice", "stutter", "cut") to make after rendering."""
    k = min(roll - 1, 2)
    rng = np.random.default_rng([zlib.crc32(b"glitch-plan"), roll])
    ops = []
    bars = 8
    last = bars * 16
    claimed = {0, 4 * bars - 4, 4 * bars - 3, 4 * bars - 2, 4 * bars - 1}  # beat indices already spoken for

    def pick_beat(lo: int = 4, hi: int = 4 * (bars - 1)) -> int:
        for _ in range(80):
            b = int(rng.integers(lo, hi))
            if b not in claimed and b % 4 != 0:
                claimed.add(b)
                return b
        for b in range(lo, hi):
            if b not in claimed:
                claimed.add(b)
                return b
        return lo

    def ratchet_all(evs, prob):
        out = []
        for e in evs:
            pr = prob.get(e.role, 0.0)
            r = rng.random()
            c = rng.integers(0, 6)
            if e.role != "bass" and 16 <= e.pos < last - 8 and r < pr:
                shapes = {"hat": ["r2", "r3", "r4", "acc", "dec", "rise"], "snare": ["r2", "r3", "snr", "r4", "r2", "r3"],
                          "kick": ["r2", "r2", "drop", "r2", "drop", "r2"]}[e.role]
                out += ratchet(e, shapes[int(c)], 3.0)
            else:
                out.append(e)
        return out

    if k == 0:
        # restrained: hat pairs at the ends of odd bars, a snare roll into the loop point, one hole, one stutter
        out = []
        for e in evs:
            if e.role == "hat" and e.pos in (31.0, 63.0, 95.0):
                out += ratchet(e, "r2")
            elif e.role == "snare" and e.pos >= last - 4:
                continue
            else:
                out.append(e)
        out += ratchet(Ev("snare", last - 4, 0.7, 0.6, 0.12, None, 5001), "snr", 4.0)
        evs = out
        ops += [("cut", (62.0, 64.0, None)), ("stutter", (92.0, 0.5, 4, None, None))]
    else:
        if k == 1:
            evs = half_time(evs, 80.0)
            claimed.add(20)
        else:
            evs = half_time(evs, 80.0)
            evs = double_time(evs, 40.0)
            claimed |= {10, 11, 20}
        evs = ratchet_all(evs, {"hat": (0.12, 0.25)[k - 1], "snare": (0.15, 0.3)[k - 1], "kick": (0.06, 0.12)[k - 1]})
        # fill: snare rolls over the last two beats (and a pitch-dropping kick roll in roll 3)
        evs = [e for e in evs if not (e.role in ("snare", "kick") and e.pos >= last - 8)]
        evs += ratchet(Ev("snare", last - 8, 0.6, 0.6, 0.12, None, 5002), "acc", 4.0)
        evs += ratchet(Ev("snare", last - 4, 0.8, 0.7, 0.12, None, 5003), "snr", 4.0)
        if k == 2:
            evs += ratchet(Ev("kick", last - 8, 0.9, 0.6, 0.12, None, 5004), "drop", 4.0)
            evs += ratchet(Ev("hat", last - 6, 0.7, 0.9, 0.1, None, 5005), "rise", 2.0)
        ops.append(("cut", (last - (1.0 if k == 1 else 2.0), float(last), None)))
        for _ in range(1 + k):  # beat slices re-ordered
            dst = pick_beat()
            src = int(rng.integers(0, 4 * (bars - 1)))
            while src == dst:
                src = int(rng.integers(0, 4 * (bars - 1)))
            ops.append(("slice", (4.0 * dst, 4.0 * src, bool(k == 2 and rng.random() < 0.4))))
        for _ in range(1 + k):  # buffer-repeat stutters
            b = pick_beat()
            start = 4.0 * b + float(rng.choice([0.0, 2.0]))
            win = float(rng.choice([1.0, 0.5, 0.5, 1.0, 2.0]))
            reps = int(round((4.0 if start % 4 == 0 else 2.0) / win))
            ladder = None
            if k == 2 and rng.random() < 0.7:
                lad = [[0, 2, 4, 7, 9, 12, 14, 16], [0, -2, -5, -7, -9, -12, -14, -17], [0, 7, 0, 7, 12, 7, 12, 19]][int(rng.integers(0, 3))]
                ladder = (lad + [lad[-1]] * reps)[:reps]
            roles = None if rng.random() < 0.6 else [["kick", "snare", "hat", "bass"][int(rng.integers(0, 4))]]
            ops.append(("stutter", (start, win, reps, ladder, roles)))
        for _ in range(1 + k + (k == 2)):  # cuts to silence
            b = pick_beat()
            a = 4.0 * b + float(rng.choice([0.0, 1.0, 2.0, 3.0]))
            ln = float(rng.choice([0.5, 1.0, 1.0, 2.0]))
            roles = None if rng.random() < 0.5 else [["kick", "snare", "hat", "bass"][int(rng.integers(0, 4))]]
            ops.append(("cut", (a, a + ln, roles)))
    return dedupe(evs), ops


# ------------------------------------------------------------------ rendering


def place(evs: list, role: str, p: dict, roll: int, clock: Clock, n_total: int, sr: int) -> np.ndarray:
    out = np.zeros(n_total)
    mine = sorted((e for e in evs if e.role == role), key=lambda e: e.pos)
    starts = [clock(e.pos) for e in mine]
    fade = int(0.0015 * sr)
    for i, e in enumerate(mine):
        n0 = starts[i]
        if n0 >= n_total:
            continue
        rng = np.random.default_rng([roll, ROLE_IDX[role], int(e.pos * 96), e.src & 0xFFFF])
        y = VOICES[role](e, p, rng, sr) * LEVEL[role] * vel_gain(e.vel) * e.amp
        if i + 1 < len(mine):
            room = starts[i + 1] - n0
            if room <= 0:
                continue
            if room < len(y):
                y = fade_out(y[: room + fade].copy(), fade)
        y = y[: n_total - n0]
        out[n0 : n0 + len(y)] += y
    return out


def apply_streams(st: dict, ops: list, clock: Clock, sr: int) -> dict:
    n_total = len(next(iter(st.values())))
    orig = {r: y.copy() for r, y in st.items()}
    f1 = max(1, int(0.0005 * sr))
    for kind, args in ops:
        if kind == "slice":
            dst, src, rev = args
            a, b = clock(dst), clock(dst + 4.0)
            s0 = clock(src)
            for r in st:
                seg = pad_to(orig[r][s0 : s0 + (b - a)], b - a)
                if rev and r in ("hat", "snare"):
                    seg = seg[::-1]
                seg = seg.copy()
                seg[:f1] *= np.linspace(0, 1, f1)
                seg = fade_out(seg, f1)
                st[r][a:b] = seg
        elif kind == "stutter":
            start, win, reps, ladder, roles = args
            a = clock(start)
            for r in st:
                if roles is not None and r not in roles:
                    continue
                w = clock(start + win) - a
                src = st[r][a : a + w].copy()
                if len(src) < 2:
                    continue
                for j in range(reps):
                    d0, d1 = clock(start + j * win), min(n_total, clock(start + (j + 1) * win))
                    ln = d1 - d0
                    if ln <= 0:
                        continue
                    rate = 2 ** ((ladder[j] if ladder else 0) / 12)
                    seg = np.interp(np.arange(ln) * rate, np.arange(len(src)), src, right=0.0)
                    if j:
                        seg[:f1] *= np.linspace(0, 1, f1)
                    st[r][d0:d1] = fade_out(seg, f1)
        elif kind == "cut":
            a0, b0, roles = args
            a, b = clock(a0), min(n_total, clock(b0))
            for r in st:
                if roles is not None and r not in roles:
                    continue
                st[r][a:b] = 0.0
                if b < n_total:
                    m = min(f1, n_total - b)
                    st[r][b : b + m] *= np.linspace(0, 1, m)
    return st


def render_loop(loop, p: dict, roll: int) -> np.ndarray:
    sr_s = loop.sr * OVER
    n_total = loop.n * OVER
    clock = Clock(loop, sr_s)
    evs, ops = transform(loop, roll, base_events(loop))
    streams = {r: place(evs, r, p, roll, clock, n_total, sr_s) for r in ("kick", "snare", "hat", "bass")}
    streams = apply_streams(streams, ops, clock, sr_s)
    return signal.resample_poly(sum(streams.values()), 1, OVER)


# ------------------------------------------------------------------ fallback phrase voice (render_loop does the real work)


def phrase(c: sketch.Controls, p: dict, rng, sr: int) -> np.ndarray:
    ph = phase_of(c.f, sr)
    x = 2 * ((ph / TAU) % 1.0) - 1
    x = sinefold(x, 1 + p["bass_fold"] * (c.amp + c.acc))
    x = filt(x, "lp", 700.0, p["bass_q"], sr)
    return np.tanh(2 * x) * c.amp


PALETTE = sketch.Palette(
    name="glitch",
    blurb="Short, crushed and tightly gated drums in A with a squelchy acid bass, chopped up like drill-and-bass: "
          "ratchets, stutters, half-time beats and sudden dropouts, with the groove kept underneath.",
    hits={"kick": kick, "snare": snare, "hat": hat},
    phrase=phrase,
    params=params,
    levels={"kick": 1.0, "snare": 1.0, "hat": 1.0, "phrase": 1.0},
)
PALETTE.render_loop = render_loop
