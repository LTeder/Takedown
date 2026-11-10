"""filterscream: drums as filter pings and screaming band-passes, an acid bass on a
hard-sync wavetable through a self-singing resonant low-pass.

Everything is tuned to the loop's key (A minor): the kick's floor, the snare's
body and its tuned-noise comb, the pitches the hats ring at, and the bass's
resonance, which tracks the note it plays.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import numpy as np  # noqa: E402
from scipy import signal  # noqa: E402

import sketch  # noqa: E402

TAU = 2 * np.pi
# A minor, high octaves, for the hats to ring at (Hz)
HAT_SCALES = {
    "penta": [3520.0, 4186.0, 5274.0, 6272.0, 7040.0, 8372.0, 10548.0],      # A C E G A C E
    "minor": [3520.0, 4699.0, 5274.0, 6272.0, 7040.0, 8372.0, 9397.0],        # A D E G A C D
    "fifths": [3520.0, 5274.0, 7040.0, 5274.0, 7040.0, 10548.0, 10548.0],     # A E A E A E E
}


# ------------------------------------------------------------------ filters


def coefs(kind: str, fc, q, sr: int):
    fc = np.clip(np.asarray(fc, float), 20.0, 0.45 * sr)
    w = TAU * fc / sr
    al = np.sin(w) / (2 * np.asarray(q, float))
    c = np.cos(w)
    if kind == "lp":
        b0, b1, b2 = (1 - c) / 2, 1 - c, (1 - c) / 2
    elif kind == "hp":
        b0, b1, b2 = (1 + c) / 2, -(1 + c), (1 + c) / 2
    else:
        b0, b1, b2 = al, np.zeros_like(al), -al
    a0 = 1 + al
    return (np.stack([b0 / a0, b1 / a0, b2 / a0], -1), np.stack([np.ones_like(a0), -2 * c / a0, (1 - al) / a0], -1))


def tvf(x, kind: str, fc, q, sr: int, block: int = 32):
    """Resonant filter whose cutoff and Q follow arrays, coefficients updated per block, state carried over."""
    n = len(x)
    starts = np.arange(0, n, block)
    fcb = np.asarray(fc, float)
    fcb = fcb[starts + np.minimum(block // 2, n - 1 - starts)] if fcb.ndim else np.full(len(starts), float(fcb))
    qb = np.asarray(q, float)
    qb = qb[starts + np.minimum(block // 2, n - 1 - starts)] if qb.ndim else np.full(len(starts), float(qb))
    b, a = coefs(kind, fcb, qb, sr)
    y = np.empty(n)
    zi = np.zeros(2)
    for k, s in enumerate(starts):
        y[s : s + block], zi = signal.lfilter(b[k], a[k], x[s : s + block], zi=zi)
    return y


def ping(n: int, sr: int, f, tau: float, phase0: float = 0.0):
    """A filter pinged by an impulse: a decaying sine at f (array allowed) with time constant tau."""
    t = sketch.t_axis(n, sr)
    ph = TAU * np.cumsum(np.broadcast_to(np.asarray(f, float), (n,))) / sr + phase0
    return np.sin(ph) * np.exp(-t / tau)


def gate_end(y, sr: int, ms: float = 6.0):
    k = min(len(y), int(ms * 1e-3 * sr))
    if k > 1:
        y[-k:] *= np.cos(np.linspace(0, np.pi / 2, k)) ** 2
    return y


def soft(x, g):
    return np.tanh(g * x) / np.tanh(g)


# ------------------------------------------------------------------ kick


def kick(h, p, rng, sr):
    """Self-oscillating filter: a sine that falls from a high cutoff to the key's root, squashed by the
    filter's own limiter, with FM sidebands and a swept band-pass snap on the front."""
    length = float(np.clip(0.30 + 0.5 * h.dur, 0.30, 0.46)) * p["k_len"]
    n = int(length * sr)
    t = sketch.t_axis(n, sr)
    root = p["k_root"]
    octs = p["k_octs"] * (0.35 + 0.65 * h.bright) * (0.75 + 0.25 * h.vel)
    f_hi = root * 2 ** octs
    tau_p = p["k_tau"] * (1.25 - 0.5 * h.vel)
    f = root * (1 + p["k_glide"] * np.exp(-t / 0.07)) + (f_hi - root) * np.exp(-t / tau_p)
    ph = sketch.phase_of(f, sr)
    idx = p["k_fm"] * (0.4 + h.vel) * np.exp(-t / 0.010)
    body = np.sin(ph + idx * np.sin(p["k_ratio"] * ph))
    body = soft(body, 1.6 + 3.2 * h.vel * p["k_drive"])
    env = np.minimum(1.0, t / 0.0006) * np.exp(-t / ((0.075 + 0.07 * min(h.dur, 0.4)) * p["k_len"]))
    y = body * env
    # the snap: noise and the mouth's transient through a band-pass that screams downward
    nc = int(0.012 * sr)
    tc = sketch.t_axis(nc, sr)
    exc = rng.standard_normal(nc) + 3.0 * p["mouth"] * sketch.pad_to(h.mouth, nc)
    fc = (1500.0 + 4500.0 * h.bright) * np.exp(-tc / 0.004) + 400.0
    snap = tvf(exc, "bp", fc, 5.0, sr, 16) * np.exp(-tc / 0.0022)
    snap = sketch.norm(snap) * (0.04 + 0.55 * h.bright) * p["k_snap"]
    y[:nc] += snap
    return gate_end(y, sr, 8.0)


