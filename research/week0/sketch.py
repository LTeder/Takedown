"""Week 0 sound sketch: the voices and palettes `render.py` plays a take through.

Throwaway numpy synthesis, written to find out whether sounds designed here,
driven by a mouth performance, are exciting, before the Rust engine exists.
It follows the engine's rules where they are cheap to follow: every hit is
synthesised from its own features and its own seed (so one hit never changes
another), mangle (drive, fold, crush, ring-mod, filter scream, pitch
envelopes) is per voice, and there are no bus effects.

Three palettes, each a kick, snare and hat voice plus a phrase voice that
follows the pitch track:

rubber   Clean and elastic: sine kicks with long pitch drops and a rebound,
         FM snaps, metal ticks and upward "bubble" blips, an FM bass that
         bends with the hum.
crushed  Blown out: clipped and folded kicks, ring-modulated trash snares,
         aliased hats, a folded saw bass through a resonant "scream"
         filter, with the mouth's own transient mangled and layered in.
struck   The mouth is the stick: each hit's own transient excites tuned
         membrane, comb and glass resonators; the hum plays a stiff-string
         partial bank.

Each palette's parameters have a designed default (roll 1) and seeded
variations (rolls 2, 3, ...): the dice. Roll 1 plays the rhythm as
performed; later rolls may also turn some hits into short ratchets.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy import signal

TAU = 2 * np.pi


# ------------------------------------------------------------------ blocks


def t_axis(n: int, sr: int) -> np.ndarray:
    return np.arange(n) / sr


def env_ad(n: int, sr: int, attack: float, tau: float) -> np.ndarray:
    t = t_axis(n, sr)
    return np.minimum(1.0, t / max(attack, 1e-6)) * np.exp(-t / tau)


def sweep(n: int, sr: int, f_hi: float, f_lo: float, tau: float) -> np.ndarray:
    return f_lo + (f_hi - f_lo) * np.exp(-t_axis(n, sr) / tau)


def phase_of(f: np.ndarray, sr: int) -> np.ndarray:
    return TAU * np.cumsum(f) / sr


def norm(x: np.ndarray) -> np.ndarray:
    return x / (np.abs(x).max() + 1e-9)


def sinefold(x: np.ndarray, g) -> np.ndarray:
    return np.sin(0.5 * np.pi * g * x)


def crush(x: np.ndarray, bits: float) -> np.ndarray:
    q = 2.0 ** (bits - 1)
    return np.round(x * q) / q


def hold(x: np.ndarray, k: int) -> np.ndarray:
    """Sample-rate reduction with no anti-alias filter."""
    k = max(1, int(k))
    return x[(np.arange(len(x)) // k) * k]


def biquad(kind: str, fc: float, q: float, sr: int) -> tuple[np.ndarray, np.ndarray]:
    w = TAU * min(max(fc, 10.0), 0.45 * sr) / sr
    al, c = np.sin(w) / (2 * q), np.cos(w)
    if kind == "lp":
        b = np.array([(1 - c) / 2, 1 - c, (1 - c) / 2])
    elif kind == "hp":
        b = np.array([(1 + c) / 2, -(1 + c), (1 + c) / 2])
    else:  # band-pass, 0 dB at the peak
        b = np.array([al, 0.0, -al])
    a = np.array([1 + al, -2 * c, 1 - al])
    return b / a[0], a / a[0]


def filt(x: np.ndarray, kind: str, fc: float, q: float, sr: int) -> np.ndarray:
    b, a = biquad(kind, fc, q, sr)
    return signal.lfilter(b, a, x)


def tv_bandpass(x: np.ndarray, fc: np.ndarray, q: float, sr: int, block: int = 64) -> np.ndarray:
    """Band-pass whose centre follows `fc`, coefficients updated every block."""
    y = np.empty_like(x)
    zi = np.zeros(2)
    for i in range(0, len(x), block):
        b, a = biquad("bp", float(fc[i]), q, sr)
        y[i : i + block], zi = signal.lfilter(b, a, x[i : i + block], zi=zi)
    return y


def modes(x: np.ndarray, freqs, t60s, gains, sr: int) -> np.ndarray:
    """A bank of resonators: each rings at its frequency for about its t60."""
    y = np.zeros(len(x))
    for f, t60, g in zip(freqs, t60s, gains):
        if f < 0.45 * sr:
            y += g * filt(x, "bp", f, max(0.5, np.pi * f * t60 / 6.908), sr)
    return y


def comb(x: np.ndarray, f: float, t60: float, sr: int, damp: float = 0.3) -> np.ndarray:
    """Feedback comb with a one-zero low-pass in the loop (a plucked pipe)."""
    d = max(2, int(round(sr / f)))
    g = 10 ** (-3 * d / (t60 * sr))
    a = np.zeros(d + 2)
    a[0], a[d], a[d + 1] = 1.0, -g * (1 - damp), -g * damp
    return signal.lfilter([1.0], a, x)


def pad_to(x: np.ndarray, n: int) -> np.ndarray:
    return x[:n] if len(x) >= n else np.concatenate([x, np.zeros(n - len(x))])


# ------------------------------------------------------------------ events


@dataclass
class Hit:
    """One drum hit. `vel`, `bright` are 0..1; `mouth` is the take's own
    transient at the synthesis rate, peak 1, starting `pre` samples early."""

    n0: int
    role: str
    vel: float
    bright: float
    dur: float
    mouth: np.ndarray
    pre: int


@dataclass
class Controls:
    """Audio-rate curves for one stretch of the phrase voice."""

    f: np.ndarray  # Hz
    amp: np.ndarray  # 0..1, from loudness
    brt: np.ndarray  # 0..1, from the harmonic centroid
    acc: np.ndarray  # 0..1 accents at note onsets, decaying
    mouth: np.ndarray  # the take itself over this stretch


# ------------------------------------------------------------------ rubber


def rubber_kick(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    length = float(np.clip(0.28 + 1.2 * h.dur, 0.28, 0.9)) * p["kick_len"]
    n = int(length * sr)
    t = t_axis(n, sr)
    f_hi = p["kick_root"] * 2 ** (p["kick_octs"] * (0.55 + 0.75 * h.bright))
    f = sweep(n, sr, f_hi, p["kick_root"], p["kick_tau"] * (1.4 - 0.7 * h.vel))
    f = f * 2 ** (p["kick_wob"] * np.exp(-t / 0.16) * np.sin(TAU * p["kick_wob_hz"] * t))
    y = np.tanh(np.sin(phase_of(f, sr)) * (1.5 + 7 * h.vel)) * env_ad(n, sr, 0.0006, length / 3.2)
    nc = int(0.004 * sr)
    click = filt(rng.standard_normal(nc), "hp", 2500, 0.7, sr) * np.exp(-t_axis(nc, sr) / 0.0012)
    y[:nc] += (0.25 + 0.5 * h.bright) * norm(click)
    return y


def rubber_snare(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    length = float(np.clip(0.14 + 0.9 * h.dur, 0.14, 0.55))
    n = int(length * sr)
    t = t_axis(n, sr)
    f = sweep(n, sr, p["sn_hi"] * (0.6 + 0.8 * h.bright), p["sn_lo"], 0.008)
    ph = phase_of(f, sr)
    zap = np.sin(ph + p["sn_index"] * (0.5 + h.vel) * np.exp(-t / 0.018) * np.sin(p["sn_ratio"] * ph))
    nz = norm(filt(rng.standard_normal(n), "bp", p["sn_noise_hz"] * 2 ** (1.4 * h.bright - 0.5), 1.4, sr))
    y = 0.9 * zap * env_ad(n, sr, 0.0005, 0.03 + 0.1 * h.dur) + 0.8 * nz * env_ad(n, sr, 0.001, 0.04 + 0.3 * h.dur)
    if p["sn_boing"] > 0:  # a tone that bends upward after the snap
        fb = p["sn_lo"] * 2 ** (p["sn_boing"] * (1 - np.exp(-t / 0.05)))
        y += 0.35 * np.sin(phase_of(fb, sr)) * env_ad(n, sr, 0.004, 0.07)
    return np.tanh(2.2 * y)


METAL = (1.0, 1.342, 1.2312, 1.6532, 1.9523, 2.1523)


def rubber_hat(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    is_open = h.dur > 0.18
    length = float(np.clip(0.25 + h.dur, 0.25, 0.7)) if is_open else float(np.clip(0.04 + 0.25 * h.dur, 0.04, 0.11))
    n = int(length * sr)
    t = t_axis(n, sr)
    base = p["hat_base"] * 2 ** (0.5 * h.bright)
    metal = sum(np.sign(np.sin(TAU * base * r * t + rng.uniform(0, TAU))) for r in METAL)
    metal = norm(filt(metal, "hp", 6500, 0.8, sr)) * env_ad(n, sr, 0.0004, length / 3.5)
    nb = min(n, int(p["bub_len"] * sr))
    tb = t_axis(nb, sr)
    f0 = p["bub_hz"] * 2 ** (1.5 * h.bright)
    fb = f0 * 2 ** (p["bub_octs"] * (tb / tb[-1]) ** 0.7)
    bubble = np.sin(phase_of(fb, sr)) * np.sin(np.pi * tb / tb[-1]) ** 2
    y = (1 - p["bub_mix"]) * metal
    y[:nb] += p["bub_mix"] * 1.3 * bubble
    return y


def rubber_phrase(c: Controls, p: dict, rng, sr: int) -> np.ndarray:
    ph = phase_of(c.f * 2.0 ** p["ph_oct"], sr)
    index = p["ph_i0"] + p["ph_i_amp"] * c.amp + p["ph_i_brt"] * c.brt + p["ph_i_acc"] * c.acc
    x = np.sin(ph + index * np.sin(p["ph_ratio"] * ph))
    return np.tanh(1.6 * (x + p["ph_sub"] * np.sin(0.5 * ph))) * c.amp**0.8


def rubber_params(rng) -> dict:
    d = dict(kick_root=49.0, kick_octs=4.3, kick_tau=0.028, kick_wob=0.22, kick_wob_hz=11.0, kick_len=1.0,
             sn_hi=3600.0, sn_lo=190.0, sn_index=3.0, sn_ratio=2.41, sn_noise_hz=2600.0, sn_boing=0.9,
             hat_base=540.0, bub_hz=900.0, bub_octs=1.6, bub_len=0.035, bub_mix=0.55,
             ph_oct=-1.0, ph_ratio=1.0, ph_i0=0.3, ph_i_amp=2.4, ph_i_brt=2.0, ph_i_acc=4.0, ph_sub=0.45, ratchet=0.0)
    if rng is None:
        return d
    d.update(kick_root=rng.uniform(41, 62), kick_octs=rng.uniform(3.0, 5.6), kick_tau=rng.uniform(0.014, 0.05),
             kick_wob=rng.uniform(0.0, 0.45), kick_wob_hz=rng.uniform(6, 17), kick_len=rng.uniform(0.7, 1.3),
             sn_hi=rng.uniform(2200, 6000), sn_lo=rng.uniform(150, 320), sn_index=rng.uniform(1.0, 6.0),
             sn_ratio=float(rng.choice([1.41, 2.41, 2.76, 3.5, 5.33])), sn_noise_hz=rng.uniform(1800, 4200),
             sn_boing=float(rng.choice([0.0, 0.6, 1.2, -0.7])), hat_base=rng.uniform(380, 760),
             bub_hz=rng.uniform(500, 1600), bub_octs=rng.uniform(0.8, 2.4), bub_len=rng.uniform(0.02, 0.07),
             bub_mix=rng.uniform(0.2, 0.85), ph_oct=float(rng.choice([-1.0, -1.0, 0.0])),
             ph_ratio=float(rng.choice([0.5, 1.0, 2.0, 3.01])), ph_i_amp=rng.uniform(1.2, 4.0),
             ph_i_brt=rng.uniform(1.0, 3.5), ph_i_acc=rng.uniform(2.0, 7.0), ph_sub=rng.uniform(0.2, 0.7),
             ratchet=rng.uniform(0.0, 0.15))
    return d


# ----------------------------------------------------------------- crushed


def crushed_kick(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    length = float(np.clip(0.35 + 1.5 * h.dur, 0.35, 1.1))
    n = int(length * sr)
    f = sweep(n, sr, p["kick_hi"] * (0.7 + 0.6 * h.bright), p["kick_root"], 0.045)
    x = np.clip(np.sin(phase_of(f, sr)) * (3 + 10 * h.vel), -1, 1)
    x = sinefold(x, 1 + p["kick_fold"] * h.vel)
    x = filt(x, "lp", 3200, 0.7, sr) * env_ad(n, sr, 0.0005, length / 3.0)
    m = np.tanh(8 * pad_to(h.mouth, n)) * env_ad(n, sr, 0.0005, 0.05)
    y = norm(x + p["mouth"] * m)
    return crush(hold(y, p["hold"]), p["bits"])


def crushed_snare(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    length = float(np.clip(0.16 + 0.9 * h.dur, 0.16, 0.5))
    n = int(length * sr)
    t = t_axis(n, sr)
    src = rng.standard_normal(n) * env_ad(n, sr, 0.0005, 0.05 + 0.25 * h.dur)
    src += 1.5 * np.sin(TAU * 185 * t) * env_ad(n, sr, 0.0005, 0.06)
    src += p["mouth"] * 3 * np.tanh(6 * pad_to(h.mouth, n))
    rm = np.sin(TAU * p["rm_hz"] * (0.7 + 0.6 * h.bright) * t)
    x = src * (1 - p["rm_mix"] + p["rm_mix"] * rm)
    x = x + p["trash"] * comb(x, p["comb_hz"], 0.22, sr, 0.25) * 0.3
    y = np.clip(norm(x) * (2 + 6 * h.vel), -1, 1)
    gate = np.where(t < length * 0.75, 1.0, np.exp(-(t - length * 0.75) / 0.004))  # a hard, gated tail
    return crush(hold(y * gate, p["hold"]), p["bits"])


def crushed_hat(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    is_open = h.dur > 0.18
    length = float(np.clip(0.2 + h.dur, 0.2, 0.6)) if is_open else float(np.clip(0.03 + 0.2 * h.dur, 0.03, 0.09))
    n = int(length * sr)
    t = t_axis(n, sr)
    x = filt(rng.standard_normal(n), "hp", 5000, 0.8, sr) * np.sin(TAU * p["hat_rm_hz"] * t)
    x = hold(x, p["hat_hold"] + int(10 * (1 - h.bright)))  # darker mouth sound, lower fold-over
    x = filt(x, "hp", 1800, 0.7, sr)  # keep the fold-over fizz, lose the mud it drops under the kick
    return norm(x) * env_ad(n, sr, 0.0003, length / 3.0)


def crushed_phrase(c: Controls, p: dict, rng, sr: int) -> np.ndarray:
    ph = phase_of(c.f * 2.0 ** p["ph_oct"], sr)
    x = sum(2 * ((ph * 2 ** (d / 1200) / TAU) % 1.0) - 1 for d in (-p["ph_detune"], 0.0, p["ph_detune"])) / 3
    x = sinefold(x, 1 + p["ph_fold"] * (c.amp + c.acc))
    fc = p["scream_lo"] * 2 ** (p["scream_octs"] * (0.6 * c.brt + 0.4 * c.amp) + c.acc)
    y = np.clip(2.5 * (0.5 * x + 1.2 * tv_bandpass(x, fc, p["scream_q"], sr)), -1, 1)
    return crush(hold(y, p["ph_hold"]), p["ph_bits"]) * c.amp**0.7


def crushed_params(rng) -> dict:
    d = dict(kick_root=44.0, kick_hi=190.0, kick_fold=2.0, mouth=0.35, bits=6.0, hold=4,
             rm_hz=1100.0, rm_mix=0.5, comb_hz=310.0, trash=1.0, hat_rm_hz=7300.0, hat_hold=6,
             ph_oct=-1.0, ph_detune=14.0, ph_fold=2.5, scream_lo=300.0, scream_octs=3.5, scream_q=6.0,
             ph_hold=3, ph_bits=7.0, ratchet=0.0)
    if rng is None:
        return d
    d.update(kick_root=rng.uniform(38, 55), kick_hi=rng.uniform(120, 420), kick_fold=rng.uniform(0.5, 4.0),
             mouth=rng.uniform(0.15, 0.8), bits=float(rng.choice([4.0, 5.0, 6.0, 8.0])), hold=int(rng.integers(2, 10)),
             rm_hz=rng.uniform(500, 2600), rm_mix=rng.uniform(0.2, 0.9), comb_hz=rng.uniform(180, 520),
             trash=rng.uniform(0.3, 1.6), hat_rm_hz=rng.uniform(4500, 11000), hat_hold=int(rng.integers(3, 12)),
             ph_oct=float(rng.choice([-1.0, -1.0, -2.0, 0.0])), ph_detune=rng.uniform(5, 35),
             ph_fold=rng.uniform(1.0, 5.0), scream_lo=rng.uniform(180, 500), scream_octs=rng.uniform(2.5, 4.5),
             scream_q=rng.uniform(3, 12), ph_hold=int(rng.integers(1, 8)), ph_bits=float(rng.choice([5.0, 6.0, 8.0])),
             ratchet=rng.uniform(0.05, 0.3))
    return d


# ------------------------------------------------------------------ struck

MEMBRANE = (1.0, 1.59, 2.14, 2.30, 2.65, 2.92)
PLATE = (1.0, 1.47, 2.09, 2.56, 3.14, 3.86)


def strike(h: Hit, n: int, sr: int, tilt: float) -> np.ndarray:
    """The exciter: the mouth's transient plus a click, so a soft mouth sound still rings."""
    x = pad_to(h.mouth, n).copy()
    x[h.pre] += 0.5
    return filt(x, "hp", tilt, 0.7, sr)


