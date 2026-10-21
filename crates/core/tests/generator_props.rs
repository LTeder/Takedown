//! The shared generators are not degenerate: over a few hundred draws, every
//! hard case of the core-types §3 generator table appears.

mod common;

use std::collections::BTreeMap;

use common::{any_rate, pos, rate, seed_tree, std_rate, track};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use takedown_core::control::{ControlTrack, Curve, Link, MAX_HIT_MS};
use takedown_core::time::{MAX_RATE_HZ, MIN_RATE_HZ, control_index};

fn draw<S: Strategy>(s: &S, n: usize) -> Vec<S::Value> {
    let mut runner = TestRunner::deterministic();
    (0..n)
        .map(|_| {
            s.new_tree(&mut runner)
                .expect("generator never rejects")
                .current()
        })
        .collect()
}

fn require(seen: &BTreeMap<&'static str, usize>) {
    let missing: Vec<_> = seen
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| *k)
        .collect();
    assert!(
        missing.is_empty(),
        "hard cases never generated: {missing:?}"
    );
}

#[test]
fn rates_and_positions_cover_their_hard_cases() {
    let mut seen: BTreeMap<&'static str, usize> = BTreeMap::new();
    for k in [
        "44.1k",
        "88.2k",
        "8k",
        "min",
        "max",
        "pos 0",
        "pos 10 min",
        "pos > 2^40",
        "frame boundary -1",
    ] {
        seen.insert(k, 0);
    }
    let mut hit = |k: &'static str| *seen.get_mut(k).unwrap() += 1;
    for r in draw(&std_rate(), 400) {
        match r.hz() {
            44_100 => hit("44.1k"),
            88_200 => hit("88.2k"),
            8_000 => hit("8k"),
            _ => {}
        }
    }
    for r in draw(&any_rate(), 400) {
        if r.hz() == MIN_RATE_HZ {
            hit("min");
        }
        if r.hz() == MAX_RATE_HZ {
            hit("max");
        }
    }
    let r = rate(44_100);
    for p in draw(&pos(r), 2_000) {
        if p == 0 {
            hit("pos 0");
        }
        if p == common::TEN_MINUTES_44K {
            hit("pos 10 min");
        }
        if p > 1 << 40 {
            hit("pos > 2^40");
        }
        if control_index(p + 1, r).1 == 0 && p > 0 {
            hit("frame boundary -1");
        }
    }
    require(&seen);
}

/// Curve values at -0.0, subnormal, or on a range bound.
fn curve_cases(t: &ControlTrack, hit: &mut impl FnMut(&'static str)) {
    for c in Curve::ALL {
        let (lo, hi) = c.range();
        for v in t.curves().get(c) {
            if v.to_bits() == (-0.0f32).to_bits() {
                hit("-0.0");
            }
            if v.is_subnormal() {
                hit("subnormal");
            }
            if v.to_bits() == lo.to_bits() || v.to_bits() == hi.to_bits() {
                hit("range bound");
            }
        }
    }
}

#[test]
fn tracks_cover_their_hard_cases() {
    let keys = [
        "tiny",
        "typical",
        "ten minutes",
        "retrigger",
        "legato",
        "span under 5 ms",
        "span ends at len",
        "dur 1",
        "dur at 60 s cap",
        "at before anchor",
        "at after anchor",
        "shared anchor",
        "more than 12 overlapping",
        "hit at 0",
        "hit ends at len",
        "12 roles",
        "-0.0",
        "subnormal",
        "range bound",
        "take",
    ];
    let mut seen: BTreeMap<&'static str, usize> = keys.iter().map(|k| (*k, 0)).collect();
    let mut hit = |k: &'static str| *seen.get_mut(k).unwrap() += 1;
    for t in draw(&std_rate().prop_flat_map(track), 600) {
        let hz = u64::from(t.rate().hz());
        let len = t.len();
        if len <= 400 {
            hit("tiny");
        } else if len <= 4 * hz {
            hit("typical");
        } else {
            hit("ten minutes");
        }
        for (i, s) in t.spans().iter().enumerate() {
            match s.link {
                Link::Retrigger => hit("retrigger"),
                Link::Legato => hit("legato"),
                Link::Detached => {}
            }
            if (s.off.0 - s.on.0) * 200 < hz {
                hit("span under 5 ms");
            }
            if s.off.0 == len && i + 1 == t.spans().len() {
                hit("span ends at len");
            }
        }
        let cap = t.rate().samples_for_ms(MAX_HIT_MS);
        let mut anchors: BTreeMap<u64, usize> = BTreeMap::new();
        for h in t.hits() {
            if h.dur == 1 {
                hit("dur 1");
            }
            if h.dur == cap {
                hit("dur at 60 s cap");
            }
            if h.at < h.anchor {
                hit("at before anchor");
            }
            if h.at > h.anchor {
                hit("at after anchor");
            }
            if h.at.0 == 0 {
                hit("hit at 0");
            }
            if h.at.0 + h.dur == len {
                hit("hit ends at len");
            }
            *anchors.entry(h.anchor.0).or_insert(0) += 1;
            let overlapping = t
                .hits()
                .iter()
                .filter(|o| o.at <= h.at && h.at.0 < o.at.0 + o.dur)
                .count();
            if overlapping > 12 {
                hit("more than 12 overlapping");
            }
        }
        if anchors.values().any(|n| *n >= 2) {
            hit("shared anchor");
        }
        if t.roles().len() == 12 {
            hit("12 roles");
        }
        curve_cases(&t, &mut hit);
        if t.take().is_some() {
            hit("take");
        }
    }
    require(&seen);
}

#[test]
fn seed_trees_cover_their_hard_cases() {
    let mut seen: BTreeMap<&'static str, usize> = [
        "no regions",
        "4 regions",
        "overlapping regions",
        "locks",
        "rolls",
    ]
    .iter()
    .map(|k| (*k, 0))
    .collect();
    let mut hit = |k: &'static str| *seen.get_mut(k).unwrap() += 1;
    for t in draw(&seed_tree(), 400) {
        assert!(t.validate().is_ok());
        match t.regions.len() {
            0 => hit("no regions"),
            4 => hit("4 regions"),
            _ => {}
        }
        let overlap = t.regions.iter().enumerate().any(|(i, a)| {
            t.regions[i + 1..]
                .iter()
                .any(|b| a.span.start() < b.span.end() && b.span.start() < a.span.end())
        });
        if overlap {
            hit("overlapping regions");
        }
        if !t.locked.is_empty() {
            hit("locks");
        }
        if !t.rolls.is_empty() {
            hit("rolls");
        }
    }
    require(&seen);
}
