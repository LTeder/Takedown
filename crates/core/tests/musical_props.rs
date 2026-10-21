//! core-types §3.4, `time_map_exactness`: timeline samples and SMF ticks
//! convert with exact rationals, piecewise across tempo changes.

mod common;

use common::{STD_RATES, rate, splitmix};
use proptest::collection::vec;
use proptest::prelude::*;
use takedown_core::musical::{MAX_PPQ, TempoMap, TempoSegment, TimeMap, TimeMapError};
use takedown_core::time::{MAX_RATE_HZ, SampleRate, TimelinePos, control_index};

/// Tempi: the musical range, the whole SMF range, and 120 BPM.
fn tempo_us() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => 200_000u32..=2_000_000,
        1 => 1u32..=0xFF_FFFF,
        1 => Just(500_000u32),
    ]
}

/// Any SMF map: coarse, typical and any PPQ; 1 to 3 tempo segments; any origin.
fn time_map() -> impl Strategy<Value = TimeMap> {
    (
        prop_oneof![2 => 1u16..=96, 3 => 24u16..=1920, 1 => 1u16..=MAX_PPQ],
        tempo_us(),
        vec((1u64..=50_000, tempo_us()), 0..=2),
        prop_oneof![
            2 => Just(0u64),
            2 => 0u64..100_000,
            1 => 0u64..(1 << 32),
        ],
    )
        .prop_map(|(ppq, first, rest, origin)| {
            let mut segments = vec![TempoSegment {
                tick: 0,
                us_per_quarter: first,
            }];
            let mut tick = 0;
            for (gap, us) in rest {
                tick += gap;
                segments.push(TempoSegment {
                    tick,
                    us_per_quarter: us,
                });
            }
            TimeMap::new(ppq, TempoMap::try_from(segments).unwrap(), origin).unwrap()
        })
}

