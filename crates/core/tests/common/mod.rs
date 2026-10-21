//! Shared proptest generators (core-types §3). Each core test binary includes
//! this with `mod common;`; render's testkit includes the same file with
//! `#[path = "../../core/tests/common/mod.rs"] pub mod common;`. So it is one
//! file, uses only `takedown_core::` paths and needs nothing but proptest.
#![allow(dead_code)]

use std::collections::BTreeMap;

use proptest::collection::{btree_set, vec};
use proptest::prelude::*;
use proptest::sample::select;
use takedown_core::control::{
    ControlFrame, ControlTrack, Curve, Curves, GateSpan, HitEvent, Lane, Link, MAX_HIT_MS,
    TrackParts, U7, Velocity,
};
use takedown_core::hit::RoleName;
use takedown_core::ids::{Hash32, TakeId};
use takedown_core::seed::{Aspect, Seed, SeedTree, VoiceSeeds};
use takedown_core::time::{
    MAX_RATE_HZ, MIN_RATE_HZ, SampleRate, TimelinePos, TimelineSpan, frame_start, frames_covering,
};

/// The frozen-values fixture shared by every frozen test.
pub const FROZEN: &str = include_str!("../fixtures/frozen-keys.txt");

/// The rates `std_rate` draws from: 44.1 and 88.2 kHz have non-integer
/// samples per frame; 8 kHz is the coarsest.
pub const STD_RATES: [u32; 10] = [
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 192_000,
];

/// Ten minutes at 44.1 kHz.
pub const TEN_MINUTES_44K: u64 = 26_460_000;

/// A rate known to be valid.
pub fn rate(hz: u32) -> SampleRate {
    SampleRate::new(hz).expect("rate in range")
}

/// The SplitMix64 finaliser, for deterministic expansion of plans.
pub fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Bits of every field, for bit-exact comparison.
pub fn frame_bits(f: &ControlFrame) -> [u32; 6] {
    [
        f.pitch_st.to_bits(),
        f.amp_db.to_bits(),
        f.bright.to_bits(),
        f.breath.to_bits(),
        f.vowel[0].to_bits(),
        f.vowel[1].to_bits(),
    ]
}

/// The common rates.
pub fn std_rate() -> impl Strategy<Value = SampleRate> {
    select(STD_RATES.to_vec()).prop_map(rate)
}

/// Uniform in [1 kHz, 1.536 MHz], with both bounds.
pub fn any_rate() -> impl Strategy<Value = SampleRate> {
    prop_oneof![
        1 => Just(MIN_RATE_HZ),
        1 => Just(MAX_RATE_HZ),
        8 => MIN_RATE_HZ..=MAX_RATE_HZ,
    ]
    .prop_map(rate)
}

/// Positions at `rate`: 0, frame boundaries ±1, up to 2^44, and ten minutes
/// at 44.1 kHz.
pub fn pos(rate: SampleRate) -> impl Strategy<Value = u64> {
    prop_oneof![
        1 => Just(0u64),
        1 => Just(TEN_MINUTES_44K),
        2 => 0u64..10_000,
        3 => 0u64..=(1u64 << 44),
        3 => (0u64..(1 << 30), -1i64..=1)
            .prop_map(move |(k, d)| frame_start(k, rate).saturating_add_signed(d)),
    ]
}

/// One curve value: the range bounds, -0.0, subnormals, and values inside.
/// Pitch adds whole semitones and values near half-semitone edges.
pub fn curve_value(c: Curve) -> BoxedStrategy<f32> {
    let (lo, hi) = c.range();
    let tiny = f32::from_bits(1);
    let mut hard = vec![
        lo,
        hi,
        -0.0,
        0.0,
        tiny,
        f32::MIN_POSITIVE,
        f32::MIN_POSITIVE / 2.0,
    ];
    if lo < 0.0 {
        hard.push(-tiny);
    }
    match c {
        Curve::PitchSt => prop_oneof![
            2 => select(hard),
            3 => lo..=hi,
            3 => (36i32..96).prop_map(|k| k as f32),
            2 => (36i32..96, -0.499f32..0.499).prop_map(|(k, d)| k as f32 + d),
        ]
        .boxed(),
        _ => prop_oneof![2 => select(hard), 5 => lo..=hi].boxed(),
    }
}

/// A compact description of one curve: a few knots repeated in random runs.
#[derive(Clone, Debug)]
pub struct CurvePlan {
    pub knots: Vec<f32>,
    pub run: u64,
    pub salt: u64,
}

impl CurvePlan {
    pub fn fill(&self, frames: u64) -> Vec<f32> {
        let k = self.knots.len() as u64;
        (0..frames)
            .map(|i| self.knots[(splitmix(self.salt ^ (i / self.run)) % k) as usize])
            .collect()
    }
}

