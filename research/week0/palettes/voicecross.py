"""voicecross: the take as raw material, made into a machine.

Every voice is a synthetic carrier (sine sweep, saw, pulse, metal squares, noise)
whose spectral envelope is taken from the performer's own sound: a short-time
cross-synthesis (an STFT channel vocoder). Hits use their own mouth transient;
the bass is a saw whose vowel follows the hummed stretch of the take.
"""
from __future__ import annotations

import dataclasses
import sys
import zlib
from pathlib import Path

import numpy as np
from scipy import signal
from scipy.ndimage import uniform_filter1d

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import loop as loopmod  # noqa: E402
import sketch  # noqa: E402

TAU = 2 * np.pi
SR = 96000
HUM = (113.0, 143.0)


# ------------------------------------------------------------ helpers


def tax(n):
    return np.arange(n) / SR


def env(n, att, tau):
    t = tax(n)
    return np.minimum(1.0, t / att) * np.exp(-t / tau)


def peak(x, v=1.0):
    return x * (v / (np.abs(x).max() + 1e-9))


def fit(x, n):
    return x[:n] if len(x) >= n else np.concatenate([x, np.zeros(n - len(x))])


def hardclip(x, g, lim=0.8):
    return np.clip(x * g, -lim, lim) / lim


def lp(x, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc, "lp", fs=SR, output="sos"), x)


def hp(x, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc, "hp", fs=SR, output="sos"), x)


def saw(f):
    ph = np.cumsum(f) / SR
    return 2 * (ph % 1.0) - 1


def pulse_train(f, width=0.12):
    ph = (np.cumsum(f) / SR) % 1.0
    return np.where(ph < width, 1.0, -1.0) - (1 - 2 * width)


def cross(carrier, mouth, nfft=512, hop=64, width=7, gamma=1.0, floor=0.02):
    """Impose the mouth's smoothed spectral envelope (over time) on a carrier."""
    n = len(carrier)
    mouth = fit(mouth, n)
    kw = dict(nperseg=nfft, noverlap=nfft - hop, boundary="zeros", padded=True)
    _, _, C = signal.stft(carrier, **kw)
    _, _, M = signal.stft(mouth, **kw)
    em = uniform_filter1d(np.abs(M), width, axis=0)
    em = (em / (em.max() + 1e-12)) ** gamma + floor
    ec = uniform_filter1d(np.abs(C), width, axis=0) + 1e-9
    _, y = signal.istft(C / ec * em, nperseg=nfft, noverlap=nfft - hop, boundary=True)
    return fit(y, n)


def up_mouth(m):
    return signal.resample_poly(m, 2, 1) if len(m) > 8 else np.zeros(32)


def metal(f0, n, rng):
    t = tax(n)
    ratios = (1.0, 1.342, 1.2312, 1.6532, 1.9523, 2.1523)
    x = sum(np.sign(np.sin(TAU * f0 * r * t + rng.uniform(0, TAU))) for r in ratios)
    return x / 6


# ------------------------------------------------------------ voices (return signal, pre-roll samples)


def v_kick(h, p, rng):
    L = float(np.clip(0.17 + 0.5 * h.dur, 0.17, 0.34)) * p["klen"]
    n = int(L * SR)
    t = tax(n)
    hi = p["kroot"] * 2 ** (p["koct"] * (0.65 + 0.55 * h.bright))
    f = p["kroot"] + (hi - p["kroot"]) * np.exp(-t / (p["ktau"] * (1.3 - 0.5 * h.vel)))
    f = f * (1 + p["kbend"] * np.exp(-((t - 0.07) / 0.03) ** 2))  # small rubber rebound
    body = np.sin(TAU * np.cumsum(f) / SR)
    body = hardclip(body, 1.5 + 4 * h.vel, 0.85) * env(n, 0.0004, L / 3.6)
    body = lp(body, 900, 2)
    # machine click: saw through the mouth's own envelope, an octave lower with 2x slowed copy
    m = up_mouth(h.mouth)
    nc = int(0.03 * SR)
    car = saw(np.full(nc, 140.0 * 2 ** (1.5 * h.bright))) + 0.4 * rng.standard_normal(nc)
    click = cross(car, m[:nc], 256, 32, 5, 1.2) * env(nc, 0.0002, 0.006)
    click = hp(hardclip(click, 4.0), 400)
    slow = lp(signal.resample_poly(m[:nc], 2, 1), 500)  # the mouth thump an octave down
    slow = fit(slow, n) * env(n, 0.0006, 0.03)
    y = body + p["kclick"] * (0.35 + 0.65 * h.bright) * fit(click, n) + 0.5 * peak(slow, 0.4)
    return hardclip(y, 1.6 + 1.0 * h.vel, 0.95), 0


