"""plugins: the loop played on installed audio plugins, mangled per voice by Airwindows.

Research only; nothing here ships. Ultra Analog Session 2 (a virtual analog)
plays the kick body, a filter-ring "laser" sweep, the buzzy noise bursts of the
hat and the bass. Lounge Lizard Session 4 (an electric piano whose hammer is
turned into a noise source) plays the snare crack and the hat tick. Each hit
is rendered on its own at 96 kHz, summed with the take's own transient, run
through its own chain of Airwindows plugins (wavefolder, sample-rate and bit
crushers, drum-slam, clipper, resonant filter) and peak-levelled, so the
mangle is per voice and there is no bus.

Surge XT is not installed on this machine, so it is not used.

All patch values below are plain raw 0..1 plugin values found by hand and by
a small measured search (kick filter-ring sweeps, snare/hat candidates judged
by spectrograms); roll 1 is exactly these, rolls 2 and 3 move them by small
seeded amounts. Ultra Analog keeps free-running analog state between notes
(a full reset costs 120 ms per call), so two renders of one roll agree in
character and level but not sample for sample (differences up to ~5% of peak).
"""

from __future__ import annotations

import sys
import zlib
from pathlib import Path

import numpy as np
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import sketch  # noqa: E402

SR = 96000
VST = "/Library/Audio/Plug-Ins/VST3/Applied Acoustics Systems/"
AU = "/Library/Audio/Plug-Ins/Components/"

# --------------------------------------------------------------- patches

UA_BASE = dict(
    playing_mode=0.0, playing_unison=0.0, global_tune=0.5, global_volume=0.5, effect_on=0.0,
    amp_attack=0.0, amp_sustain=0.0, vco_lfo=0.5, vco_sync=0.0, vco_pw=1.0,
)
KICK_BODY = dict(filter_resonance=0.55, filter_frequency=0.24, filter_envelope=0.62, filter_decay=0.42,
                 amp_decay=0.57, amp_release=0.3)
KICK_SWEEP = dict(filter_resonance=0.9, filter_frequency=0.16, filter_envelope=0.9, filter_decay=0.33,
                  amp_decay=0.42, amp_release=0.2)
SNARE_BUZZ = dict(filter_resonance=0.6, filter_frequency=0.7, filter_envelope=0.7, filter_decay=0.4,
                  amp_decay=0.5, amp_release=0.2, vco_sync=0.5, vco_pw=0.5)
HAT_NOISE = dict(filter_resonance=0.95, filter_frequency=0.8, filter_envelope=0.7, filter_decay=0.4,
                 amp_decay=0.42, amp_release=0.2, vco_sync=1.0, vco_pw=0.5)
BASS = dict(playing_mode=0.25, playing_unison=0.25, filter_resonance=0.62, filter_frequency=0.42,
            filter_envelope=0.72, filter_decay=0.4, amp_decay=0.6, amp_release=0.25, amp_sustain=0.06,
            vco_sync=0.06, vco_pw=0.6)
LL_BASE = dict(hammer_noise_amount=1.0, fork_tine_amp=1.0, fork_tone_amp=0.48, pickup_symmetry=0.74,
               amp_character=0.0, global_volume=0.7, global_tune=0.5)

# Airwindows chains: (file, {param: raw})
CHAINS = {
    "kick": [("DrumSlam", dict(drive=0.3)), ("ADClip8", dict(boost_db=0.12, ceiling=0.88))],
    "snare": [("Fracture2", dict(drive=0.3, fracture=0.25, threshold=0.5)), ("DeRez2", dict(rate=0.8, rez=0.85, hard=1.0)),
              ("ADClip8", dict(boost_db=0.2, ceiling=0.88))],
    "hat": [("Pockey2", dict(defreq=0.3, derez=0.7)), ("Fracture2", dict(drive=0.2, fracture=0.1, threshold=0.5)),
            ("ADClip8", dict(boost_db=0.15, ceiling=0.88))],
    "bass": [("Fracture2", dict(drive=0.12, fracture=0.0, threshold=0.5)), ("DrumSlam", dict(drive=0.08))],
}
LEVEL = {"kick": 0.9, "snare": 0.95, "hat": 0.7, "bass": 0.3}

# --------------------------------------------------------------- hosting

_cache: dict = {}


def _plug(key: str, path: str):
    if key not in _cache:
        import pedalboard as pb

        _cache[key] = pb.load_plugin(path)
    return _cache[key]


def _set(p, d: dict) -> None:
    for k, v in d.items():
        p.parameters[k].raw_value = float(np.clip(v, 0.0, 1.0))


