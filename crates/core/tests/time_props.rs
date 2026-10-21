//! core-types §3.1: the time base.

mod common;

use common::{TEN_MINUTES_44K, any_rate, any_track, frame_bits, pos, rate, std_rate};
use proptest::prelude::*;
use takedown_core::control::ControlTrack;
use takedown_core::time::{
    ControlClock, MAX_RATE_HZ, MIN_RATE_HZ, SampleRate, control_index, frame_start,
    frames_covering, rescale_ceil, rescale_round,
};

/// Rate pairs with `to >= from`: common rates, any rates, and integer ratios.
fn rates_up() -> impl Strategy<Value = (SampleRate, SampleRate)> {
    prop_oneof![
        (std_rate(), std_rate()),
        (any_rate(), any_rate()),
        (MIN_RATE_HZ..=MAX_RATE_HZ / 8, 1u32..=8).prop_map(|(a, k)| (rate(a), rate(a * k))),
    ]
    .prop_map(|(a, b)| if a <= b { (a, b) } else { (b, a) })
}

/// Rate pairs in either direction.
fn rates_any() -> impl Strategy<Value = (SampleRate, SampleRate)> {
    prop_oneof![(std_rate(), std_rate()), (any_rate(), any_rate())]
}

/// A track, an output rate, and a start anywhere in or just past the track.
fn track_out_start() -> impl Strategy<Value = (ControlTrack, SampleRate, u64)> {
    (any_track(), std_rate()).prop_flat_map(|(t, out)| {
        let end = rescale_ceil(t.len(), t.rate(), out).0;
        (Just(t), Just(out), prop_oneof![0..=end + 500, pos(out)])
    })
}

proptest! {
    /// ★ CLAUDE.md: sample <-> control mapping.
    #[test]
    fn sample_control_mapping_is_exact((r, p) in any_rate().prop_flat_map(|r| (Just(r), pos(r)))) {
        let (frame, rem) = control_index(p, r);
        prop_assert_eq!(
            u128::from(frame) * u128::from(r.hz()) + u128::from(rem),
            u128::from(p) * 200
        );
        prop_assert!(rem < r.hz());
        for k in [frame.saturating_sub(1), frame, frame + 1] {
            let s = frame_start(k, r);
            prop_assert_eq!(control_index(s, r).0, k);
            if k > 0 {
                prop_assert_eq!(control_index(s - 1, r).0, k - 1);
            }
        }
        prop_assert!(control_index(p + 1, r) > (frame, rem));
    }

    #[test]
    fn clock_never_drifts(r in any_rate(), start in 0u64..=(1 << 40), n in 0u64..=100_000) {
        let mut c = ControlClock::at(r, start);
        for i in 0..n {
            if i % 4_099 == 0 {
                prop_assert_eq!(c, ControlClock::at(r, start + i));
            }
            c.advance();
        }
        prop_assert_eq!(c, ControlClock::at(r, start + n));
    }

    #[test]
    fn frames_covering_is_tight(
        r in any_rate(),
        n in prop_oneof![0u64..1_000, 0u64..=(1 << 44)],
    ) {
        let f = frames_covering(n, r);
        let time = u128::from(n) * 200;
        let hz = u128::from(r.hz());
        // Exactly the frames k with k / 200 < n / rate.
        prop_assert!(u128::from(f) * hz >= time);
        if f > 0 {
            prop_assert!(u128::from(f - 1) * hz < time);
        }
    }

    #[test]
    fn rescale_round_trips_up_then_down(
        ((a, b), p) in rates_up().prop_flat_map(|(a, b)| (Just((a, b)), pos(a))),
    ) {
        let q = rescale_round(p, a, b);
        prop_assert_eq!(rescale_round(q, b, a), p);
        prop_assert!(rescale_round(p + 1, a, b) >= q);
        let err = (u128::from(q) * u128::from(a.hz())).abs_diff(u128::from(p) * u128::from(b.hz()));
        prop_assert!(2 * err <= u128::from(a.hz()), "more than half an output sample off");
    }

    #[test]
    fn rescale_ceil_is_first_sample_at_or_after(
        ((a, b), p) in rates_any().prop_flat_map(|(a, b)| (Just((a, b)), pos(a))),
    ) {
        let (q, lead) = rescale_ceil(p, a, b);
        let exact = u128::from(p) * u128::from(b.hz());
        let d = u128::from(a.hz());
        prop_assert!(u128::from(q) * d >= exact);
        if q > 0 {
            prop_assert!(u128::from(q - 1) * d < exact);
        }
        let num = u128::from(q) * d - exact;
        prop_assert!(num < d);
        // One correctly rounded division; f64 then f32 is innocuous at these widths.
        let want = (num as f64 / d as f64) as f32;
        prop_assert_eq!(lead.to_bits(), want.to_bits());
        prop_assert!((0.0..1.0).contains(&lead));
    }

    #[test]
    fn curve_evaluation_is_position_pure(
        (t, out, start) in track_out_start(),
        run in 0u64..=2_048,
        probe in any::<u64>(),
    ) {
        let mut clock = ControlClock::at(out, start);
        for i in 0..run {
            let want = frame_bits(&t.value_at(out, start + i));
            prop_assert_eq!(frame_bits(&t.at_clock(&clock)), want, "sample {}", start + i);
            clock.advance();
        }
        // On the grid, the stored values come back bit for bit.
        let frames = t.frames();
        let hz = u64::from(out.hz());
        for k in (0..frames.min(64)).chain([frames - 1, probe % frames, frames + 3]) {
            if k * hz % 200 != 0 {
                continue;
            }
            let v = t.value_at(out, k * hz / 200);
            let i = k.min(frames - 1) as usize;
            let c = t.curves();
            prop_assert_eq!(
                frame_bits(&v),
                [
                    c.pitch_st[i].to_bits(),
                    c.amp_db[i].to_bits(),
                    c.bright[i].to_bits(),
                    c.breath[i].to_bits(),
                    c.vowel_x[i].to_bits(),
                    c.vowel_y[i].to_bits(),
                ]
            );
        }
    }
}