pub fn curve_plan(c: Curve) -> impl Strategy<Value = CurvePlan> {
    (vec(curve_value(c), 1..=6), 1u64..=64, any::<u64>()).prop_map(|(knots, run, salt)| CurvePlan {
        knots,
        run,
        salt,
    })
}

/// Six valid curves of `frames` frames.
pub fn curves(frames: u64) -> impl Strategy<Value = Curves> {
    (
        curve_plan(Curve::PitchSt),
        curve_plan(Curve::AmpDb),
        curve_plan(Curve::Bright),
        curve_plan(Curve::Breath),
        curve_plan(Curve::VowelX),
        curve_plan(Curve::VowelY),
    )
        .prop_map(move |(p, a, br, bh, vx, vy)| Curves {
            pitch_st: p.fill(frames),
            amp_db: a.fill(frames),
            bright: br.fill(frames),
            breath: bh.fill(frames),
            vowel_x: vx.fill(frames),
            vowel_y: vy.fill(frames),
        })
}

/// Raw choices for one span, scaled to the track when built.
#[derive(Clone, Debug)]
pub struct SpanPlan {
    pub gap_kind: u8,
    pub dur_kind: u8,
    pub legato: bool,
    pub raw: u64,
}

pub fn span_plan() -> impl Strategy<Value = SpanPlan> {
    (0u8..3, 0u8..3, any::<bool>(), any::<u64>()).prop_map(|(gap_kind, dur_kind, legato, raw)| {
        SpanPlan {
            gap_kind,
            dur_kind,
            legato,
            raw,
        }
    })
}

/// Raw choices for one hit, scaled to the track when built.
#[derive(Clone, Debug)]
pub struct HitPlan {
    pub dur_kind: u8,
    /// Extra hits sharing this hit's anchor: 0 to 2.
    pub share: u8,
    pub raw: u64,
}

pub fn hit_plan() -> impl Strategy<Value = HitPlan> {
    (
        0u8..4,
        prop_oneof![6 => Just(0u8), 1 => Just(1u8), 1 => Just(2u8)],
        any::<u64>(),
    )
        .prop_map(|(dur_kind, share, raw)| HitPlan {
            dur_kind,
            share,
            raw,
        })
}

/// Role names: GM names and names outside the GM map.
pub const ROLE_POOL: [&str; 16] = [
    "kick",
    "snare",
    "hat_closed",
    "hat_open",
    "clap",
    "rim",
    "tom_low",
    "tom_high",
    "crash",
    "perc",
    "zap",
    "glitch_2",
    "sub",
    "breath_noise",
    "x9",
    "r_11",
];

/// Everything a track is built from.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // independent hard-case switches
pub struct TrackPlan {
    pub rate: SampleRate,
    pub len: u64,
    pub curves: Curves,
    pub spans: Vec<SpanPlan>,
    pub hits: Vec<HitPlan>,
    pub n_roles: usize,
    pub salt: u64,
    /// The last span ends at `len`.
    pub span_to_end: bool,
    /// A dense roll at 23 ms spacing with more than 12 overlapping hits.
    pub roll: bool,
    /// Hits at 0 and ending at `len`.
    pub edge_hits: bool,
    pub with_take: bool,
}

/// Lengths: tiny (1 to 400 samples), typical (1 to 4 s), and a sparse ten
/// minutes.
pub fn track_len(rate: SampleRate) -> impl Strategy<Value = u64> {
    let hz = u64::from(rate.hz());
    prop_oneof![
        4 => 1u64..=400,
        5 => hz..=4 * hz,
        1 => Just(600 * hz),
    ]
}

pub fn track_plan(rate: SampleRate) -> impl Strategy<Value = TrackPlan> {
    track_len(rate).prop_flat_map(move |len| {
        let long = len > 60 * u64::from(rate.hz());
        let (max_spans, max_hits) = if long { (6, 24) } else { (12, 16) };
        (
            curves(frames_covering(len, rate)),
            vec(span_plan(), 0..=max_spans),
            vec(hit_plan(), 0..=max_hits),
            0usize..=12,
            any::<u64>(),
            (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()),
        )
            .prop_map(
                move |(
                    curves,
                    spans,
                    hits,
                    n_roles,
                    salt,
                    (span_to_end, roll, edge_hits, with_take),
                )| {
                    TrackPlan {
                        rate,
                        len,
                        curves,
                        spans,
                        hits,
                        n_roles,
                        salt,
                        span_to_end,
                        roll,
                        edge_hits,
                        with_take,
                    }
                },
            )
    })
}

fn velocity(raw: u64) -> Velocity {
    Velocity::new(1 + (raw % 127) as u8).expect("1..=127")
}

