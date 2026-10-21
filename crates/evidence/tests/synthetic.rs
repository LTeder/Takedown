//! Evidence on synthetic signals with known answers.

use std::f64::consts::PI;

use takedown_evidence::decode::{DecodeParams, decode};
use takedown_evidence::frame::{CONTROL_HZ, frame_count};
use takedown_evidence::onset::{PickParams, pick};
use takedown_evidence::{Analysis, analyze};

/// Harmonic tone with time-varying f0 (Hz as a function of seconds).
fn tone(sr: u32, secs: f64, amp: f64, harmonics: &[f64], f0: impl Fn(f64) -> f64) -> Vec<f32> {
    let n = (secs * f64::from(sr)) as usize;
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / f64::from(sr);
            let f = f0(t);
            let mut s = 0.0;
            for (k, &a) in harmonics.iter().enumerate() {
                if f * ((k + 1) as f64) < 0.45 * f64::from(sr) {
                    s += a * (phase * (k + 1) as f64).sin();
                }
            }
            phase += 2.0 * PI * f / f64::from(sr);
            (amp * s) as f32
        })
        .collect()
}

/// Deterministic white noise in [-amp, amp].
fn noise(n: usize, amp: f32, seed: u64) -> Vec<f32> {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            amp * ((s >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) as f32
        })
        .collect()
}

fn cents(a: f64, b: f64) -> f64 {
    1200.0 * (a / b).log2()
}

fn frame_time(k: usize) -> f64 {
    k as f64 / CONTROL_HZ as f64
}

/// Fraction of frames in [t0, t1) whose decoded pitch is within `tol` cents of truth.
fn pitch_hit_rate(a: &Analysis, f0: impl Fn(f64) -> f64, t0: f64, t1: f64, tol: f64) -> (f64, f64) {
    let d = decode(&a.pitch, &DecodeParams::default());
    let (mut n, mut hit, mut voiced) = (0, 0, 0);
    for k in 0..a.frames() {
        let t = frame_time(k);
        if t < t0 || t >= t1 {
            continue;
        }
        n += 1;
        if cents(f64::from(d.f0_hz[k]), f0(t)).abs() <= tol {
            hit += 1;
        }
        if d.voiced[k] {
            voiced += 1;
        }
    }
    (
        f64::from(hit) / f64::from(n),
        f64::from(voiced) / f64::from(n),
    )
}

fn mean_over(a: &Analysis, t0: f64, t1: f64, f: impl Fn(usize) -> f32) -> f64 {
    let ks: Vec<usize> = (0..a.frames())
        .filter(|&k| (t0..t1).contains(&frame_time(k)))
        .collect();
    ks.iter().map(|&k| f64::from(f(k))).sum::<f64>() / ks.len() as f64
}

const VOICE: [f64; 5] = [1.0, 0.5, 0.33, 0.25, 0.2];

#[test]
fn frame_count_matches_grid() {
    for sr in [16_000, 44_100, 48_000, 96_000] {
        let x = vec![0.0f32; 12_345];
        assert_eq!(analyze(&x, sr).frames(), frame_count(12_345, sr));
    }
}

#[test]
fn vibrato_is_tracked_within_25_cents() {
    let f0 = |t: f64| 220.0 * 2f64.powf(50.0 / 1200.0 * (2.0 * PI * 5.5 * t).sin());
    for sr in [44_100, 48_000] {
        let x = tone(sr, 2.0, 0.3, &VOICE, f0);
        let (hit, voiced) = pitch_hit_rate(&analyze(&x, sr), f0, 0.1, 1.9, 25.0);
        assert!(hit > 0.98, "{sr}: hit rate {hit}");
        assert!(voiced > 0.98, "{sr}: voiced {voiced}");
    }
}

#[test]
fn scoop_into_a_note_has_no_octave_errors() {
    // Hold 150 Hz, scoop up an octave over 150 ms, hold 300 Hz.
    let f0 = |t: f64| {
        let u = ((t - 0.3) / 0.15).clamp(0.0, 1.0);
        150.0 * 2f64.powf(u)
    };
    let sr = 44_100;
    let x = tone(sr, 1.0, 0.3, &VOICE, f0);
    let a = analyze(&x, sr);
    let (hit, _) = pitch_hit_rate(&a, f0, 0.05, 0.95, 50.0);
    assert!(hit > 0.95, "hit rate {hit}");
    let (no_octave, _) = pitch_hit_rate(&a, f0, 0.05, 0.95, 600.0);
    assert!(
        (no_octave - 1.0).abs() < f64::EPSILON,
        "octave errors: {no_octave}"
    );
}

#[test]
fn hum_with_dominant_second_harmonic_reads_the_fundamental() {
    let f0 = |_| 110.0;
    let sr = 48_000;
    let x = tone(sr, 1.5, 0.3, &[0.3, 1.0, 0.5, 0.25, 0.1], f0);
    let (hit, voiced) = pitch_hit_rate(&analyze(&x, sr), f0, 0.1, 1.4, 50.0);
    assert!(hit > 0.95, "hit rate {hit}");
    assert!(voiced > 0.95, "voiced {voiced}");
}