/// `map` with PPQ, every tempo tick and the origin times `factor`, when that
/// stays inside SMF's ranges.
fn refine(map: &TimeMap, factor: u64) -> Option<TimeMap> {
    let ppq = u16::try_from(u64::from(map.ppq()) * factor).ok()?;
    let segments = map
        .tempo()
        .segments()
        .iter()
        .map(|s| {
            Some(TempoSegment {
                tick: s.tick.checked_mul(factor)?,
                us_per_quarter: s.us_per_quarter,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    TimeMap::new(
        ppq,
        TempoMap::try_from(segments).ok()?,
        map.origin_tick().checked_mul(factor)?,
    )
    .ok()
}

/// Ticks to probe: the origin and just after it, every tempo change at or
/// after the origin and its neighbours, and a few far ones.
fn probe_ticks(map: &TimeMap, salt: u64) -> Vec<u64> {
    let o = map.origin_tick();
    let mut out = vec![o, o + 1, o + 2, o + 959, o + 960, o + 961];
    for s in map.tempo().segments() {
        for t in [s.tick.saturating_sub(1), s.tick, s.tick + 1] {
            if t >= o {
                out.push(t);
            }
        }
    }
    for i in 0..16 {
        out.push(o + splitmix(salt ^ i) % 10_000_000);
    }
    out
}

/// Frames to probe: the first ones, those around every tempo change, and a
/// few far ones.
fn probe_frames(map: &TimeMap, r: SampleRate, salt: u64) -> Vec<u64> {
    let mut out: Vec<u64> = (0..40).collect();
    for s in map.tempo().segments() {
        let (k, _) = control_index(map.sample_of_tick(s.tick, r).0, r);
        out.extend(k.saturating_sub(3)..k + 3);
    }
    for i in 0..16 {
        let k = splitmix(salt ^ !i) % 2_000_000;
        out.extend([k, k + 1]);
    }
    out
}

proptest! {
    #[test]
    fn time_map_exactness(map in time_map(), salt in any::<u64>()) {
        for hz in STD_RATES {
            let r = rate(hz);
            let Some(e) = map.exportable_for(r) else {
                // Then no integer refinement at all is exportable.
                let mut m = 1;
                while let Some(refined) = refine(&map, m) {
                    prop_assert!(refined.check_exportable(r).is_err(), "factor {} passes at {} Hz", m, hz);
                    m += 1;
                }
                continue;
            };
            prop_assert_eq!(e.check_exportable(r), Ok(()));

            // An integer refinement, and the coarsest one.
            prop_assert_eq!(e.ppq() % map.ppq(), 0);
            let m = u64::from(e.ppq() / map.ppq());
            let expected = refine(&map, m);
            prop_assert_eq!(Some(&e), expected.as_ref());
            if m > 1 {
                prop_assert!(refine(&map, m - 1).unwrap().check_exportable(r).is_err());
            }
            if map.check_exportable(r).is_ok() {
                prop_assert_eq!(&e, &map);
            }

            // Every instant keeps its time, exactly, at any rate.
            for t in probe_ticks(&map, salt) {
                for probe in [r, rate(MAX_RATE_HZ)] {
                    prop_assert_eq!(map.sample_of_tick(t, probe), e.sample_of_tick(t * m, probe));
                }
            }

            // Ticks round-trip through their nearest samples, and samples move
            // monotonically: a tick is longer than a sample.
            for t in probe_ticks(&e, salt) {
                let s = e.sample_of_tick(t, r);
                prop_assert_eq!(e.tick_of_sample(s, r), t, "tick {} at {} Hz", t, hz);
                prop_assert!(e.sample_of_tick(t + 1, r) > s);
                let next = TimelinePos(s.0 + 1);
                prop_assert!(e.tick_of_sample(next, r) >= e.tick_of_sample(s, r));
            }

            // Distinct frames get strictly increasing ticks: a tick is at most
            // one frame long.
            for k in probe_frames(&e, r, salt) {
                prop_assert!(e.tick_of_frame(k + 1) > e.tick_of_frame(k), "frame {} at {} Hz", k, hz);
            }
        }
    }
}

#[test]
fn time_map_known_values() {
    let r48 = rate(48_000);
    let m = TimeMap::default_export();
    // 120 BPM at 960 PPQ: 1920 ticks a second.
    assert_eq!(m.tick_of_sample(TimelinePos(48_000), r48), 1_920);
    assert_eq!(m.sample_of_tick(960, r48), TimelinePos(24_000));
    assert_eq!(m.tick_of_frame(200), 1_920);
    // One tick is 25 samples at 48 kHz, so the nearest tick rounds half up.
    assert_eq!(m.tick_of_sample(TimelinePos(12), r48), 0);
    assert_eq!(m.tick_of_sample(TimelinePos(13), r48), 1);
    for hz in STD_RATES {
        assert_eq!(m.check_exportable(rate(hz)), Ok(()));
        assert_eq!(m.exportable_for(rate(hz)), Some(m.clone()));
    }

    // A tempo change: 120 BPM for one quarter, then 60 BPM.
    let tempo = TempoMap::try_from(vec![
        TempoSegment {
            tick: 0,
            us_per_quarter: 500_000,
        },
        TempoSegment {
            tick: 960,
            us_per_quarter: 1_000_000,
        },
    ])
    .unwrap();
    let m = TimeMap::new(960, tempo, 0).unwrap();
    assert_eq!(m.sample_of_tick(960, r48), TimelinePos(24_000));
    assert_eq!(m.sample_of_tick(1_920, r48), TimelinePos(72_000));
    assert_eq!(m.tick_of_sample(TimelinePos(72_000), r48), 1_920);
    assert_eq!(m.tick_of_frame(300), 1_920);

    // The origin is the tick of sample 0; earlier ticks clamp to sample 0.
    let m = TimeMap::new(960, TempoMap::constant(500_000).unwrap(), 1_920).unwrap();
    assert_eq!(m.tick_of_sample(TimelinePos(0), r48), 1_920);
    assert_eq!(m.sample_of_tick(2_880, r48), TimelinePos(24_000));
    assert_eq!(m.sample_of_tick(0, r48), TimelinePos(0));

    // A coarse map refines by the smallest factor that brings a tick to 5 ms:
    // 24 PPQ at 120 BPM is 20.8 ms a tick, so the factor is 5.
    let coarse = TimeMap::new(24, TempoMap::constant(500_000).unwrap(), 7).unwrap();
    assert_eq!(
        coarse.check_exportable(r48),
        Err(TimeMapError::NotExportable)
    );
    let fine = coarse.exportable_for(r48).unwrap();
    assert_eq!((fine.ppq(), fine.origin_tick()), (120, 35));

    // A tick of one sample or less is never exportable: 32767 PPQ at 1 µs a
    // quarter.
    let too_fine = TimeMap::new(32_767, TempoMap::constant(1).unwrap(), 0).unwrap();
    assert_eq!(too_fine.exportable_for(rate(8_000)), None);
}