def v_snare(h, p, rng):
    L = float(np.clip(0.11 + 0.45 * h.dur, 0.11, 0.24))
    n = int(L * SR)
    t = tax(n)
    m = up_mouth(h.mouth)
    f = 175 + (p["sfreq"] * (0.7 + 0.6 * h.bright) - 175) * np.exp(-t / 0.012)
    ph = TAU * np.cumsum(f) / SR
    body = np.sin(ph + p["sidx"] * (0.5 + h.vel) * np.exp(-t / 0.02) * np.sin(p["sratio"] * ph)) * env(n, 0.0003, 0.035)
    car = 0.7 * rng.standard_normal(n) + 0.8 * pulse_train(np.full(n, p["spulse"])) + 0.6 * saw(f * 2)
    rasp = cross(car, m, 384, 48, 6, p["sgamma"], 0.04)
    rasp = peak(rasp * env(n, 0.0003, 0.02 + 0.06 * h.dur + 0.02 * h.bright))
    rm = np.sin(TAU * p["srm"] * (0.8 + 0.5 * h.bright) * t)
    ring = rasp * (1 - p["srmmix"] + p["srmmix"] * rm)
    y = 0.65 * body + 0.9 * ring
    y = hp(hardclip(y, 2.2 + 5 * h.vel, 0.85), 120)
    gate = np.where(t < L * 0.8, 1.0, np.exp(-(t - L * 0.8) / 0.003))
    y = y * gate
    pre = 0
    if h.vel > 0.55 and p["swell"] > 0:  # reversed mouth swell into the hit, a machine intake
        npre = int(0.07 * SR)
        noise = rng.standard_normal(npre)
        sw = cross(noise, m[:npre][::-1], 256, 32, 6, 1.0)
        sw = hp(sw, 1500) * np.linspace(0, 1, npre) ** 2.5 * p["swell"] * 0.45
        y = np.concatenate([sw, y])
        pre = npre
    return y, pre


def v_hat(h, p, rng):
    is_open = h.dur > 0.2
    L = float(np.clip(0.08 + 0.25 * h.dur, 0.08, 0.17)) if is_open else float(np.clip(0.02 + 0.12 * h.dur, 0.025, 0.055))
    n = int(L * SR)
    t = tax(n)
    m = up_mouth(h.mouth)
    base = p["hbase"] * 2 ** (0.6 * h.bright)
    car = metal(base, n, rng) * np.sin(TAU * p["hrm"] * t + 0.5) + 0.5 * rng.standard_normal(n)
    v = cross(car, m, 256, 32, 5, p["hgamma"], 0.03)
    v = hp(v, 3500 * 2 ** (0.7 * h.bright), 2)
    v = hardclip(peak(v), 2.0 + 3 * h.vel, 0.8) * env(n, 0.0002, L / 3.0)
    gate = np.where(t < L * 0.85, 1.0, np.exp(-(t - L * 0.85) / 0.0015))
    return v * gate, 0


# ------------------------------------------------------------ bass