fn u7(raw: u64) -> U7 {
    U7::new((raw % 128) as u8).expect("0..=127")
}

impl TrackPlan {
    fn ms(&self, ms: u32) -> u64 {
        self.rate.samples_for_ms(ms).max(1)
    }

    fn build_spans(&self) -> Vec<GateSpan> {
        let len = self.len;
        let mut out: Vec<GateSpan> = Vec::new();
        let mut pos = 0u64;
        for p in &self.spans {
            let r = splitmix(p.raw);
            let gap = match p.gap_kind {
                0 => 0,
                1 => 1 + r % self.ms(40),
                _ => 1 + r % (len / 8 + 1),
            };
            let r2 = splitmix(r);
            let dur = match p.dur_kind {
                // Under 5 ms.
                0 => 1 + r2 % (self.ms(5) - 1).max(1),
                1 => self.ms(5) + r2 % self.ms(200),
                _ => 1 + r2 % (len / 3 + 1),
            };
            let on = pos + gap;
            if on >= len {
                break;
            }
            let off = (on + dur).min(len);
            let link = match out.last() {
                Some(prev) if prev.off.0 == on => {
                    if p.legato {
                        Link::Legato
                    } else {
                        Link::Retrigger
                    }
                }
                _ => Link::Detached,
            };
            let jitter = self.ms(30);
            let anchor = (on + splitmix(r2) % (2 * jitter + 1)).saturating_sub(jitter);
            out.push(GateSpan {
                on: TimelinePos(on),
                off: TimelinePos(off),
                anchor: TimelinePos(anchor.min(len)),
                vel: velocity(r2 >> 7),
                link,
            });
            pos = off;
        }
        if self.span_to_end
            && let Some(last) = out.last_mut()
        {
            last.off = TimelinePos(len);
        }
        out
    }

    fn hit(&self, at: u64, dur: u64, anchor: u64, role: usize, raw: u64) -> HitEvent {
        let at = at.min(self.len - 1);
        HitEvent {
            at: TimelinePos(at),
            anchor: TimelinePos(anchor.min(self.len)),
            dur: dur.clamp(1, self.len - at),
            role: role as u8,
            vel: velocity(raw),
            bright: u7(raw >> 8),
            breath: u7(raw >> 16),
            mouth: u7(raw >> 24),
        }
    }

    fn build_hits(&self, n_roles: usize) -> Vec<HitEvent> {
        if n_roles == 0 {
            return Vec::new();
        }
        let len = self.len;
        let cap = self.rate.samples_for_ms(MAX_HIT_MS);
        let jitter = self.ms(30);
        let mut out = Vec::new();
        for p in &self.hits {
            let r = splitmix(p.raw);
            let at = r % len;
            let r2 = splitmix(r);
            let dur = match p.dur_kind {
                0 => 1,
                1 => 1 + r2 % self.ms(500),
                2 => cap,
                _ => 1 + r2 % cap,
            };
            let anchor = (at + splitmix(r2) % (2 * jitter + 1)).saturating_sub(jitter);
            let role = (r2 >> 32) as usize % n_roles;
            out.push(self.hit(at, dur, anchor, role, r2));
            for j in 1..=u64::from(p.share) {
                let at2 = at + j * self.ms(2);
                out.push(self.hit(at2, dur, anchor, (role + j as usize) % n_roles, r2 >> j));
            }
        }
        let r = splitmix(self.salt);
        if self.roll {
            let start = r % len;
            for j in 0..20 {
                let at = start + j * self.ms(23);
                if at >= len {
                    break;
                }
                out.push(self.hit(at, self.ms(400), at, (j as usize) % n_roles, r >> j));
            }
        }
        if self.edge_hits {
            let d = self.ms(30).min(len);
            out.push(self.hit(0, 1 + r % self.ms(50), 0, 0, r));
            out.push(self.hit(len - d, d, len - d, n_roles - 1, r >> 3));
        }
        out
    }

    fn roles(&self) -> Vec<RoleName> {
        let rot = (self.salt % ROLE_POOL.len() as u64) as usize;
        let mut names: Vec<RoleName> = (0..self.n_roles)
            .map(|i| RoleName::new(ROLE_POOL[(rot + i) % ROLE_POOL.len()]).expect("pool names"))
            .collect();
        names.sort();
        names
    }

    /// Valid, canonical parts.
    pub fn build(&self) -> TrackParts {
        let roles = self.roles();
        let parts = TrackParts {
            rate: self.rate,
            len: self.len,
            take: self
                .with_take
                .then(|| TakeId::from_hash(Hash32([(self.salt & 0xFF) as u8; 32]))),
            curves: self.curves.clone(),
            spans: self.build_spans(),
            hits: self.build_hits(roles.len()),
            roles,
        };
        ControlTrack::new(parts)
            .expect("the generator builds valid tracks")
            .into_parts()
    }
}