# ------------------------------------------------------------------ snare


def snare(h, p, rng, sr):
    """Noise through a band-pass whose resonance rings and whose centre falls, plus a pinged body and a
    comb that makes the noise sing a note of the key."""
    length = float(np.clip(0.15 + 0.7 * h.dur, 0.15, 0.30))
    n = int(length * sr)
    t = sketch.t_axis(n, sr)
    nz = rng.standard_normal(n) + 2.0 * p["mouth"] * sketch.pad_to(h.mouth, n)
    fc = p["sn_fc"] * 2 ** (2.0 * h.bright - 0.9) * (1.0 + 1.6 * np.exp(-t / 0.025))
    q = p["sn_q"] * (1.0 + 0.5 * h.vel)
    scream = sketch.norm(tvf(nz, "bp", fc, q, sr, 32))
    scream = soft(scream * (1.5 + 5 * h.vel), 2.0) * sketch.env_ad(n, sr, 0.0003, 0.028 + 0.1 * h.dur)
    # high ring, a pinged filter at the key's upper octave
    hiring = ping(n, sr, p["sn_ring"] * (1 + 0.35 * np.exp(-t / 0.008)), 0.03)
    # the comb: noise burst in a feedback comb tuned to the key
    burst = nz[: int(0.004 * sr)] * np.hanning(int(0.004 * sr))
    cmb = sketch.comb(sketch.pad_to(burst, n), p["sn_tune"] * 2, 0.08 + 0.12 * h.dur, sr, 0.35)
    cmb = sketch.norm(cmb)
    # body
    fb = p["sn_tune"] * (1 + 0.9 * np.exp(-t / 0.010))
    body = soft(np.sin(sketch.phase_of(fb, sr)), 2.5) * np.exp(-t / 0.05)
    y = 0.75 * scream + 0.45 * p["sn_comb"] * cmb + 0.65 * body + 0.18 * hiring
    y = soft(y, 1.2 + 1.5 * h.vel)
    return gate_end(y, sr, 6.0)


# ------------------------------------------------------------------ hat


def hat_one(h, p, rng, sr, n: int, level: float = 1.0):
    t = sketch.t_axis(n, sr)
    scale = HAT_SCALES[p["hat_scale"]]
    pos = np.clip(h.bright, 0, 0.999) * (len(scale) - 1)
    f0 = scale[int(round(pos))]
    is_open = h.dur > 0.18
    tau = (0.045 if is_open else 0.007 + 0.012 * h.vel) * p["hat_len"]
    sq = 1 + p["hat_squeak"] * (1 - np.exp(-t / 0.006))  # the squeak: every ping glides up a little
    y = np.zeros(n)
    for r, g in ((1.0, 1.0), (1.5, 0.7), (2.01, 0.45)):
        y += g * ping(n, sr, f0 * r * sq * (1 + 0.003 * rng.standard_normal()), tau / (0.8 + 0.4 * r), rng.uniform(0, TAU))
    nz = rng.standard_normal(n) + 2.0 * p["mouth"] * sketch.pad_to(h.mouth, n)
    nz = signal.lfilter(*sketch.biquad("bp", f0 * 0.9 * 2 ** (0.6 * (h.bright - 0.5)), p["hat_q"], sr), nz)
    nz = sketch.norm(nz) * np.exp(-t / (tau * 0.8))
    y = 0.65 * y + 0.9 * nz
    y = soft(y / (np.abs(y).max() + 1e-9), 1.5 + 2.0 * h.vel)
    return y * level * np.minimum(1.0, t / 0.0002)


def hat(h, p, rng, sr):
    is_open = h.dur > 0.18
    n = int((0.22 if is_open else 0.06) * sr)
    y = hat_one(h, p, rng, sr, n)
    if (not is_open) and h.vel > 0.35 and rng.random() < p["ratchet"]:  # a drill-and-bass triplet burst
        gap = int(0.043 * sr)
        out = np.zeros(n + 2 * gap)
        out[:n] += y
        for k in (1, 2):
            out[k * gap : k * gap + n] += 0.75 ** k * hat_one(h, p, rng, sr, n)
        y = out
    return gate_end(y, sr, 2.0)


# ------------------------------------------------------------------ acid bass