#[test]
fn low_bass_voice_is_tracked() {
    let f0 = |_| 65.0;
    let sr = 44_100;
    let x = tone(sr, 1.5, 0.3, &VOICE, f0);
    let (hit, _) = pitch_hit_rate(&analyze(&x, sr), f0, 0.1, 1.4, 25.0);
    assert!(hit > 0.95, "hit rate {hit}");
}

#[test]
fn clicks_are_found_to_the_millisecond() {
    let sr = 44_100;
    let times = [0.2, 0.45, 0.8, 1.3];
    let mut x = noise((1.6 * f64::from(sr)) as usize, 1e-4, 7);
    for &t in &times {
        x[(t * f64::from(sr)) as usize] = 0.9;
    }
    let a = analyze(&x, sr);
    let picked = pick(&a.onsets, sr, &PickParams::default());
    assert_eq!(picked.len(), times.len(), "{picked:?}");
    for (p, &t) in picked.iter().zip(&times) {
        let err_ms = (p.sample as f64 / f64::from(sr) - t) * 1000.0;
        assert!(err_ms.abs() <= 2.0, "click at {t}: error {err_ms} ms");
    }
}

#[test]
fn noise_bursts_are_onsets_and_not_voiced() {
    let sr = 48_000;
    let times = [0.3, 0.7, 1.1];
    let mut x = noise((1.5 * f64::from(sr)) as usize, 3e-4, 11);
    let burst = noise((0.05 * f64::from(sr)) as usize, 0.5, 12);
    let attack = (0.005 * f64::from(sr)) as usize;
    for &t in &times {
        let start = (t * f64::from(sr)) as usize;
        for (i, &b) in burst.iter().enumerate() {
            let g = if i < attack {
                0.5 - 0.5 * (PI * i as f64 / attack as f64).cos()
            } else {
                1.0
            };
            x[start + i] += b * g as f32;
        }
    }
    let a = analyze(&x, sr);
    let picked = pick(&a.onsets, sr, &PickParams::default());
    assert_eq!(picked.len(), times.len(), "{picked:?}");
    for (p, &t) in picked.iter().zip(&times) {
        let err_ms = (p.sample as f64 / f64::from(sr) - t) * 1000.0;
        assert!(err_ms.abs() <= 5.0, "burst at {t}: error {err_ms} ms");
    }
    let d = decode(&a.pitch, &DecodeParams::default());
    let voiced = (0..a.frames()).filter(|&k| d.voiced[k]).count();
    assert!(
        voiced * 20 < a.frames(),
        "{voiced} of {} frames voiced in noise",
        a.frames()
    );
}

#[test]
fn loudness_is_calibrated_and_a_weighted() {
    let sr = 48_000;
    // Peak 0.1 sine: RMS -23.0 dBFS.
    let x1k = tone(sr, 0.5, 0.1, &[1.0], |_| 1000.0);
    let a1k = analyze(&x1k, sr);
    let l1k = mean_over(&a1k, 0.1, 0.4, |k| a1k.timbre[k].loudness_db);
    assert!((l1k + 23.01).abs() < 0.2, "1 kHz: {l1k}");
    let x100 = tone(sr, 0.5, 0.1, &[1.0], |_| 100.0);
    let a100 = analyze(&x100, sr);
    let l100 = mean_over(&a100, 0.1, 0.4, |k| a100.timbre[k].loudness_db);
    // 19.1 dB in theory; the 23 ms window spreads 100 Hz over bins of 43 Hz
    // where the weighting changes by several dB, so allow 1 dB.
    assert!(
        (l100 - l1k + 19.1).abs() < 1.0,
        "100 Hz is {} dB below 1 kHz",
        l1k - l100
    );
}

#[test]
fn brightness_in_harmonic_units_separates_sine_from_buzz() {
    let sr = 44_100;
    let sine = tone(sr, 0.6, 0.3, &[1.0], |_| 200.0);
    let a = analyze(&sine, sr);
    let h_sine = mean_over(&a, 0.1, 0.5, |k| a.timbre[k].centroid_harm);
    assert!(
        (h_sine - 1.0).abs() < 0.15,
        "sine centroid {h_sine} harmonics"
    );
    let saw: Vec<f64> = (1..=20).map(|k| 1.0 / f64::from(k)).collect();
    let buzz = tone(sr, 0.6, 0.1, &saw, |_| 200.0);
    let b = analyze(&buzz, sr);
    let h_buzz = mean_over(&b, 0.1, 0.5, |k| b.timbre[k].centroid_harm);
    // sum(k / k) / sum(1 / k) over 20 harmonics is about 5.5.
    assert!(
        (h_buzz - 5.5).abs() < 1.0,
        "buzz centroid {h_buzz} harmonics"
    );
}

