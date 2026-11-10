"""fmmetal: one alien FM instrument. Every role is a phase-modulation stack that
shares the same inharmonic modulator family (sqrt2, 2.76, 3.5), so kick, snare,
hat and bass read as one object. Index follows brightness and velocity."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import numpy as np  # noqa: E402
from scipy import signal  # noqa: E402

import sketch  # noqa: E402

TAU = 2 * np.pi
tx, env_ad, phase_of, norm, filt = sketch.t_axis, sketch.env_ad, sketch.phase_of, sketch.norm, sketch.filt


def pm(ph, ratio, index, fb=0.0):
    """Two-operator phase modulation; the modulator feeds back on itself."""
    m = np.sin(ratio * ph + fb * np.sin(ratio * ph))
    return np.sin(ph + index * m)


def fade(y, sr, ms):
    """Short raised-cosine fade on the tail so a voice cut off at 4-9 percent of its peak does not tick."""
    k = min(len(y), max(2, int(ms * 1e-3 * sr)))
    y[-k:] *= 0.5 * (1 + np.cos(np.pi * np.arange(k) / k))
    return y


def kick(h, p, rng, sr):
    n = int((0.16 + 0.12 * p["len"] + 0.1 * min(h.dur, 0.4)) * sr)
    t = tx(n, sr)
    hi = p["root"] * 2 ** (p["octs"] * (0.6 + 0.5 * h.vel) + 0.5 * h.bright)
    f = p["root"] + (hi - p["root"]) * np.exp(-t / (0.016 + 0.012 * (1 - h.vel)))
    ph = phase_of(f, sr)
    idx = (0.3 + 4.0 * h.bright ** 1.2 + 1.5 * h.vel) * p["kidx"]
    body = pm(ph, p["r1"], idx * np.exp(-t / 0.014) + 0.15, fb=1.2)
    body *= env_ad(n, sr, 0.0004, 0.085 * p["len"] + 0.04 * min(h.dur, 0.4))
    body = np.tanh(body * (1.4 + 1.8 * h.vel))
    nc = int(0.012 * sr)
    tc = tx(nc, sr)
    fc = 2200 * 2 ** (0.8 * h.bright) * (1 + 0.5 * np.exp(-tc / 0.002))
    click = pm(phase_of(fc, sr), p["r2"], 5 * np.exp(-tc / 0.003), fb=1.0) * np.exp(-tc / 0.0018)
    y = body
    y[:nc] += (0.15 + 0.9 * h.bright) * click
    y = filt(y, "hp", 28, 0.7, sr)
    return fade(y / 1.1, sr, 12)


def snare(h, p, rng, sr):
    n = int((0.13 + 0.12 * min(h.dur, 0.3) + 0.06 * p["len"]) * sr)
    t = tx(n, sr)
    f = 175 * p["sn"] + 420 * np.exp(-t / 0.012) * (0.6 + 0.6 * h.vel)
    ph = phase_of(f, sr)
    idx = (0.8 + 6.0 * h.bright + 1.5 * h.vel) * p["sidx"]
    tone = pm(ph, p["r2"], idx * np.exp(-t / 0.03), fb=1.5) * env_ad(n, sr, 0.0003, 0.05)
    # metallic noise: a high carrier hit by a big inharmonic index
    fh = 1800 * 2 ** (1.8 * h.bright) * p["sn"]
    hiph = phase_of(np.full(n, fh), sr)
    zz = pm(hiph, p["r3"], (6 + 6 * h.bright) * np.exp(-t / 0.05), fb=2.0)
    zz = zz + 0.5 * filt(rng.standard_normal(n), "hp", 3000, 0.7, sr)
    zz = filt(zz, "hp", 1800, 0.7, sr) * env_ad(n, sr, 0.0003, 0.035 + 0.05 * p["len"] + 0.1 * min(h.dur, 0.3))
    y = 0.8 * tone + 0.75 * norm(zz)
    y = np.tanh(y * (1.5 + 1.5 * h.vel))
    gate = np.where(t < 0.7 * n / sr, 1.0, np.exp(-(t - 0.7 * n / sr) / 0.006))
    return y * gate


def hat(h, p, rng, sr):
    is_open = h.dur > 0.2
    L = (0.14 + 0.1 * min(h.dur, 0.4)) if is_open else (0.022 + 0.03 * min(h.dur, 0.12))
    n = int(L * sr)
    t = tx(n, sr)
    base = p["hat"] * 2 ** (1.0 * h.bright - 0.3)
    ph = phase_of(np.full(n, base), sr)
    idx = (1.5 + 8 * h.bright + 2 * h.vel) * np.exp(-t / (L / 2.5)) + 1.0
    a = pm(ph, p["r3"], idx, fb=1.5)
    b = pm(ph * p["r1"] * 1.5, p["r2"], idx * 0.8, fb=1.5)
    y = filt(a + 0.8 * b, "hp", 5000, 0.8, sr)
    y = filt(y, "lp", 6500 * 2 ** (1.5 * h.bright), 0.8, sr)  # brightness opens the top
    out = norm(y) * env_ad(n, sr, 0.0002, L / 3.2) * 0.9 * 10 ** ((h.bright - 0.5) * 18 / 20)  # brighter ticks carry more HF energy
    return fade(out, sr, 3)


def phrase(c, p, rng, sr):
    f = c.f * p["boct"]
    ph = phase_of(f, sr)
    idx = p["bi0"] + p["bi_amp"] * c.amp + p["bi_brt"] * c.brt + p["bi_acc"] * c.acc
    car = pm(ph, p["br"], idx, fb=0.9)
    car2 = np.sin(ph + 0.8 * idx * np.sin(p["r1"] * ph + 0.5))  # inharmonic partner, shared with the kit
    x = np.tanh(1.5 * (car + 0.35 * car2))
    x = filt(x, "hp", 60, 0.7, sr)
    duck = 1 - 0.65 * c.acc ** 1.5  # make room for the kick's snap
    return x * duck * c.amp ** 0.8 * (0.3 + 0.7 * c.acc ** 0.5)


def params(rng):
    d = dict(root=46.0, octs=3.2, len=1.0, kidx=1.0, r1=1.414, r2=2.76, r3=3.5, sn=1.0, sidx=1.0,
             hat=3300.0, br=1.0, boct=2.0, bi0=0.5, bi_amp=1.0, bi_brt=1.2, bi_acc=1.8)
    if rng is None:
        return d
    d.update(root=rng.uniform(42, 56), octs=rng.uniform(2.6, 4.0), len=rng.uniform(0.7, 1.3), kidx=rng.uniform(0.6, 1.5),
             r1=float(rng.choice([1.414, 1.41, 1.73, 2.0])), r2=float(rng.choice([2.76, 2.41, 3.0, 3.73])),
             r3=float(rng.choice([3.5, 4.07, 5.4])), sn=rng.uniform(0.85, 1.4), sidx=rng.uniform(0.7, 1.5),
             hat=rng.uniform(2600, 4200), br=float(rng.choice([1.0, 1.0, 2.0, 0.5])), boct=float(rng.choice([2.0, 2.0, 1.0])),
             bi0=rng.uniform(0.4, 1.4), bi_amp=rng.uniform(0.8, 2.4), bi_brt=rng.uniform(1.0, 3.0), bi_acc=rng.uniform(1.5, 3.5))
    return d


PALETTE = sketch.Palette(
    name="fmmetal",
    blurb="Latex-snap kicks, zippy metallic snares and glassy FM ticks with a growling bass, all one alien metal instrument.",
    hits={"kick": kick, "snare": snare, "hat": hat}, phrase=phrase, params=params,
    levels={"kick": 1.0, "snare": 0.8, "hat": 0.9, "phrase": 0.45})