def hum_frames(take, sr):
    a, b = int(HUM[0] * sr), int(min(HUM[1] * sr, len(take)))
    if b - a < sr:
        a, b = 0, len(take)
    x = take[a:b]
    _, _, Z = signal.stft(x, nperseg=1024, noverlap=512, boundary=None, padded=False)
    M = np.abs(Z)
    M = uniform_filter1d(M, 9, axis=0)
    e = np.log(M.sum(0) + 1e-9)
    keep = e > np.percentile(e, 55)
    F = M[:, keep]
    F = F / (F.max(0, keepdims=True) + 1e-12)
    return F  # 513 x frames


def bass_voice(loop, p, rng):
    sr_s = SR
    c = loopmod.bass_controls(loop, sr_s)
    n = loop.n * 2
    nfft, hop = 2048, 512
    car = saw(c.f * 2.0 ** p["boct"])
    car = car + p["bdet"] * saw(c.f * 2.0 ** p["boct"] * 1.006)
    kw = dict(nperseg=nfft, noverlap=nfft - hop, boundary="zeros", padded=True)
    _, _, C = signal.stft(car, **kw)
    F = hum_frames(loop.take, loop.sr)
    nfr = C.shape[1]
    pos = np.linspace(0, F.shape[1] - 1, nfr) * p["bscan"] % (F.shape[1] - 1)
    E = np.empty((C.shape[0], nfr))
    lo = np.floor(pos).astype(int)
    fr = pos - lo
    E[:513] = F[:, lo] * (1 - fr) + F[:, np.minimum(lo + 1, F.shape[1] - 1)] * fr
    E[513:] = 0.0
    E = uniform_filter1d(E, 3, axis=1)
    cf = np.clip(np.arange(nfr) * hop, 0, n - 1)
    acc = c.acc[cf] * 0.0 + c.acc[cf]
    freqs = np.arange(C.shape[0]) * SR / nfft
    tilt = (np.maximum(freqs, 200) / 400.0)[:, None] ** (p["btilt"] * (0.3 + c.brt[cf] + 1.5 * acc))[None, :]
    E = (E ** p["bgamma"] + 0.01) * np.minimum(tilt, 6.0)
    # leave the sub to the sine, keep the vowel above ~140 Hz
    E *= np.clip((freqs - 90) / 120, 0, 1)[:, None]
    _, v = signal.istft(C * E, nperseg=nfft, noverlap=nfft - hop, boundary=True)
    v = fit(v, n)
    v = peak(v)
    sub = np.sin(TAU * np.cumsum(c.f * 2.0 ** p["boct"]) / SR)
    return v, sub, c


def bass_mix(loop, p, rng):
    v, sub, c = bass_voice(loop, p, rng)
    n = len(v)
    duck = np.ones(n)
    for h in loop.hits:
        if h.role != "kick":
            continue
        a = h.sample * 2
        L = int(0.16 * SR)
        if a >= n:
            continue
        seg = min(L, n - a)
        t = np.arange(seg) / SR
        d = 0.9 * np.exp(-t / 0.075)
        duck[a : a + seg] = np.minimum(duck[a : a + seg], 1 - d)
    vduck = 0.5 + 0.5 * duck
    y = p["bbuzz"] * hardclip(v, 2.2 + 2 * p["bdrive"], 0.9) * vduck + p["bsub"] * sub * duck
    y = hardclip(y, 1.0 + p["bdrive"], 0.9) * (c.amp ** 1.7)
    return lp(y, 4500, 2)


# ------------------------------------------------------------ whole loop