#[test]
fn tilt_is_flat_for_white_noise_and_negative_when_lowpassed() {
    let sr = 44_100;
    let white = noise(sr as usize, 0.3, 3);
    let a = analyze(&white, sr);
    let t_white = mean_over(&a, 0.1, 0.9, |k| a.timbre[k].tilt_db_oct);
    assert!(t_white.abs() < 1.0, "white noise tilt {t_white} dB/oct");
    // One-pole lowpass at about 300 Hz: -6 dB/oct above the corner.
    let c = (-2.0 * PI * 300.0 / f64::from(sr)).exp() as f32;
    let mut y = 0.0f32;
    let dark: Vec<f32> = white
        .iter()
        .map(|&v| {
            y = (1.0 - c) * v + c * y;
            y
        })
        .collect();
    let b = analyze(&dark, sr);
    let t_dark = mean_over(&b, 0.1, 0.9, |k| b.timbre[k].tilt_db_oct);
    assert!(t_dark < -4.0, "lowpassed noise tilt {t_dark} dB/oct");
}

#[test]
fn cpp_falls_as_breath_noise_rises() {
    let sr = 44_100;
    let harmonics: Vec<f64> = (1..=30).map(|k| 1.0 / f64::from(k)).collect();
    let voiced = tone(sr, 0.8, 0.2, &harmonics, |_| 150.0);
    let rms =
        (voiced.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / voiced.len() as f64).sqrt();
    let mut cpps = Vec::new();
    for snr_db in [40.0, 10.0, 0.0, -20.0] {
        let amp = (rms * 3f64.sqrt() / 10f64.powf(snr_db / 20.0)) as f32;
        let n = noise(voiced.len(), amp, 5);
        let x: Vec<f32> = voiced.iter().zip(&n).map(|(a, b)| a + b).collect();
        let a = analyze(&x, sr);
        cpps.push(mean_over(&a, 0.1, 0.7, |k| a.timbre[k].cpp_db));
    }
    assert!(
        cpps.windows(2).all(|w| w[0] > w[1]),
        "CPP by SNR 40, 10, 0, -20 dB: {cpps:?}"
    );
    assert!(cpps[0] - cpps[3] > 6.0, "{cpps:?}");
}

#[test]
fn silence_is_silent() {
    let sr = 48_000;
    let a = analyze(&vec![0.0f32; sr as usize], sr);
    assert_eq!(a.onsets, []);
    let d = decode(&a.pitch, &DecodeParams::default());
    assert!(d.voiced.iter().all(|&v| !v));
    for t in &a.timbre {
        assert!(t.loudness_db <= -119.0);
        assert!(t.centroid_hz.is_nan() && t.cpp_db.is_nan() && t.tilt_db_oct.is_nan());
    }
}

#[test]
fn empty_take_has_no_frames() {
    let a = analyze(&[], 44_100);
    assert_eq!(a.frames(), 0);
    assert_eq!(a.onsets, []);
    assert_eq!(decode(&a.pitch, &DecodeParams::default()).f0_hz.len(), 0);
}

#[test]
fn cpp_is_stable_across_nearby_pitches() {
    let sr = 44_100;
    // Voice-like roll-off. (A flat 10-harmonic tone with nothing above it
    // still varies by about 2.5 dB: the cliff at the last harmonic dominates
    // its cepstrum.)
    let harmonics: Vec<f64> = (1..=40).map(|k| 1.0 / f64::from(k)).collect();
    let cpps: Vec<f64> = (0..12)
        .map(|i| {
            let f = 218.0 + 0.4 * f64::from(i);
            let t = tone(sr, 0.5, 0.3, &harmonics, |_| f);
            // A 40 dB SNR floor: in a noiseless tone the dips between
            // harmonics fall to window sidelobes, whose depth depends on
            // where harmonics sit between bins, and they dominate the cepstrum.
            let n = noise(t.len(), 3e-3, 4);
            let x: Vec<f32> = t.iter().zip(&n).map(|(a, b)| a + b).collect();
            let a = analyze(&x, sr);
            mean_over(&a, 0.1, 0.4, |k| a.timbre[k].cpp_db)
        })
        .collect();
    let lo = cpps.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = cpps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(hi - lo < 1.5, "CPP over 218 to 222.4 Hz: {cpps:?}");
}

#[test]
fn centroid_does_not_scale_with_capture_rate() {
    let centroid = |sr: u32| {
        let t = tone(sr, 0.5, 0.3, &VOICE, |_| 220.0);
        let n = noise(t.len(), 1e-4, 9);
        let x: Vec<f32> = t.iter().zip(&n).map(|(a, b)| a + b).collect();
        let a = analyze(&x, sr);
        mean_over(&a, 0.1, 0.4, |k| a.timbre[k].centroid_hz)
    };
    let (c44, c96) = (centroid(44_100), centroid(96_000));
    assert!(
        (c96 / c44 - 1.0).abs() < 0.05,
        "44.1 kHz {c44} Hz, 96 kHz {c96} Hz"
    );
}

#[test]
fn nan_samples_do_not_poison_neighbours() {
    let sr = 44_100;
    let mut x = tone(sr, 1.0, 0.3, &VOICE, |_| 220.0);
    x[22_050] = f32::NAN;
    let a = analyze(&x, sr);
    assert!(a.odf.iter().all(|v| v.is_finite()));
    assert!(
        a.timbre
            .iter()
            .skip(10)
            .take(180)
            .all(|t| t.loudness_db > -40.0)
    );
}