def bass(c, p, rng, sr):
    n = len(c.f)
    f = c.f * 2.0 ** p["b_oct"]
    ph = np.cumsum(f) / sr
    m = ph % 1.0
    # wavetable by hard sync: the slave saw's ratio is scanned by brightness and each pluck's accent
    ratio = 1.0 + p["b_sync"] * (0.35 * c.brt + 0.65 * c.acc)
    slave = 2 * ((ratio * m) % 1.0) - 1
    master = 2 * m - 1
    x = (1 - p["b_wave"]) * master + p["b_wave"] * slave
    x = np.tanh((1.0 + p["b_drive"] * (0.4 + c.acc)) * x)
    # cutoff follows the note, pluck envelope and accent open it, the fundamental wobbles it (filter FM)
    e = 0.6 * c.amp + 0.9 * c.acc
    octs = p["b_lo"] + p["b_span"] * e + p["b_brt"] * (c.brt - 0.5)
    octs = octs + p["b_fm"] * (0.3 + c.acc) * np.sin(TAU * ph * p["b_fm_ratio"])
    fc = np.clip(f * 2.0**octs, 90.0, 7000.0)
    q = p["b_q"] * (1.0 + 0.5 * c.acc)
    y = tvf(x, "lp", fc, q, sr, 64)
    y = tvf(np.tanh(0.6 * y), "lp", fc * 1.4, 0.8, sr, 64)
    y = signal.sosfilt(signal.butter(2, p["b_hp"], "highpass", fs=sr, output="sos"), y)
    y = y / (np.percentile(np.abs(y), 99.5) + 1e-9)
    return np.tanh(1.2 * y) * np.clip(c.amp, 0, 1) ** p["b_gate"]


# ------------------------------------------------------------------ params


def params(rng):
    d = dict(
        k_root=55.0, k_octs=3.4, k_tau=0.020, k_glide=0.25, k_fm=1.1, k_ratio=2.0, k_drive=1.0, k_len=1.0, k_snap=1.0,
        sn_tune=220.0, sn_fc=3200.0, sn_q=9.0, sn_ring=5274.0, sn_comb=1.0,
        hat_scale="penta", hat_squeak=0.22, hat_q=14.0, hat_len=1.0, ratchet=0.12,
        mouth=0.30,
        b_oct=0.0, b_wave=0.6, b_sync=2.2, b_drive=1.6, b_lo=1.2, b_span=3.2, b_brt=1.0, b_fm=0.25, b_fm_ratio=1.0,
        b_q=9.0, b_hp=80.0, b_gate=1.4,
    )
    if rng is None:
        return d
    d.update(
        k_root=float(rng.choice([55.0, 49.0, 43.65, 51.9])), k_octs=rng.uniform(2.6, 4.4), k_tau=rng.uniform(0.012, 0.035),
        k_glide=rng.uniform(0.05, 0.45), k_fm=rng.uniform(0.3, 2.0), k_ratio=float(rng.choice([1.0, 2.0, 2.0, 3.0, 1.5])),
        k_drive=rng.uniform(0.6, 1.6), k_len=rng.uniform(0.75, 1.15), k_snap=rng.uniform(0.6, 1.6),
        sn_tune=float(rng.choice([220.0, 165.0, 196.0, 261.6, 247.0])), sn_fc=rng.uniform(2200, 4800),
        sn_q=rng.uniform(5, 16), sn_ring=float(rng.choice([3520.0, 5274.0, 6272.0, 7040.0])), sn_comb=rng.uniform(0.3, 1.5),
        hat_scale=str(rng.choice(list(HAT_SCALES))), hat_squeak=rng.uniform(0.0, 0.6), hat_q=rng.uniform(6, 30),
        hat_len=rng.uniform(0.7, 1.5), ratchet=rng.uniform(0.0, 0.3), mouth=rng.uniform(0.1, 0.6),
        b_oct=float(rng.choice([0.0, 0.0, 0.0, 1.0])), b_wave=rng.uniform(0.3, 0.9), b_sync=rng.uniform(0.0, 3.5),
        b_drive=rng.uniform(0.8, 2.6), b_lo=rng.uniform(0.7, 1.8), b_span=rng.uniform(2.0, 4.0), b_brt=rng.uniform(0.3, 1.8),
        b_fm=rng.uniform(0.0, 0.6), b_fm_ratio=float(rng.choice([1.0, 1.0, 2.0])), b_q=rng.uniform(5, 14), b_hp=rng.uniform(60, 110), b_gate=rng.uniform(1.0, 2.0),
    )
    return d


PALETTE = sketch.Palette(
    name="filterscream",
    blurb="Squelching acid-bass and tuned filter-ping drums: thumping pitch-dive kicks, singing noise snares, metallic squeaks for hats.",
    hits={"kick": kick, "snare": snare, "hat": hat},
    phrase=bass,
    params=params,
    levels={"kick": 1.0, "snare": 0.8, "hat": 0.5, "phrase": 0.7},
)
