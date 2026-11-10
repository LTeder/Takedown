"""westcoast: Buchla-style drums and bass for the loop.

Everything is a complex oscillator (a modulator FM-ing a carrier), a multi-stage
wavefolder whose depth is the timbre control, and a low-pass gate: a one-pole
filter and VCA driven together by a vactrol-like envelope (fast first decay,
slow second decay, a little lag), pinged by a short trigger. The ping is the drum.

The kit is tuned to the bassline: the kick's sub follows the bass note, the snare's
tone sits a fifth above it, and the bass plays an octave above the kick's sub so
the two share no low frequency.
"""

from __future__ import annotations

import dataclasses
import sys
from pathlib import Path

import numpy as np
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import sketch  # noqa: E402

TAU = 2 * np.pi


# ------------------------------------------------------------------ west coast blocks


def fold(x, g, bias=0.0):
    """One sine wavefolder stage; g is the fold depth (1 = barely folded)."""
    return np.sin(0.5 * np.pi * (g * x + bias))


def vactrol(n, sr, strike, t_fast, t_slow, mix, att=0.0005):
    """Envelope of an LPG cell: fast + slow decay, with a short attack lag."""
    t = np.arange(n) / sr
    e = (1 - mix) * np.exp(-t / t_fast) + mix * np.exp(-t / t_slow)
    return strike * e * (1 - np.exp(-t / att))


def tv_lowpass(x, fc, sr, poles=2, block=32):
    """Cascaded one-pole low-pass with a cutoff that follows `fc` (block-wise)."""
    y = x.copy()
    zis = [np.zeros(1) for _ in range(poles)]
    for i in range(0, len(x), block):
        a = 1 - np.exp(-TAU * min(float(fc[i]), 0.45 * sr) / sr)
        seg = y[i : i + block]
        for k in range(poles):
            seg, zis[k] = signal.lfilter([a], [1.0, a - 1.0], seg, zi=zis[k])
        y[i : i + block] = seg
    return y


def lpg(x, e, fc_lo, fc_hi, sr, lag=0.003, vca_pow=1.0, poles=2):
    """Low-pass gate: the same envelope opens the filter and the VCA.
    The filter cutoff lags the envelope a little (the cell's thermal tail)."""
    a = np.exp(-1 / (lag * sr))
    el = signal.lfilter([1 - a], [1, -a], e)
    fc = fc_lo * (fc_hi / fc_lo) ** np.clip(el, 0, 1)
    return tv_lowpass(x, fc, sr, poles) * np.clip(e, 0, None) ** vca_pow


def phase(f, sr):
    return TAU * np.cumsum(f) / sr


def pad(x, n):
    return sketch.pad_to(x, n)


def peak1(x):
    return x / (np.abs(x).max() + 1e-9)


# ------------------------------------------------------------------ voices


def kick(h, p, rng, sr):
    n = int(0.40 * sr)
    t = np.arange(n) / sr
    root = p["kick_root_now"]
    strike = 0.55 + 0.45 * h.vel
    octs = p["kick_octs"] + 1.2 * h.bright
    f = root + (root * 2**octs - root) * np.exp(-t / p["kick_tau"])
    ph = phase(f, sr)
    e = vactrol(n, sr, 1.0, 0.028 + 0.15 * h.dur, 0.10 + 0.55 * h.dur, 0.40)
    ef = np.exp(-t / 0.012)  # the strike transient
    idx = (0.4 + 2.2 * h.vel) * ef * p["kick_fm"]
    x = np.sin(ph + idx * np.sin(p["kick_ratio"] * ph))
    g = 1.0 + p["kick_fold"] * (0.25 + 0.75 * h.vel) * (0.2 + 0.8 * np.exp(-t / 0.035))
    y = fold(fold(x, g), 1.0 + 0.5 * p["kick_fold"] * h.bright * ef, 0.15 * ef)
    ceil = min(14000.0, p["kick_ceil"] * 2 ** (1.8 * h.bright) * (0.5 + h.vel))
    y = lpg(y, e * strike, 70.0, ceil, sr, lag=0.004, vca_pow=0.9)
    return peak1(y)