def struck_kick(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    t60 = float(np.clip(0.35 + 1.6 * h.dur, 0.35, 1.3))
    n = int((t60 + 0.05) * sr)
    f0 = p["kick_f0"] * 2 ** (0.3 * h.bright)
    gains = [1.0 / (i + 1) ** p["kick_dark"] for i in range(len(MEMBRANE))]
    x = strike(h, n, sr, 30)
    y = modes(x, [f0 * r for r in MEMBRANE], [t60 / (1 + 0.7 * i) for i in range(len(MEMBRANE))], gains, sr)
    thump = filt(x, "lp", 1500, 0.7, sr) * env_ad(n, sr, 0.0003, 0.03)  # the membrane takes 20 ms to speak; the stick does not
    return np.tanh((norm(y) + 0.6 * norm(thump)) * (1.2 + 2.5 * h.vel))


def struck_snare(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    t60 = float(np.clip(0.18 + 0.8 * h.dur, 0.18, 0.6))
    n = int((t60 + 0.05) * sr)
    x = strike(h, n, sr, 150)
    f = p["sn_f"] * 2 ** (0.6 * h.bright)
    pipe = comb(x, f, t60, sr, p["sn_damp"])
    plate = modes(x, [p["plate_f"] * r for r in PLATE], [t60 * 0.6] * len(PLATE), [1.0] * len(PLATE), sr)
    y = norm(pipe) + p["plate_mix"] * norm(plate) + 0.5 * filt(x, "hp", 1500, 0.7, sr)
    return np.tanh(norm(y) * (1.0 + 2.0 * h.vel))


def struck_hat(h: Hit, p: dict, rng, sr: int) -> np.ndarray:
    t60 = float(np.clip(0.3 + h.dur, 0.3, 0.8)) if h.dur > 0.18 else float(np.clip(0.05 + 0.3 * h.dur, 0.05, 0.14))
    n = int((t60 + 0.03) * sr)
    x = strike(h, n, sr, 3000)
    freqs = [f * 2 ** (0.4 * h.bright) for f in p["glass"]]
    return norm(norm(modes(x, freqs, [t60] * len(freqs), [1.0] * len(freqs), sr)) + 0.2 * norm(x))


def struck_phrase(c: Controls, p: dict, rng, sr: int) -> np.ndarray:
    t = t_axis(len(c.f), sr)
    f = c.f * 2.0 ** p["ph_oct"]
    y = np.zeros(len(f))
    for k in range(1, p["ph_partials"] + 1):
        fk = f * k * np.sqrt(1 + p["ph_stiff"] * k * k)
        level = np.exp(-np.log(k) * (p["ph_roll"] - 1.2 * c.brt - 0.8 * c.acc))  # brighter hum, more upper partials
        trem = 1 + 0.3 * np.sin(TAU * rng.uniform(2.5, 6.5) * t + rng.uniform(0, TAU))
        y += np.where(fk < 0.45 * sr, level * trem, 0.0) * np.sin(phase_of(fk, sr))
    y = y * (1 + p["ph_breath"] * norm(c.mouth))  # the hum itself roughens the tone
    return np.tanh(0.8 * y) * c.amp**0.8


def struck_params(rng) -> dict:
    d = dict(kick_f0=52.0, kick_dark=1.2, sn_f=220.0, sn_damp=0.35, plate_f=410.0, plate_mix=0.6,
             glass=(4300.0, 5650.0, 6900.0, 8200.0, 9800.0, 11400.0),
             ph_oct=0.0, ph_partials=7, ph_stiff=0.004, ph_roll=1.9, ph_breath=0.5, ratchet=0.0)
    if rng is None:
        return d
    d.update(kick_f0=rng.uniform(42, 70), kick_dark=rng.uniform(0.6, 2.0), sn_f=rng.uniform(140, 380),
             sn_damp=rng.uniform(0.15, 0.6), plate_f=rng.uniform(280, 700), plate_mix=rng.uniform(0.2, 1.0),
             glass=tuple(sorted(rng.uniform(3200, 12500, 6))), ph_oct=float(rng.choice([0.0, 0.0, -1.0, 1.0])),
             ph_partials=int(rng.integers(4, 10)), ph_stiff=float(rng.choice([0.0, 0.004, 0.012, 0.03])),
             ph_roll=rng.uniform(1.3, 2.6), ph_breath=rng.uniform(0.0, 1.2), ratchet=rng.uniform(0.0, 0.2))
    return d


# ---------------------------------------------------------------- palettes


@dataclass
class Palette:
    name: str
    blurb: str
    hits: dict  # role -> voice
    phrase: object
    params: object
    levels: dict  # role or "phrase" -> gain


PALETTES = {
    "rubber": Palette("rubber", "Clean and elastic: pitch-drop kicks, FM snaps, bubbles, a bending FM bass.",
                      {"kick": rubber_kick, "snare": rubber_snare, "hat": rubber_hat}, rubber_phrase, rubber_params,
                      {"kick": 1.0, "snare": 0.75, "hat": 0.4, "phrase": 0.6}),
    "crushed": Palette("crushed", "Blown out: clipped, folded and bit-crushed, with your mouth's transient layered in.",
                       {"kick": crushed_kick, "snare": crushed_snare, "hat": crushed_hat}, crushed_phrase, crushed_params,
                       {"kick": 1.0, "snare": 0.8, "hat": 0.35, "phrase": 0.55}),
    "struck": Palette("struck", "Your mouth is the stick: each hit rings a tuned membrane, pipe or glass.",
                      {"kick": struck_kick, "snare": struck_snare, "hat": struck_hat}, struck_phrase, struck_params,
                      {"kick": 1.0, "snare": 0.8, "hat": 0.45, "phrase": 0.6}),
}