#[test]
fn clock_never_drifts_over_ten_minutes_at_44k() {
    let r = rate(44_100);
    let mut c = ControlClock::at(r, 0);
    for _ in 0..TEN_MINUTES_44K {
        c.advance();
    }
    assert_eq!(c, ControlClock::at(r, TEN_MINUTES_44K));
    assert_eq!(c.frame(), 120_000);
    assert_eq!(c.frac().to_bits(), 0);
}

#[test]
fn ten_minutes_is_120000_frames() {
    assert_eq!(frames_covering(TEN_MINUTES_44K, rate(44_100)), 120_000);
    assert_eq!(frames_covering(28_800_000, rate(48_000)), 120_000);
}

#[test]
fn frames_alternate_220_and_221_samples_at_44k() {
    let r = rate(44_100);
    for k in 0..1_000 {
        let n = frame_start(k + 1, r) - frame_start(k, r);
        assert_eq!(n, if k % 2 == 0 { 221 } else { 220 }, "frame {k}");
    }
}

#[test]
fn extremes_saturate_without_panicking() {
    for r in [rate(MIN_RATE_HZ), rate(MAX_RATE_HZ)] {
        let _ = control_index(u64::MAX, r);
        assert_eq!(frame_start(u64::MAX, rate(MAX_RATE_HZ)), u64::MAX);
        let _ = frames_covering(u64::MAX, r);
        let mut c = ControlClock::at(r, u64::MAX);
        c.advance();
    }
    assert_eq!(
        rescale_round(u64::MAX, rate(MIN_RATE_HZ), rate(MAX_RATE_HZ)),
        u64::MAX
    );
    assert_eq!(
        rescale_ceil(u64::MAX, rate(MIN_RATE_HZ), rate(MAX_RATE_HZ)),
        (u64::MAX, 0.0)
    );
    assert_eq!(SampleRate::new(MIN_RATE_HZ - 1), None);
    assert_eq!(SampleRate::new(MAX_RATE_HZ + 1), None);
    assert!(serde_json::from_str::<SampleRate>("999").is_err());
    assert_eq!(
        serde_json::from_str::<SampleRate>("48000").ok(),
        SampleRate::new(48_000)
    );
}