def snare(h, p, rng, sr):
    n = int(0.42 * sr)
    t = np.arange(n) / sr
    tone = p["snare_tone_now"]
    f = tone * (1 + 1.1 * np.exp(-t / 0.012))
    ph = phase(f, sr)
    # complex oscillator: a folded, softened square plus an inharmonic FM partner
    sq = np.tanh(3.0 * np.sin(ph + 0.6 * np.sin(p["snare_ratio"] * ph) * np.exp(-t / 0.03)))
    e1 = vactrol(n, sr, 0.6 + 0.4 * h.vel, 0.018, 0.075, 0.30)
    g = 1.5 + p["snare_fold"] * (0.3 + 0.7 * h.vel) * (0.3 + 0.7 * np.exp(-t / 0.03))
    body = lpg(fold(sq, g), e1, 300.0, 5500 * 2 ** (1.2 * h.bright), sr, vca_pow=1.0)
    nz = rng.standard_normal(n)
    nz = signal.lfilter(*signal.butter(2, 700 * 2 ** (1.6 * h.bright) / (0.5 * sr), "high"), nz)
    nz = nz + 0.5 * pad(h.mouth, n) * 4  # the mouth's own crack excites the second gate
    nz = fold(nz, 1.0 + 1.5 * h.vel)
    tail = 0.06 + 0.5 * h.dur
    e2 = vactrol(n, sr, 0.7 + 0.3 * h.vel, 0.012, tail, 0.42, att=0.0003)
    hiss = lpg(nz, e2, 900.0, 10000 * 2 ** (2.2 * (h.bright - 0.5)), sr, vca_pow=1.0)
    return np.tanh(1.6 * ((1.15 - 0.75 * h.bright) * peak1(body) + (0.45 + 0.9 * h.bright) * peak1(hiss)))


def hat(h, p, rng, sr):
    n = int(0.16 * sr)
    t = np.arange(n) / sr
    c = p["hat_hz"] * 2 ** (0.9 * h.bright)
    ph = phase(np.full(n, c), sr) + rng.uniform(0, TAU)
    idx = (3.0 + 5.0 * h.vel) * (0.4 + 0.6 * np.exp(-t / 0.01))
    x = np.sin(ph + idx * np.sin(1.4706 * ph) + 0.6 * idx * np.sin(2.0932 * ph + 1))
    x = fold(x, 1.5 + 2.5 * (0.3 + 0.7 * h.bright) * (0.5 + 0.5 * h.vel))
    x = signal.lfilter(*signal.butter(2, 4000 * 2 ** (1.0 * h.bright) / (0.5 * sr), "high"), x)
    decay = 0.007 + 0.035 * float(np.clip((h.dur - 0.1) / 0.15, 0, 1)) * p["hat_open"]
    e = vactrol(n, sr, 0.6 + 0.4 * h.vel, decay, decay * 3.0, 0.2, att=0.0002)
    y = lpg(x, e, 2500.0, min(20000.0, 6000 * 2 ** (2.2 * h.bright)), sr, lag=0.0015, vca_pow=1.0)
    return peak1(y)


def phrase(c, p, rng, sr):
    f = c.f
    ph = phase(f, sr)
    idx = p["bass_i0"] + p["bass_i_amp"] * c.amp + p["bass_i_acc"] * c.acc
    x = np.sin(ph + idx * np.sin(p["bass_ratio"] * ph))
    # fold depth follows the accent and the loudness: plucks bite, then relax
    g = 1.0 + p["bass_fold"] * (0.25 * c.amp + 0.75 * c.acc) + 0.8 * c.brt
    y = fold(fold(x, g), 1.2 + 0.4 * c.acc, 0.25)
    y = signal.lfilter(*signal.butter(2, 55 / (0.5 * sr), "high"), y)
    e = c.amp
    y = lpg(y, e, 110.0, p["bass_ceil"], sr, lag=0.006, vca_pow=1.6, poles=2)
    return y


# ------------------------------------------------------------------ params


def params(rng):
    d = dict(kick_octs=3.0, kick_tau=0.014, kick_fm=1.0, kick_ratio=2.0, kick_fold=2.5, kick_ceil=2500.0,
             snare_ratio=1.5, snare_fold=3.5, hat_hz=3300.0, hat_open=1.0,
             bass_i0=0.3, bass_i_amp=0.9, bass_i_acc=1.6, bass_ratio=2.0, bass_fold=2.5, bass_ceil=1500.0,
             ratchet=0.0)
    if rng is None:
        return d
    d.update(kick_octs=rng.uniform(2.4, 3.6), kick_tau=rng.uniform(0.009, 0.022), kick_fm=rng.uniform(0.5, 1.6),
             kick_ratio=float(rng.choice([1.0, 2.0, 3.0, 1.5])), kick_fold=rng.uniform(1.5, 4.0),
             kick_ceil=rng.uniform(1600, 4000), snare_ratio=float(rng.choice([1.0, 1.5, 2.76, 3.0])),
             snare_fold=rng.uniform(2.0, 5.5), hat_hz=rng.uniform(2400, 4600), hat_open=rng.uniform(0.6, 1.6),
             bass_i0=rng.uniform(0.1, 0.6), bass_i_amp=rng.uniform(0.4, 1.4), bass_i_acc=rng.uniform(0.8, 2.4),
             bass_ratio=float(rng.choice([1.0, 2.0, 2.0, 3.0, 0.5])), bass_fold=rng.uniform(1.5, 3.8),
             bass_ceil=rng.uniform(1400, 3200), ratchet=rng.uniform(0.04, 0.10))
    return d


