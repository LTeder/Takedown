//! core-types §3.6: `generators_are_not_degenerate`. The render properties
//! only prove something if their draws sound, reset and trigger.

use std::collections::BTreeMap;

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;
use takedown_render::contract::testkit::{
    FIXED_BLOCKS, RenderCase, ToyConfig, blocks, render_case, toy_config,
};

fn draw<S: Strategy>(s: &S, n: usize) -> Vec<S::Value> {
    let mut runner = TestRunner::deterministic();
    (0..n)
        .map(|_| {
            s.new_tree(&mut runner)
                .expect("generator never gives up")
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

/// More than ¾ of renders are audible, more than ⅓ have several segments,
/// more than ½ have triggers.
#[test]
fn generators_are_not_degenerate() {
    let cases: Vec<RenderCase> = draw(&render_case(), 160);
    let (mut renders, mut audible, mut several, mut triggered) = (0u32, 0u32, 0u32, 0u32);
    let mut seen: BTreeMap<&'static str, usize> = [
        "unequal rates",
        "44.1k timeline",
        "88.2k output",
        "slices",
        "phrase voice with spans",
        "hit voice with hits",
        "cells",
        "latency",
        "tail",
    ]
    .into_iter()
    .map(|k| (k, 0))
    .collect();
    let mut hit = |k: &'static str| *seen.get_mut(k).expect("listed") += 1;
    for case in &cases {
        if case.track.rate() != case.out_rate {
            hit("unequal rates");
        }
        if case.track.rate().hz() == 44_100 {
            hit("44.1k timeline");
        }
        if case.out_rate.hz() == 88_200 {
            hit("88.2k output");
        }
        for v in case.bind() {
            renders += 1;
            let out = v.mono(v.render_len());
            let peak = out.iter().fold(0.0f32, |p, x| p.max(x.abs()));
            audible += u32::from(peak > 1e-3);
            let active = v
                .bound
                .segments()
                .iter()
                .filter(|s| s.quiet_from > s.start)
                .count();
            several += u32::from(active >= 2);
            triggered += u32::from(!v.bound.triggers().is_empty());
            if v.bound.triggers().iter().any(|t| t.kind.slice().is_some()) {
                hit("slices");
            }
            if v.cfg.phrase && !case.track.spans().is_empty() {
                hit("phrase voice with spans");
            }
            if !v.cfg.phrase && !v.bound.triggers().is_empty() {
                hit("hit voice with hits");
            }
            if v.cfg.cell > 1 && !v.bound.triggers().is_empty() {
                hit("cells");
            }
            if v.cfg.latency > 0 && !v.bound.triggers().is_empty() {
                hit("latency");
            }
            if v.cfg.tail > 0 && !v.bound.triggers().is_empty() {
                hit("tail");
            }
        }
    }
    eprintln!(
        "{renders} renders: {audible} audible, {several} with several active segments, {triggered} with triggers"
    );
    assert!(4 * audible > 3 * renders, "{audible} of {renders} audible");
    assert!(
        3 * several > renders,
        "{several} of {renders} with several segments"
    );
    assert!(
        2 * triggered > renders,
        "{triggered} of {renders} with triggers"
    );
    require(&seen);
}

/// Every option of `toy_config` and the fixed block cycle are drawn.
#[test]
fn toy_config_and_blocks_cover_their_options() {
    let configs: Vec<ToyConfig> = draw(&toy_config(), 400);
    for (name, values, get) in [
        (
            "latency",
            vec![0, 1, 7, 64],
            (|c: &ToyConfig| c.latency) as fn(&ToyConfig) -> u32,
        ),
        ("cell", vec![0, 7, 32], |c: &ToyConfig| c.cell),
        ("tail", vec![0, 4, 30], |c: &ToyConfig| c.tail),
        ("slice_max_ms", vec![0, 30, u32::MAX], |c: &ToyConfig| {
            c.slice_max_ms
        }),
    ] {
        for v in values {
            assert!(
                configs.iter().any(|c| get(c) == v),
                "{name} {v} never drawn"
            );
        }
    }
    assert!(configs.iter().any(|c| c.phrase) && configs.iter().any(|c| !c.phrase));
    assert!(configs.iter().any(|c| c.roles.is_empty()));
    assert!(configs.iter().any(|c| c.roles.len() > 8));
    assert!(configs.iter().all(ToyConfig::honest));
    let seqs = draw(&blocks(256), 200);
    assert!(seqs.iter().any(|s| s[..] == FIXED_BLOCKS[..]));
    assert!(seqs.iter().flatten().any(|&n| n == 1));
    assert!(seqs.iter().flatten().any(|&n| n > 256));
    assert!(
        seqs.iter()
            .flatten()
            .all(|&n| (1..=512).contains(&n) || FIXED_BLOCKS.contains(&n))
    );
}