/// Valid, canonical TrackParts at `rate`, with every §3 hard case.
pub fn track_parts(rate: SampleRate) -> impl Strategy<Value = TrackParts> {
    track_plan(rate).prop_map(|p| p.build())
}

/// A valid ControlTrack at `rate`.
pub fn track(rate: SampleRate) -> impl Strategy<Value = ControlTrack> {
    track_parts(rate).prop_map(|p| ControlTrack::new(p).expect("valid parts"))
}

/// A valid ControlTrack at a `std_rate`.
pub fn any_track() -> impl Strategy<Value = ControlTrack> {
    std_rate().prop_flat_map(track)
}

pub fn aspect() -> impl Strategy<Value = Aspect> {
    select(Aspect::ALL.to_vec())
}

/// Spans in a small domain, so anchors land inside, on and beside them.
pub fn region_span() -> impl Strategy<Value = TimelineSpan> {
    (0u64..2_000, 0u64..1_000).prop_map(|(s, l)| {
        TimelineSpan::new(TimelinePos(s), TimelinePos(s + l)).expect("start <= end")
    })
}

/// A SeedTree with 0 to 4 region rolls (often overlapping), rolls and locks.
pub fn seed_tree() -> impl Strategy<Value = SeedTree> {
    (
        any::<u64>(),
        vec((aspect(), 1u32..4), 0..5),
        btree_set(aspect(), 0..3),
        vec((aspect(), region_span(), 1u32..4), 0..=4),
    )
        .prop_map(|(root, rolls, locked, regions)| {
            let mut t = SeedTree::new(Seed(root));
            for (a, n) in rolls {
                t.rolls.insert(a, n);
            }
            for (a, span, n) in regions {
                for _ in 0..n {
                    t.roll_region(a, span);
                }
            }
            t.locked = locked;
            t
        })
}

/// Anchors on every region's edges: just before, at the start, the last
/// sample inside, the (exclusive) end and just after.
pub fn region_probe_anchors(t: &SeedTree) -> Vec<TimelinePos> {
    let mut out = vec![TimelinePos(0), TimelinePos(1_500)];
    for r in &t.regions {
        let (s, e) = (r.span.start().0, r.span.end().0);
        for p in [
            s.saturating_sub(1),
            s,
            e.saturating_sub(1),
            e,
            e.saturating_add(1),
        ] {
            out.push(TimelinePos(p));
        }
    }
    out
}

/// One event's identity and seeds: (lane, anchor, ordinal, seeds), with the
/// ordinal counting earlier events of the same (lane, anchor) in canonical
/// order.
pub fn event_seeds(
    track: &ControlTrack,
    tree: &SeedTree,
    voice: &str,
) -> Vec<(Lane, TimelinePos, u32, VoiceSeeds)> {
    let mut counts: BTreeMap<(Lane, TimelinePos), u32> = BTreeMap::new();
    track
        .events()
        .map(|e| {
            let lane = e.kind.lane();
            let n = counts.entry((lane, e.anchor)).or_insert(0);
            let ordinal = *n;
            *n += 1;
            (
                lane,
                e.anchor,
                ordinal,
                tree.trigger_seeds(voice, lane, e.anchor, ordinal),
            )
        })
        .collect()
}

/// Parses `name = value` lines; `#` starts a comment.
pub fn frozen_entries() -> BTreeMap<String, String> {
    FROZEN
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (k, v) = l
                .split_once(" = ")
                .expect("fixture lines are `name = value`");
            (k.to_owned(), v.to_owned())
        })
        .collect()
}

/// Every computed value must equal its pinned value, and every pinned value
/// under `prefix` must still be computed. Prints the lines to pin on failure.
pub fn check_frozen(prefix: &str, computed: &[(String, String)]) {
    let pinned = frozen_entries();
    let mut problems = Vec::new();
    for (k, v) in computed {
        assert!(k.starts_with(prefix), "{k} is outside {prefix}");
        match pinned.get(k) {
            Some(p) if p == v => {}
            Some(p) => problems.push(format!("DRIFT {k}: pinned {p}, now {v}")),
            None => problems.push(format!("UNPINNED {k} = {v}")),
        }
    }
    for k in pinned.keys().filter(|k| k.starts_with(prefix)) {
        if !computed.iter().any(|(c, _)| c == k) {
            problems.push(format!("NO LONGER COMPUTED {k}"));
        }
    }
    assert!(
        problems.is_empty(),
        "frozen values:\n{}",
        problems.join("\n")
    );
}