def _play(p, note: int, vel: float, dur: float, total: float, reset: bool = False) -> np.ndarray:
    # reset=True costs 120 ms per call, so a render resets once (begin()) and every hit after that
    # starts with all-sound-off instead; the render is still deterministic because the call order is.
    msgs = [(bytes([0xB0, 120, 0]), 0.0), (bytes([0xB0, 123, 0]), 0.0),
            (bytes([0x90, int(note), int(np.clip(vel, 1, 127))]), 0.0), (bytes([0x80, int(note), 0]), float(dur))]
    y = p(msgs, duration=float(total), sample_rate=SR, num_channels=2, reset=reset)
    return np.asarray(y, float).mean(0)


def begin() -> None:
    """Start a render from the same plugin state every time."""
    _play(_plug("ua", VST + "Ultra Analog Session 2.vst3"), 60, 1, 0.01, 0.02, reset=True)
    _play(_plug("ll", VST + "Lounge Lizard Session 4.vst3"), 60, 1, 0.01, 0.02, reset=True)


def ua_hit(patch: dict, note: int, vel: float, dur: float, total: float) -> np.ndarray:
    p = _plug("ua", VST + "Ultra Analog Session 2.vst3")
    _set(p, {**UA_BASE, **patch})
    return _play(p, note, vel, dur, total)


def ll_hit(note: int, vel: float, dur: float, total: float, **over) -> np.ndarray:
    p = _plug("ll", VST + "Lounge Lizard Session 4.vst3")
    p.reverb_on = False
    _set(p, {**LL_BASE, **over})
    return _play(p, note, vel, dur, total)


def chain(role: str, y: np.ndarray, amt: dict, pre: float = 0.4) -> np.ndarray:
    """Run `y` through the role's Airwindows chain; `amt` scales/overrides the first parameter of named plugins."""
    y = peak_to(y, pre)
    x = np.stack([y, y]).astype(np.float32)
    for name, prm in CHAINS[role]:
        p = _plug("aw" + name, AU + name + ".component")
        prm = dict(prm)
        for k, v in amt.get(name, {}).items():
            prm[k] = v
        _set(p, prm)
        x = p(x, SR, reset=True)
        x = np.nan_to_num(np.asarray(x, np.float32))
    return x.mean(0).astype(float)


# --------------------------------------------------------------- helpers


def fade(y: np.ndarray, ms: float = 4.0) -> np.ndarray:
    n = min(len(y), int(ms * 1e-3 * SR))
    y = y.copy()
    if n:
        y[-n:] *= np.cos(np.linspace(0, np.pi / 2, n)) ** 2
    return y


def lp(y, fc):
    return signal.sosfilt(signal.butter(2, fc, "low", fs=SR, output="sos"), y)


def tilt(y, fc):
    """The performer's brightness as a tone control: a 2-pole low-pass that opens with the hit's brightness."""
    return lp(y, float(np.clip(fc, 200, 0.45 * SR)))


def dcblock(y, fc=20.0):
    """The asymmetric clippers leave a DC shift on kick and bass; a 20 Hz 2-pole high-pass per voice removes it."""
    return hp(y, fc, 2)


def hp(y, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc, "high", fs=SR, output="sos"), y)


def mix(*parts) -> np.ndarray:
    n = max(len(p) for p in parts)
    out = np.zeros(n)
    for p in parts:
        out[: len(p)] += p
    return out


def peak_to(y: np.ndarray, target: float) -> np.ndarray:
    pk = np.abs(y).max()
    return y * (target / pk) if pk > 1e-9 else y