def render_loop(loop, p, roll):
    n_s = loop.n * 2
    out = np.zeros(n_s)
    hits = list(loop.hits)
    if p["ratchet"] > 0:
        rr = np.random.default_rng([roll, 77])
        six = loop.sixteenth * loop.sr
        extra = []
        for h in hits:
            if h.role == "hat" and rr.random() < p["ratchet"]:
                extra.append(dataclasses.replace(h, sample=h.sample + int(0.5 * six), vel=h.vel * 0.7, src=h.src + 1))
        hits += extra
    for ri, role in enumerate(("kick", "snare", "hat")):
        group = sorted((h for h in hits if h.role == role), key=lambda h: h.sample)
        fade = int(0.002 * SR)
        for i, h in enumerate(group):
            hh = sketch.Hit(n0=h.sample * 2, role=role, vel=h.vel, bright=h.bright, dur=h.dur, mouth=h.mouth, pre=0)
            hh = dataclasses.replace(hh)
            hh.mouth = h.mouth
            rng = np.random.default_rng([roll, ri, h.src, h.sample])
            fn = {"kick": v_kick, "snare": v_snare, "hat": v_hat}[role]
            y, pre = fn(hh, p, rng)
            n0 = h.sample * 2
            if i + 1 < len(group):
                room = group[i + 1].sample * 2 - n0 + fade + (pre if False else 0)
                if room < len(y) - pre:
                    y = y[: room + pre].copy()
                    y[-fade:] *= np.cos(np.linspace(0, np.pi / 2, fade)) ** 2
            s0 = n0 - pre
            if s0 < 0:
                y = y[-s0:]
                s0 = 0
            y = y[: n_s - s0]
            out[s0 : s0 + len(y)] += y * p["lv_" + role] * 10 ** ((h.vel - 1) * p["vrange"] / 20)
    if loop.bass:
        out += p["lv_bass"] * bass_mix(loop, p, None)
    return signal.resample_poly(out, 1, 2)


def params(rng):
    d = dict(kroot=52.0, koct=2.0, ktau=0.022, kbend=0.06, klen=1.0, kclick=0.45,
             sfreq=1300.0, sidx=2.5, sratio=2.41, spulse=140.0, sgamma=1.0, srm=1700.0, srmmix=0.5, swell=1.0,
             hbase=520.0, hrm=5300.0, hgamma=1.0,
             boct=0.0, bdet=0.5, bscan=1.0, btilt=0.6, bgamma=1.2, bbuzz=0.6, bsub=0.9, bdrive=2.0,
             ratchet=0.0, vrange=18.0, lv_kick=1.0, lv_snare=1.1, lv_hat=0.7, lv_bass=0.3)
    if rng is None:
        return d
    d.update(kroot=rng.uniform(44, 62), koct=rng.uniform(1.4, 2.8), ktau=rng.uniform(0.014, 0.034),
             kbend=rng.uniform(0.0, 0.14), kclick=rng.uniform(0.25, 0.7),
             sfreq=rng.uniform(900, 2200), sidx=rng.uniform(1.0, 5.0),
             sratio=float(rng.choice([1.41, 2.41, 3.5, 5.33])), spulse=rng.uniform(95, 260),
             sgamma=rng.uniform(0.8, 1.5), srm=rng.uniform(900, 3200), srmmix=rng.uniform(0.2, 0.8),
             swell=float(rng.choice([0.0, 1.0, 1.0])), hbase=rng.uniform(380, 800), hrm=rng.uniform(3200, 8500),
             hgamma=rng.uniform(0.8, 1.6), bdet=rng.uniform(0.2, 0.9), bscan=float(rng.choice([1.0, 2.0, 3.0])),
             btilt=rng.uniform(0.2, 1.0), bgamma=rng.uniform(0.9, 1.8), bbuzz=rng.uniform(0.5, 1.1),
             bdrive=rng.uniform(1.0, 3.5), ratchet=rng.uniform(0.1, 0.3))
    return d


PALETTE = sketch.Palette(
    name="voicecross",
    blurb="Machine drums that talk: robotic, vocoded snaps and rasps shaped by your own mouth, a clipped sub kick, and a buzzing bass that mumbles vowels from your hum.",
    hits={"kick": lambda h, p, r, sr: v_kick(h, p, r)[0], "snare": lambda h, p, r, sr: v_snare(h, p, r)[0],
          "hat": lambda h, p, r, sr: v_hat(h, p, r)[0]},
    phrase=lambda c, p, r, sr: np.zeros(len(c.f)),
    params=params,
    levels={"kick": 1.0, "snare": 0.7, "hat": 0.32, "phrase": 0.45},
)
PALETTE.render_loop = render_loop