# ------------------------------------------------------------------ whole loop


def _fold_midi(m, lo, hi):
    while m < lo:
        m += 12
    while m > hi:
        m -= 12
    return m


def render_loop(lp, p, roll):
    import loop as L

    OVER, sr_s, n_s = L.OVER, lp.sr * L.OVER, lp.n * L.OVER
    # the bass plays between D2 and C#3, an octave above the kick's sub
    notes = [dataclasses.replace(b, midi=_fold_midi(b.midi, 38, 49)) for b in lp.bass]
    starts = np.array([b.start for b in notes])

    def note_at(sample):
        i = int(np.searchsorted(starts, sample, side="right")) - 1
        return notes[max(i, 0)].midi if notes else 45

    hits = list(lp.hits)
    if p["ratchet"] > 0:  # rolls 2 and 3: some hats stutter into a second hit half a sixteenth later
        rr = np.random.default_rng([roll, 77])
        half = int(lp.sixteenth * lp.sr * 0.5)
        for h in [x for x in lp.hits if x.role == "hat"]:
            if rr.random() < p["ratchet"]:
                hits.append(dataclasses.replace(h, sample=h.sample + half, src=h.src + 1, vel=h.vel * 0.75))
    out = np.zeros(n_s)
    fade = int(0.003 * sr_s)
    levels = PALETTE.levels
    for ri, role in enumerate(("kick", "snare", "hat")):
        group = sorted((h for h in hits if h.role == role), key=lambda h: h.sample)
        for i, h in enumerate(group):
            n0 = h.sample * OVER
            m = note_at(h.sample)
            q = dict(p)
            hz = 440.0 * 2 ** ((m - 69) / 12)
            q["kick_root_now"] = hz / 2 if hz / 2 >= 40 else hz
            sn = 440.0 * 2 ** ((m + 7 - 69) / 12)
            while sn > 250:
                sn /= 2
            while sn < 160:
                sn *= 2
            q["snare_tone_now"] = sn
            mouth = signal.resample_poly(h.mouth, OVER, 1) if len(h.mouth) > 8 else np.zeros(16)
            hit = sketch.Hit(n0=n0, role=role, vel=h.vel, bright=h.bright, dur=h.dur, mouth=sketch.norm(mouth), pre=0)
            y = PALETTE.hits[role](hit, q, np.random.default_rng([roll, ri, h.src]), sr_s)
            if i + 1 < len(group):
                room = group[i + 1].sample * OVER - n0 + fade
                if room < len(y):
                    y = y[:room].copy()
                    y[-fade:] *= np.cos(np.linspace(0, np.pi / 2, fade)) ** 2
            y = y[: max(0, n_s - n0)]
            out[n0 : n0 + len(y)] += y * levels[role] * 10 ** ((h.vel - 1) * 24 / 20)
    if notes:
        lp2 = dataclasses.replace(lp, bass=notes)
        ctrl = L.bass_controls(lp2, sr_s)
        out += L.BASS_GAIN * phrase(ctrl, p, np.random.default_rng([roll, 9]), sr_s)[:n_s] * levels["phrase"]
    return signal.resample_poly(out, 1, OVER)


def _default_voice_check(h, p, rng, sr):  # voices called outside render_loop still work
    q = dict(p, kick_root_now=52.0, snare_tone_now=196.0)
    return kick(h, q, rng, sr)


PALETTE = sketch.Palette(
    name="westcoast",
    blurb="Round, bubbly West Coast synth drums: kicks that boing, snares that ping and bark, hats like wet metal, and a plucked bass that bites then relaxes.",
    hits={"kick": kick, "snare": snare, "hat": hat},
    phrase=phrase,
    params=params,
    levels={"kick": 1.0, "snare": 1.25, "hat": 0.9, "phrase": 0.45},
)
PALETTE.render_loop = render_loop