def hit_mouth(h, n: int) -> np.ndarray:
    m = np.zeros(n)
    if len(h.mouth) > 8:
        x = signal.resample_poly(np.asarray(h.mouth, float), SR // 48000, 1)[:n]
        m[: len(x)] = x
    return m


def jit(rng, k: float) -> float:
    return 0.0 if rng is None else float(rng.uniform(-k, k))


# --------------------------------------------------------------- voices


def kick(h, P, ctx) -> np.ndarray:
    v = 70 + 57 * h.vel
    ad = 0.5 + 0.07 * min(1.0, h.dur / 0.14) + P["k_decay"]
    note = P["k_note"]
    body = ua_hit({**KICK_BODY, "amp_decay": ad, "filter_frequency": 0.2 + 0.05 * h.vel + P["k_cut"]}, note, v, 0.1, 0.22)
    sw_note = 48 + int(round(P["k_sweep_note"] + 6 * h.bright))
    sweep = ua_hit({**KICK_SWEEP, "filter_frequency": 0.12 + 0.08 * h.bright, "filter_envelope": 0.84 + 0.1 * h.bright}, sw_note, v, 0.05, 0.1)
    body, sweep = fade(body, 6), fade(sweep, 3)
    n = len(body)
    click = hp(hit_mouth(h, n), 1200) * P["k_mouth"] * (0.4 + h.bright)
    y = mix(peak_to(body, 1.0), peak_to(sweep, 1.0) * (0.03 + 0.55 * h.bright ** 1.3) * P["k_sweep"], click)
    y = chain("kick", y, {"DrumSlam": {"drive": 0.15 + 0.3 * h.bright * P["k_drive"]}})
    return dcblock(tilt(y, 600 * 6 ** h.bright * P["tilt"]))


def snare(h, P, ctx) -> np.ndarray:
    v = 70 + 57 * h.vel
    note = 55 + int(round(12 * h.bright)) + P["s_shift"]
    long_ = np.clip((h.dur - 0.05) / 0.16, 0, 1)
    crack = ll_hit(note, v, 0.03 + 0.05 * long_, 0.3)
    buzz = ua_hit({**SNARE_BUZZ, "amp_decay": 0.46 + 0.05 * long_, "filter_frequency": 0.55 + 0.25 * h.bright, "vco_sync": 0.4 + 0.3 * h.bright},
                  52 + P["s_shift"], v, 0.05, 0.3)
    n = len(crack)
    m = hit_mouth(h, n) * P["s_mouth"]
    wires = ua_hit({**HAT_NOISE, "amp_decay": 0.44 + 0.06 * long_, "filter_frequency": 0.6 + 0.2 * h.bright}, 80, v, 0.04, 0.3)
    wires = hp(peak_to(fade(wires, 5), 1.0), 2500)
    y = mix(peak_to(crack, 1.0), peak_to(fade(buzz, 4), 1.0) * 0.45 * P["s_buzz"], wires * (0.05 + 0.9 * h.bright ** 1.5), m)
    y = y * np.exp(-np.arange(len(y)) / SR / (0.05 + 0.08 * long_))  # tame the long LL tail into a snare shape
    y = chain("snare", y * 1.6, {"Fracture2": {"drive": 0.2 + 0.3 * h.bright * P["s_fold"]}, "DeRez2": {"rate": 0.7 + 0.25 * h.bright}})
    return tilt(y, 2500 * 5 ** h.bright * P["tilt"])


def hat(h, P, ctx) -> np.ndarray:
    v = 60 + 67 * h.vel
    open_ = float(np.clip((h.dur - 0.09) / 0.12, 0, 1))
    note = 84 + int(round(8 * h.bright)) + P["h_shift"]
    noise = ua_hit({**HAT_NOISE, "amp_decay": 0.38 + 0.12 * open_, "filter_frequency": 0.7 + 0.15 * h.bright},
                   note, v, 0.02 + 0.1 * open_, 0.3)
    tick = ll_hit(96 + int(round(6 * h.bright)), v, 0.015, 0.2, hammer_noise_amount=0.6 + 0.4 * h.bright)
    n = len(noise)
    m = hp(hit_mouth(h, n), 5000) * P["h_mouth"]
    y = mix(hp(peak_to(noise, 1.0), 3000) * (0.7 + 0.3 * h.bright), peak_to(tick, 1.0) * 0.5, m)
    env = np.exp(-np.arange(len(y)) / SR / (0.012 + 0.07 * open_))
    y = fade(y * env, 5)
    y = chain("hat", y * 1.5, {"Pockey2": {"defreq": 0.15 + 0.35 * (1 - h.bright) * P["h_crush"]}})
    return tilt(y, 4500 * 3.5 ** h.bright * P["tilt"])


VOICES = {"kick": kick, "snare": snare, "hat": hat}


# --------------------------------------------------------------- bass


def bass_note(nt, P, sr_loop: int, kick_times: list) -> np.ndarray:
    dur = (nt.end - nt.start) / sr_loop
    note = int(nt.midi) + P["b_oct"]
    v = 70 + 57 * nt.vel
    patch = {**BASS, "filter_frequency": 0.34 + P["b_cut"] + 0.3 * nt.brt + 0.14 * nt.vel, "filter_resonance": 0.6 + P["b_res"]}
    y = ua_hit(patch, note, v, dur, dur + 0.15)
    return y


def render_bass(loop, P) -> np.ndarray:
    out = np.zeros(loop.n * (SR // loop.sr))
    k = SR // loop.sr
    kicks = sorted(h.sample * k for h in loop.hits if h.role == "kick")
    for nt in loop.bass:
        y = bass_note(nt, P, loop.sr, kicks)
        n = int((nt.end - nt.start) / loop.sr * SR) + int(0.02 * SR)
        y = fade(y[:n], 15)
        y = chain("bass", y, {}, pre=0.25)
        y = dcblock(lp(y, P["b_lp"]))
        y = peak_to(y, 1.0) * (0.5 + 0.5 * nt.vel)
        a = nt.start * k
        y = y[: len(out) - a]
        out[a : a + len(y)] += y
    # duck under every kick, so the kick's thump has the low end to itself
    g = np.ones_like(out)
    t = np.arange(len(out)) / SR
    for kt in kicks:
        d = np.zeros_like(out)
        m = t >= kt / SR
        d[m] = np.exp(-(t[m] - kt / SR) / P["b_duck_tau"])
        g = np.minimum(g, 1 - P["b_duck"] * d)
    return out * g


# --------------------------------------------------------------- loop


def render_loop(loop, P, roll: int) -> np.ndarray:
    begin()
    k = SR // loop.sr
    n = loop.n * k
    out = np.zeros(n)
    for ri, role in enumerate(("kick", "snare", "hat")):
        group = sorted((h for h in loop.hits if h.role == role), key=lambda h: h.sample)
        lane = np.zeros(n)
        for i, h in enumerate(group):
            y = VOICES[role](h, P, None)
            n0 = h.sample * k
            if i + 1 < len(group):
                room = group[i + 1].sample * k - n0
                if room < len(y):
                    y = fade(y[:room], 3.0)
            y = peak_to(y, 1.0)
            y = y[: n - n0]
            lane[n0 : n0 + len(y)] += y * LEVEL[role] * P["lvl_" + role] * 10 ** ((h.vel - 1) * 24 / 20)
        out += lane
    if loop.bass:
        out += render_bass(loop, P) * LEVEL["bass"] * P["lvl_bass"]
    return signal.resample_poly(out, 1, k)


# --------------------------------------------------------------- params


def params(rng) -> dict:
    P = dict(tilt=1.0, k_note=31, k_decay=0.0, k_cut=0.0, k_sweep_note=0, k_sweep=1.0, k_mouth=0.12, k_drive=1.0,
             s_shift=0, s_mouth=0.25, s_buzz=1.0, s_fold=1.0,
             h_shift=0, h_mouth=0.2, h_crush=1.0,
             b_oct=0, b_cut=0.0, b_res=0.0, b_lp=650.0, b_duck=0.85, b_duck_tau=0.06,
             lvl_kick=1.0, lvl_snare=1.0, lvl_hat=1.0, lvl_bass=1.0)
    if rng is None:
        return P
    P["tilt"] = float(rng.uniform(0.8, 1.25))
    P["k_note"] = int(rng.choice([29, 31, 33]))
    P["k_decay"] = float(rng.uniform(-0.02, 0.03))
    P["k_cut"] = float(rng.uniform(-0.04, 0.05))
    P["k_sweep_note"] = int(rng.integers(-3, 4))
    P["k_sweep"] = float(rng.uniform(0.7, 1.4))
    P["k_drive"] = float(rng.uniform(0.7, 1.4))
    P["s_shift"] = int(rng.integers(-3, 5))
    P["s_buzz"] = float(rng.uniform(0.6, 1.4))
    P["s_fold"] = float(rng.uniform(0.6, 1.5))
    P["h_shift"] = int(rng.integers(-4, 5))
    P["h_crush"] = float(rng.uniform(0.5, 1.5))
    P["b_cut"] = float(rng.uniform(-0.05, 0.08))
    P["b_res"] = float(rng.uniform(-0.05, 0.1))
    P["b_lp"] = float(rng.uniform(600, 1400))
    return P


PALETTE = sketch.Palette(
    name="plugins",
    blurb="Analog-synth kicks with a laser chirp, electric-piano-hammer snare cracks and fizzy bit-crushed hats, over a ducked acid bass.",
    hits={"kick": lambda h, p, rng, sr: np.zeros(1), "snare": lambda h, p, rng, sr: np.zeros(1), "hat": lambda h, p, rng, sr: np.zeros(1)},
    phrase=lambda c, p, rng, sr: np.zeros(len(c.f)),
    params=params,
    levels={"kick": 1.0, "snare": 0.7, "hat": 0.32, "phrase": 0.5},
)
PALETTE.render_loop = render_loop
