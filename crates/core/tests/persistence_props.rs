//! core-types §3.3, the persistence half: interpretation and render keys,
//! `project.json`, migrations, the evidence container, take ids and the
//! project directory. The key encoding and frozen keys are `key_props.rs`.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use common::{ROLE_POOL, STD_RATES, check_frozen, rate, seed_tree, splitmix, std_rate};
use proptest::collection::{btree_map, btree_set, vec};
use proptest::option;
use proptest::prelude::*;
use proptest::sample::select;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use takedown_core::canonical::{MidiBinding, MpeProfile};
use takedown_core::control::Velocity;
use takedown_core::edit::{Articulation, Edit, EditEntry};
use takedown_core::evidence::{
    CANONICAL_NAN_BITS, CONTAINER_VERSION, Column, Evidence, EvidenceError, EvidenceHeader,
    EvidenceResolution, OnsetCandidate, Provenance, resolve_evidence,
};
use takedown_core::hit::{HitAnchor, RoleName};
use takedown_core::ids::{
    ControlId, EditId, EvidenceKey, Hash32, HitSetKey, InterpId, InterpKey, LayerId, MidiFileId,
    PaletteId, ProjectId, TakeId, TeachSetId, WavId, context,
};
use takedown_core::interp::{ControlSource, InterpError, Interpretation};
use takedown_core::key::{KeyError, KeyHash, KeyKind, KeyWriter, key_of};
use takedown_core::layer::{
    BackendStamp, Layer, LayerStatus, PlatformClass, RenderInputs, RenderRef, StaleReason,
};
use takedown_core::lens::{LensError, LensSettings, PaletteRef, Slider, Sliders};
use takedown_core::migrate::{self, MigrationError, Step, chain_is_total};
use takedown_core::musical::{TempoMap, TempoSegment, TimeMap};
use takedown_core::params::{ParamValue, Params};
use takedown_core::project::{
    PaletteMeta, Project, ProjectError, SCHEMA_VERSION, TakeAdded, json, paths,
};
use takedown_core::seed::{Aspect, Seed, SeedTree};
use takedown_core::take::{AudioInfo, CaptureSource, HostAnchor, Take, TakeError, TakeHasher};
use takedown_core::time::{SampleRate, TimelinePos, TimelineSpan, frames_covering};

/// The frozen schema-1 project fixture. It loads forever, with frozen keys.
const PROJECT_V1: &str = include_str!("fixtures/project.v1.json");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "takedown-core-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// The names of the entries directly inside, sorted.
    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Deterministic PCM in [-1, 1).
fn pcm(n: usize, salt: u64) -> Vec<f32> {
    (0..n)
        .map(|i| (splitmix(salt ^ i as u64) >> 40) as f32 / 8_388_608.0 - 1.0)
        .collect()
}

fn take_id_of(rate: SampleRate, channels: u16, samples: &[f32]) -> (TakeId, u64) {
    let mut h = TakeHasher::new(rate, channels);
    h.update(samples).unwrap();
    h.finish().unwrap()
}

fn take_of(rate: SampleRate, channels: u16, samples: &[f32], name: &str) -> Take {
    let (id, frames) = take_id_of(rate, channels, samples);
    Take {
        id,
        audio: AudioInfo {
            rate,
            channels,
            frames,
        },
        source: CaptureSource::Recorded {
            device: "Scarlett 2i2".to_owned(),
            latency_compensated: true,
        },
        anchor: None,
        name: name.to_owned(),
        captured_at: "2026-10-21T09:30:00Z".to_owned(),
    }
}

fn span(start: u64, end: u64) -> TimelineSpan {
    TimelineSpan::new(TimelinePos(start), TimelinePos(end)).unwrap()
}

fn role(name: &str) -> RoleName {
    RoleName::new(name).unwrap()
}

/// What the running build would record for a fresh render of `interp`.
fn current_inputs(interp: &Interpretation, rate: SampleRate) -> RenderInputs {
    RenderInputs {
        interp: interp.key().unwrap(),
        evidence: Some(Hash32([7; 32])),
        hits: Some(Hash32([8; 32])),
        lens_version: 1,
        backends: vec![BackendStamp {
            id: "plaits".to_owned(),
            version: 1,
        }],
        contract: 1,
        rate,
    }
}

fn render_ref(inputs: RenderInputs, wav_bytes: &[u8]) -> RenderRef {
    RenderRef {
        key: inputs.key().unwrap(),
        inputs,
        wav: WavId::of_bytes(wav_bytes),
        frames: 4_800,
        platform: PlatformClass {
            arch: "aarch64".to_owned(),
            os: "macos".to_owned(),
        },
    }
}

/// A small valid project at 48 kHz: one take, one palette, one Lens
/// interpretation with edits, and one layer with a fresh 48 kHz render.
/// `timing_feel` is 0.25 and `glide_retrigger` 0.0, which the migration
/// scenarios rely on.
fn scenario_project() -> Project {
    let r48 = rate(48_000);
    let mut p = Project::new(ProjectId::from_hash(Hash32([9; 32])), r48);
    let take = take_of(r48, 1, &pcm(4_800, 1), "beatbox");
    let take_id = take.id;
    assert_eq!(p.add_take(take), TakeAdded::New);
    let palette = PaletteId::of_bytes(br#"{"voices":[]}"#);
    p.palettes.insert(
        palette,
        PaletteMeta {
            name: "grit".to_owned(),
        },
    );
    let id = InterpId(p.alloc_id());
    let mut edits = Vec::new();
    for edit in [
        Edit::ForcePitch {
            span: span(480, 960),
            st: 62.5,
        },
        Edit::SplitAt {
            at: TimelinePos(2_400),
            articulation: Articulation::Legato,
        },
        Edit::AddHit {
            hit: HitAnchor {
                at: TimelinePos(3_000),
                tol: 1_200,
            },
            dur: 2_400,
            role: role("kick"),
            vel: Velocity::new(100).unwrap(),
        },
    ] {
        edits.push(EditEntry {
            id: EditId(p.alloc_id()),
            edit,
        });
    }
    let mut seeds = SeedTree::new(Seed(0xFEED));
    seeds.rolls.insert(Aspect::Body, 2);
    seeds.roll_region(Aspect::Mangle, span(1_000, 2_000));
    let interp = Interpretation {
        id,
        name: "take 1".to_owned(),
        take: Some(take_id),
        source: ControlSource::Lens {
            evidence: EvidenceKey::from_hash(Hash32([5; 32])),
            hits: Some(HitSetKey::from_hash(Hash32([6; 32]))),
            edits,
        },
        lens: LensSettings {
            palette: PaletteRef {
                name: "grit".to_owned(),
                id: palette,
            },
            sliders: Sliders {
                mouth_amount: 0.5,
                fidelity: 1.0,
                mangle: 0.3,
                timing_feel: 0.25,
                register_st: -12,
                glide_retrigger: 0.0,
            },
            locks: [Slider::Mangle].into_iter().collect(),
        },
        seeds,
        midi: MidiBinding::default_export(),
    };
    let inputs = current_inputs(&interp, r48);
    p.interpretations.push(interp);
    let mut layer = Layer {
        id: LayerId(p.alloc_id()),
        interp: id,
        gain_db: -3.0,
        mute: false,
        renders: BTreeMap::new(),
    };
    layer.store_render(render_ref(inputs, b"render at 48 kHz, lens 1"));
    p.layers.push(layer);
    p.validate().unwrap();
    p
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn hash32() -> impl Strategy<Value = Hash32> {
    any::<[u8; 32]>().prop_map(Hash32)
}

/// Any finite f32 by bit pattern, plus -0.0, subnormals and the extremes.
fn finite_f32() -> BoxedStrategy<f32> {
    prop_oneof![
        1 => select(vec![
            0.0f32,
            -0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            f32::MIN_POSITIVE,
            f32::MIN_POSITIVE / 2.0,
            f32::MAX,
            f32::MIN,
            0.1,
            1.0 / 3.0,
        ]),
        4 => any::<u32>()
            .prop_map(f32::from_bits)
            .prop_filter("finite", |x| x.is_finite()),
    ]
    .boxed()
}

/// Any finite f64 by bit pattern, plus the hard cases.
fn finite_f64() -> BoxedStrategy<f64> {
    prop_oneof![
        1 => select(vec![
            0.0f64,
            -0.0,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            f64::MAX,
            0.1,
            120.0,
            1.0 / 3.0,
        ]),
        4 => any::<u64>()
            .prop_map(f64::from_bits)
            .prop_filter("finite", |x| x.is_finite()),
    ]
    .boxed()
}

/// A value in `[lo, hi]`: the bounds, -0.0 and subnormals when inside, and
/// values between.
fn ranged(lo: f32, hi: f32) -> BoxedStrategy<f32> {
    let mut hard = vec![
        lo,
        hi,
        0.0,
        -0.0,
        f32::from_bits(1),
        f32::MIN_POSITIVE / 2.0,
    ];
    hard.retain(|v| (lo..=hi).contains(v));
    prop_oneof![2 => select(hard), 3 => lo..=hi].boxed()
}

fn sliders() -> impl Strategy<Value = Sliders> {
    (
        ranged(0.0, 1.0),
        ranged(0.0, 1.0),
        ranged(0.0, 1.0),
        ranged(0.0, 1.0),
        -36i8..=36,
        ranged(-1.0, 1.0),
    )
        .prop_map(
            |(mouth_amount, fidelity, mangle, timing_feel, register_st, glide_retrigger)| Sliders {
                mouth_amount,
                fidelity,
                mangle,
                timing_feel,
                register_st,
                glide_retrigger,
            },
        )
}

const SLIDERS: [Slider; 6] = [
    Slider::MouthAmount,
    Slider::Fidelity,
    Slider::Mangle,
    Slider::TimingFeel,
    Slider::Register,
    Slider::GlideRetrigger,
];

fn time_span() -> impl Strategy<Value = TimelineSpan> {
    (0u64..10_000_000, 0u64..500_000).prop_map(|(s, l)| span(s, s + l))
}

fn hit_anchor() -> impl Strategy<Value = HitAnchor> {
    (0u64..10_000_000, 0u32..5_000).prop_map(|(at, tol)| HitAnchor {
        at: TimelinePos(at),
        tol,
    })
}

fn role_name() -> impl Strategy<Value = RoleName> {
    select(ROLE_POOL.to_vec()).prop_map(role)
}

fn velocity() -> impl Strategy<Value = Velocity> {
    (1u8..=127).prop_map(|v| Velocity::new(v).unwrap())
}

/// One edit of kind `kind` (0 to 6).
fn edit_of_kind(kind: u8) -> BoxedStrategy<Edit> {
    match kind {
        0 => (time_span(), ranged(-64.0, 192.0))
            .prop_map(|(span, st)| Edit::ForcePitch { span, st })
            .boxed(),
        1 => (0u64..10_000_000, any::<bool>())
            .prop_map(|(at, legato)| Edit::SplitAt {
                at: TimelinePos(at),
                articulation: if legato {
                    Articulation::Legato
                } else {
                    Articulation::Retrigger
                },
            })
            .boxed(),
        2 => (0u64..10_000_000, 0u32..5_000)
            .prop_map(|(at, tol)| Edit::JoinAt {
                at: TimelinePos(at),
                tol,
            })
            .boxed(),
        3 => (hit_anchor(), role_name())
            .prop_map(|(hit, role)| Edit::ForceRole { hit, role })
            .boxed(),
        4 => hit_anchor().prop_map(|hit| Edit::DeleteHit { hit }).boxed(),
        5 => (hit_anchor(), 1u64..2_880_000, role_name(), velocity())
            .prop_map(|(hit, dur, role, vel)| Edit::AddHit {
                hit,
                dur,
                role,
                vel,
            })
            .boxed(),
        _ => time_span().prop_map(|span| Edit::MuteSpan { span }).boxed(),
    }
}

/// Every edit kind once, then up to three more of any kind.
fn edits() -> impl Strategy<Value = Vec<Edit>> {
    (
        (0..7u8).map(edit_of_kind).collect::<Vec<_>>(),
        vec((0u8..7).prop_flat_map(edit_of_kind), 0..=3),
    )
        .prop_map(|(mut all, extra)| {
            all.extend(extra);
            all
        })
}

fn midi_binding() -> impl Strategy<Value = MidiBinding> {
    prop_oneof![
        Just(MidiBinding::default_export()),
        (
            1u8..=96,
            24u16..=1920,
            1u64..=3,
            any::<u64>(),
            0u64..100_000
        )
            .prop_map(|(bend, ppq, n, salt, origin)| {
                let segments: Vec<TempoSegment> = (0..n)
                    .map(|i| TempoSegment {
                        tick: i * 7_680,
                        us_per_quarter: 200_000 + (splitmix(salt ^ i) % 1_800_000) as u32,
                    })
                    .collect();
                let profile = MpeProfile {
                    bend_range_st: bend,
                    ..MpeProfile::default()
                };
                MidiBinding {
                    profile,
                    time: TimeMap::new(ppq, TempoMap::try_from(segments).unwrap(), origin).unwrap(),
                }
            }),
    ]
}

fn host_anchor() -> impl Strategy<Value = HostAnchor> {
    (
        std_rate(),
        any::<i64>(),
        finite_f64(),
        finite_f64(),
        any::<(u8, u8)>(),
    )
        .prop_map(
            |(host_rate, host_sample_at_start, ppq_at_start, tempo_bpm, time_sig)| HostAnchor {
                host_rate,
                host_sample_at_start,
                ppq_at_start,
                tempo_bpm,
                time_sig,
            },
        )
}

fn take() -> impl Strategy<Value = Take> {
    (
        hash32(),
        std_rate(),
        1u16..=2,
        0u64..(1 << 40),
        any::<bool>(),
        option::of(host_anchor()),
        "\\PC{0,12}",
        "\\PC{0,8}",
        "\\PC{0,8}",
    )
        .prop_map(
            |(id, rate, channels, frames, recorded, anchor, name, a, b)| {
                let source = if recorded {
                    CaptureSource::Recorded {
                        device: a,
                        latency_compensated: b.len() % 2 == 0,
                    }
                } else {
                    CaptureSource::Dropped {
                        original_name: a,
                        source_format: b,
                    }
                };
                Take {
                    id: TakeId::from_hash(id),
                    audio: AudioInfo {
                        rate,
                        channels,
                        frames,
                    },
                    source,
                    anchor,
                    name,
                    captured_at: "2026-10-21T12:00:00+02:00".to_owned(),
                }
            },
        )
}

#[derive(Clone, Debug)]
struct LayerPlan {
    gain_db: f32,
    mute: bool,
    /// Some: the renders were keyed by another interpretation key (stale).
    stale: Option<Hash32>,
    evidence: Option<Hash32>,
    hits: Option<Hash32>,
    lens_version: u32,
    backends: BTreeSet<(String, u32)>,
    contract: u32,
    wavs: (Hash32, Hash32),
    frames: u64,
}

fn layer_plan() -> impl Strategy<Value = LayerPlan> {
    (
        finite_f32(),
        any::<bool>(),
        option::of(hash32()),
        option::of(hash32()),
        option::of(hash32()),
        1u32..5,
        btree_set(
            (
                select(vec!["plaits", "toy", "tract"]).prop_map(String::from),
                1u32..4,
            ),
            1..=3,
        ),
        1u32..3,
        (hash32(), hash32()),
        0u64..(1 << 36),
    )
        .prop_map(
            |(
                gain_db,
                mute,
                stale,
                evidence,
                hits,
                lens_version,
                backends,
                contract,
                wavs,
                frames,
            )| {
                LayerPlan {
                    gain_db,
                    mute,
                    stale,
                    evidence,
                    hits,
                    lens_version,
                    backends,
                    contract,
                    wavs,
                    frames,
                }
            },
        )
}

#[derive(Clone, Debug)]
struct InterpPlan {
    name: String,
    take: usize,
    /// Some((control, file, with a take)): an Imported source.
    imported: Option<(Hash32, Option<Hash32>, bool)>,
    evidence: Hash32,
    hits: Option<Hash32>,
    edits: Vec<Edit>,
    palette: usize,
    sliders: Sliders,
    locks: BTreeSet<Slider>,
    seeds: SeedTree,
    midi: MidiBinding,
    layer: Option<LayerPlan>,
}

fn interp_plan() -> impl Strategy<Value = InterpPlan> {
    (
        (
            "\\PC{0,12}",
            any::<usize>(),
            option::weighted(0.3, (hash32(), option::of(hash32()), any::<bool>())),
            hash32(),
            option::of(hash32()),
        ),
        (
            edits(),
            any::<usize>(),
            sliders(),
            btree_set(select(SLIDERS.to_vec()), 0..3),
            seed_tree(),
            midi_binding(),
        ),
        option::weighted(0.8, layer_plan()),
    )
        .prop_map(
            |(
                (name, take, imported, evidence, hits),
                (edits, palette, sliders, locks, seeds, midi),
                layer,
            )| InterpPlan {
                name,
                take,
                imported,
                evidence,
                hits,
                edits,
                palette,
                sliders,
                locks,
                seeds,
                midi,
                layer,
            },
        )
}

/// A valid Project holding every float-typed persisted field (sliders, a
/// ForcePitch, layer gain, HostAnchor's f64s), renders at two rates, and every
/// edit kind. A project holds no Params; those are covered by
/// `f32_survives_json` and the evidence tests.
fn project() -> impl Strategy<Value = Project> {
    (
        hash32(),
        0usize..STD_RATES.len(),
        1usize..STD_RATES.len(),
        vec(take(), 1..=3),
        vec((hash32(), "\\PC{0,10}"), 1..=2),
        option::of(hash32()),
        vec(interp_plan(), 1..=3),
    )
        .prop_map(|(id, out, shift, takes, palettes, teach, interps)| {
            build_project(id, out, shift, takes, &palettes, teach, interps)
        })
}

fn build_project(
    id: Hash32,
    out: usize,
    shift: usize,
    takes: Vec<Take>,
    palettes: &[(Hash32, String)],
    teach: Option<Hash32>,
    interps: Vec<InterpPlan>,
) -> Project {
    let out_rate = rate(STD_RATES[out]);
    let second_rate = rate(STD_RATES[(out + shift) % STD_RATES.len()]);
    let mut p = Project::new(ProjectId::from_hash(id), out_rate);
    for t in takes {
        p.add_take(t);
    }
    let take_ids: Vec<TakeId> = p.takes.keys().copied().collect();
    let palette_refs: Vec<PaletteRef> = palettes
        .iter()
        .map(|(h, name)| {
            let id = PaletteId::from_hash(*h);
            p.palettes.insert(id, PaletteMeta { name: name.clone() });
            PaletteRef {
                name: format!("{name} (display)"),
                id,
            }
        })
        .collect();
    p.teach_set = teach.map(TeachSetId::from_hash);
    for plan in interps {
        let id = InterpId(p.alloc_id());
        let take_id = take_ids[plan.take % take_ids.len()];
        let (take, source) = if let Some((control, file, with_take)) = plan.imported {
            (
                with_take.then_some(take_id),
                ControlSource::Imported {
                    control: ControlId::from_hash(control),
                    file: file.map(MidiFileId::from_hash),
                },
            )
        } else {
            let edits = plan
                .edits
                .into_iter()
                .map(|edit| EditEntry {
                    id: EditId(p.alloc_id()),
                    edit,
                })
                .collect();
            (
                Some(take_id),
                ControlSource::Lens {
                    evidence: EvidenceKey::from_hash(plan.evidence),
                    hits: plan.hits.map(HitSetKey::from_hash),
                    edits,
                },
            )
        };
        let interp = Interpretation {
            id,
            name: plan.name,
            take,
            source,
            lens: LensSettings {
                palette: palette_refs[plan.palette % palette_refs.len()].clone(),
                sliders: plan.sliders,
                locks: plan.locks,
            },
            seeds: plan.seeds,
            midi: plan.midi,
        };
        let key = interp.key().unwrap();
        p.interpretations.push(interp);
        if let Some(lp) = plan.layer {
            let layer = build_layer(LayerId(p.alloc_id()), id, key, &lp, [out_rate, second_rate]);
            p.layers.push(layer);
        }
    }
    p.validate().expect("the generator builds valid projects");
    p
}

/// A layer of interpretation `interp` (whose current key is `key`) with a
/// render at each of `rates`.
fn build_layer(
    id: LayerId,
    interp: InterpId,
    key: InterpKey,
    lp: &LayerPlan,
    rates: [SampleRate; 2],
) -> Layer {
    let mut layer = Layer {
        id,
        interp,
        gain_db: lp.gain_db,
        mute: lp.mute,
        renders: BTreeMap::new(),
    };
    for (r, wav) in rates.into_iter().zip([lp.wavs.0, lp.wavs.1]) {
        let inputs = RenderInputs {
            interp: lp.stale.map_or(key, InterpKey::from_hash),
            evidence: lp.evidence,
            hits: lp.hits,
            lens_version: lp.lens_version,
            backends: lp
                .backends
                .iter()
                .map(|(id, version)| BackendStamp {
                    id: id.clone(),
                    version: *version,
                })
                .collect(),
            contract: lp.contract,
            rate: r,
        };
        layer.store_render(RenderRef {
            key: inputs.key().unwrap(),
            inputs,
            wav: WavId::from_hash(wav),
            frames: lp.frames,
            platform: PlatformClass {
                arch: "x86_64".to_owned(),
                os: "windows".to_owned(),
            },
        });
    }
    layer
}

fn param_value() -> impl Strategy<Value = ParamValue> {
    prop_oneof![
        any::<bool>().prop_map(ParamValue::Bool),
        any::<i64>().prop_map(ParamValue::Int),
        finite_f32().prop_map(ParamValue::Float),
        "\\PC{0,8}".prop_map(ParamValue::Text),
    ]
}

fn provenance() -> impl Strategy<Value = Provenance> {
    (
        "[a-z-]{1,12}",
        0u32..5,
        btree_map("[a-z]{1,6}", hash32(), 0..3),
        btree_map("[a-z_]{1,8}", param_value(), 0..5),
    )
        .prop_map(|(analyzer, version, models, params)| Provenance {
            analyzer,
            version,
            models,
            params,
        })
}

/// One evidence value: -0.0, subnormals, any finite pattern, and NaN with a
/// payload in the columns that allow it.
fn evidence_value(c: Column, r: u64) -> f32 {
    match r % 8 {
        0 => -0.0,
        1 => f32::from_bits(1 + (r >> 40) as u32 % 0x7F_FFFF),
        2 if c.allows_nan() => f32::from_bits(0x7FC0_0000 | ((r >> 8) as u32 & 0x3F_FFFF)),
        _ => {
            let v = f32::from_bits((r >> 16) as u32);
            if v.is_finite() { v } else { 0.5 }
        }
    }
}

fn evidence() -> impl Strategy<Value = Evidence> {
    (
        provenance(),
        hash32(),
        std_rate(),
        prop_oneof![3 => 0u64..2_000, 1 => 0u64..50_000],
        btree_set(select(Column::ALL.to_vec()), 0..=Column::ALL.len()),
        any::<u64>(),
        vec((any::<u64>(), finite_f32(), finite_f32()), 0..8),
    )
        .prop_map(
            |(provenance, take, take_rate, take_frames, cols, salt, raw)| {
                let take = TakeId::from_hash(take);
                let n = frames_covering(take_frames, take_rate);
                let columns = cols
                    .into_iter()
                    .map(|c| {
                        let values = (0..n)
                            .map(|i| evidence_value(c, splitmix(salt ^ i ^ ((c as u64) << 40))))
                            .collect();
                        (c, values)
                    })
                    .collect();
                let mut onsets: Vec<OnsetCandidate> = if take_frames == 0 {
                    Vec::new()
                } else {
                    raw.into_iter()
                        .map(|(at, strength, rise_db)| OnsetCandidate {
                            at: TimelinePos(at % take_frames),
                            strength,
                            rise_db,
                        })
                        .collect()
                };
                onsets.sort_by_key(|o| o.at);
                let header = EvidenceHeader {
                    key: provenance.key(take).unwrap(),
                    take,
                    take_rate,
                    take_frames,
                    provenance,
                };
                Evidence::new(header, columns, onsets).unwrap()
            },
        )
}

fn render_inputs() -> impl Strategy<Value = RenderInputs> {
    (
        hash32(),
        option::of(hash32()),
        option::of(hash32()),
        1u32..100,
        btree_set(
            (
                select(vec!["plaits", "toy", "tract"]).prop_map(String::from),
                1u32..4,
            ),
            0..=3,
        ),
        1u32..100,
        std_rate(),
    )
        .prop_map(
            |(interp, evidence, hits, lens_version, backends, contract, rate)| RenderInputs {
                interp: InterpKey::from_hash(interp),
                evidence,
                hits,
                lens_version,
                backends: backends
                    .into_iter()
                    .map(|(id, version)| BackendStamp { id, version })
                    .collect(),
                contract,
                rate,
            },
        )
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

fn other_unit(v: f32) -> f32 {
    if v.to_bits() == 0.5f32.to_bits() {
        0.25
    } else {
        0.5
    }
}

fn flip(h: Hash32) -> Hash32 {
    let mut b = h.0;
    b[0] ^= 1;
    Hash32(b)
}

type Mutation = (&'static str, Box<dyn Fn(&mut Interpretation)>);

fn m(name: &'static str, f: impl Fn(&mut Interpretation) + 'static) -> Mutation {
    (name, Box::new(f))
}

proptest! {
    #[test]
    fn keys_ignore_identity_and_locks(p in project(), bend in 1u8..=96) {
        let other_take = TakeId::from_hash(Hash32([0xEE; 32]));
        for interp in &p.interpretations {
            let k = interp.key().unwrap();
            let lens = matches!(interp.source, ControlSource::Lens { .. });
            let mut same: Vec<Mutation> = vec![
                m("id", |i| i.id = InterpId(i.id.0 ^ 0xFFFF)),
                m("name", |i| i.name.push_str(" (copy)")),
                m("slider locks", |i| {
                    i.lens.locks = SLIDERS.iter().copied().filter(|s| !i.lens.locks.contains(s)).collect();
                }),
                m("aspect locks", |i| {
                    i.seeds.locked = Aspect::ALL.iter().copied().filter(|a| !i.seeds.locked.contains(a)).collect();
                }),
                m("palette name", |i| i.lens.palette.name.push('!')),
                m("midi binding", move |i| {
                    i.midi.profile.bend_range_st = if i.midi.profile.bend_range_st == bend { bend % 96 + 1 } else { bend };
                    i.midi.time = TimeMap::new(97, TempoMap::constant(431_000).unwrap(), 5).unwrap();
                }),
            ];
            let mut changed: Vec<Mutation> = vec![
                m("palette id", |i| i.lens.palette.id = PaletteId::from_hash(flip(i.lens.palette.id.hash32()))),
                m("seed root", |i| i.seeds.root = Seed(i.seeds.root.0 ^ 1)),
                m("non-zero roll", |i| {
                    let n = i.seeds.rolls.get(&Aspect::PitchReading).copied().unwrap_or(0);
                    i.seeds.rolls.insert(Aspect::PitchReading, n + 1);
                }),
                m("region roll", |i| {
                    i.seeds.locked.clear();
                    i.seeds.roll_region(Aspect::Body, span(123, 4_567));
                }),
                m("mangle", |i| i.lens.sliders.mangle = other_unit(i.lens.sliders.mangle)),
            ];
            let shaping: Vec<Mutation> = vec![
                m("mouth amount", |i| i.lens.sliders.mouth_amount = other_unit(i.lens.sliders.mouth_amount)),
                m("fidelity", |i| i.lens.sliders.fidelity = other_unit(i.lens.sliders.fidelity)),
                m("timing feel", |i| i.lens.sliders.timing_feel = other_unit(i.lens.sliders.timing_feel)),
                m("register", |i| i.lens.sliders.register_st = if i.lens.sliders.register_st == 7 { 8 } else { 7 }),
                m("glide/retrigger", |i| i.lens.sliders.glide_retrigger = other_unit(i.lens.sliders.glide_retrigger)),
            ];
            if lens {
                changed.extend(shaping);
                same.push(m("edit ids", |i| {
                    if let ControlSource::Lens { edits, .. } = &mut i.source {
                        for e in edits {
                            e.id = EditId(e.id.0 + 10_000);
                        }
                    }
                }));
                changed.extend::<Vec<Mutation>>(vec![
                    m("take", move |i| i.take = Some(other_take)),
                    m("evidence key", |i| {
                        if let ControlSource::Lens { evidence, .. } = &mut i.source {
                            *evidence = EvidenceKey::from_hash(flip(evidence.hash32()));
                        }
                    }),
                    m("hits key", |i| {
                        if let ControlSource::Lens { hits, .. } = &mut i.source {
                            *hits = match hits {
                                Some(_) => None,
                                None => Some(HitSetKey::from_hash(Hash32([1; 32]))),
                            };
                        }
                    }),
                    m("edit op", |i| {
                        if let ControlSource::Lens { edits, .. } = &mut i.source {
                            edits[0].edit = Edit::DeleteHit { hit: HitAnchor { at: TimelinePos(u64::MAX), tol: 0 } };
                        }
                    }),
                    m("edit order", |i| {
                        if let ControlSource::Lens { edits, .. } = &mut i.source {
                            edits.swap(0, 1);
                        }
                    }),
                    m("edit added", |i| {
                        if let ControlSource::Lens { edits, .. } = &mut i.source {
                            edits.push(EditEntry { id: EditId(99_999), edit: Edit::MuteSpan { span: span(1, 2) } });
                        }
                    }),
                ]);
            } else {
                same.extend(shaping);
                same.push(m("file id", |i| {
                    if let ControlSource::Imported { file, .. } = &mut i.source {
                        *file = match file {
                            Some(_) => None,
                            None => Some(MidiFileId::from_hash(Hash32([2; 32]))),
                        };
                    }
                }));
                changed.extend::<Vec<Mutation>>(vec![
                    m("take", move |i| i.take = match i.take {
                        Some(_) => None,
                        None => Some(other_take),
                    }),
                    m("control id", |i| {
                        if let ControlSource::Imported { control, .. } = &mut i.source {
                            *control = ControlId::from_hash(flip(control.hash32()));
                        }
                    }),
                ]);
            }
            for (what, f) in &same {
                let mut i = interp.clone();
                f(&mut i);
                prop_assert_eq!(i.key().unwrap(), k, "{} changed the key", what);
            }
            for (what, f) in &changed {
                let mut i = interp.clone();
                f(&mut i);
                prop_assert!(i != *interp, "{} changed nothing", what);
                prop_assert_ne!(i.key().unwrap(), k, "{} kept the key", what);
            }
        }
    }
}

/// The reproducing value of a test-only field added at "v2".
const WOBBLE_REPRODUCING: f32 = 0.0;

/// LensSettings as a v2 build would have it: one extra field, keyed with
/// `unless` at its frozen reproducing value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LensSettingsV2 {
    palette: PaletteRef,
    sliders: Sliders,
    locks: BTreeSet<Slider>,
    wobble: f32,
}

impl KeyHash for LensSettingsV2 {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        let Self {
            palette,
            sliders,
            locks: _,
            wobble,
        } = self;
        w.record(|r| {
            r.field("palette", palette)?;
            r.field("sliders", sliders)?;
            r.unless("wobble", wobble, &WOBBLE_REPRODUCING)
        })
    }
}

impl LensSettingsV2 {
    fn from_v1(v1: LensSettings, wobble: f32) -> Self {
        let LensSettings {
            palette,
            sliders,
            locks,
        } = v1;
        Self {
            palette,
            sliders,
            locks,
            wobble,
        }
    }
}

fn lens_settings() -> impl Strategy<Value = LensSettings> {
    (
        hash32(),
        "\\PC{0,8}",
        sliders(),
        btree_set(select(SLIDERS.to_vec()), 0..3),
    )
        .prop_map(|(id, name, sliders, locks)| LensSettings {
            palette: PaletteRef {
                name,
                id: PaletteId::from_hash(id),
            },
            sliders,
            locks,
        })
}

proptest! {
    #[test]
    fn reproducing_values_keep_keys(lens in lens_settings(), wobble in finite_f32()) {
        let v1 = key_of(KeyKind::Interp, &lens).unwrap();
        for reproducing in [0.0f32, -0.0] {
            let v2 = LensSettingsV2::from_v1(lens.clone(), reproducing);
            prop_assert_eq!(key_of(KeyKind::Interp, &v2).unwrap(), v1);
        }
        let v2 = LensSettingsV2::from_v1(lens, wobble);
        let same = key_of(KeyKind::Interp, &v2).unwrap() == v1;
        prop_assert_eq!(same, wobble == WOBBLE_REPRODUCING);
    }

    #[test]
    fn json_round_trip_preserves_keys(p in project(), prov in provenance(), take in hash32()) {
        let loaded = Project::from_json(&p.to_json()).unwrap();
        prop_assert_eq!(loaded.migrated_from, None);
        let q = loaded.project;
        prop_assert_eq!(&q, &p);
        for (a, b) in p.interpretations.iter().zip(&q.interpretations) {
            prop_assert_eq!(a.key().unwrap(), b.key().unwrap());
        }
        for (la, lb) in p.layers.iter().zip(&q.layers) {
            for (ra, rb) in la.renders.values().zip(lb.renders.values()) {
                prop_assert_eq!(ra.inputs.key().unwrap(), rb.inputs.key().unwrap());
                prop_assert_eq!(rb.key, rb.inputs.key().unwrap());
            }
        }
        // The evidence key survives its header's JSON, params included.
        let take = TakeId::from_hash(take);
        let header = EvidenceHeader {
            key: prov.key(take).unwrap(),
            take,
            take_rate: rate(44_100),
            take_frames: 1_000,
            provenance: prov,
        };
        let back: EvidenceHeader = serde_json::from_slice(&serde_json::to_vec(&header).unwrap()).unwrap();
        prop_assert_eq!(back.provenance.key(take).unwrap(), header.key);
        prop_assert_eq!(&back, &header);
    }

    #[test]
    fn project_round_trip_is_byte_stable(p in project()) {
        let first = p.to_json();
        let second = Project::from_json(&first).unwrap().project.to_json();
        prop_assert!(first == second, "to_json, from_json, to_json changed the bytes");
        // And through the directory: save, load, save.
        let dir = TempDir::new("byte-stable");
        p.save(dir.path(), None).unwrap();
        let on_disk = fs::read(dir.path().join(paths::PROJECT)).unwrap();
        prop_assert!(on_disk == first);
        let reloaded = Project::from_json(&on_disk).unwrap().project;
        reloaded.save(dir.path(), None).unwrap();
        prop_assert!(fs::read(dir.path().join(paths::PROJECT)).unwrap() == on_disk);
        prop_assert_eq!(dir.entries(), vec![paths::PROJECT.to_owned()]);
    }
}

// ---------------------------------------------------------------------------
// f32 and JSON
// ---------------------------------------------------------------------------

/// The two f32 patterns whose shortest decimal (±7.038531e-26) parses to the
/// f64 exactly midway between two f32s, so that reading it through an f64
/// rounds twice and lands on the neighbour. The exhaustive test checks that
/// these are the only ones, which is why `project::json` special-cases them.
const MIDPOINT_F32S: [u32; 2] = [0x15AE_43FD, 0x95AE_43FD];

fn read_param(text: &[u8]) -> Result<u32, String> {
    match serde_json::from_slice::<Params>(text).unwrap().get("x") {
        Some(ParamValue::Float(f)) => Ok(f.to_bits()),
        other => Err(format!("Params read back {other:?}")),
    }
}

/// Checks one bit pattern on every JSON path a persisted f32 takes, as
/// written by `project::json` (what `to_json` and `.tkev` headers use).
/// Non-finite patterns are skipped: keyed and persisted floats are finite.
///
/// - direct: a typed field, as `from_json` reads an unmigrated project.
/// - reprinted: through `serde_json::Value` and back to text, as `from_json`
///   reads a migrated project.
/// - f64 first: `from_value`, which is also how serde reads a number inside
///   an internally tagged enum (`Edit::ForcePitch`).
/// - Params: untagged, so an f64 first as well.
///
/// It also pins why the writer exists: serde_json's own text double-rounds on
/// the f64-first path exactly for [`MIDPOINT_F32S`], and only there does the
/// writer differ from serde_json.
fn f32_round_trips(bits: u32) -> Result<(), String> {
    let v = f32::from_bits(bits);
    if !v.is_finite() {
        return Ok(());
    }
    let midpoint = MIDPOINT_F32S.contains(&bits);
    let plain = serde_json::to_string(&v).unwrap();
    let text = String::from_utf8(json::to_vec(&v).unwrap()).unwrap();
    let plain_f64_first: f32 =
        serde_json::from_value(serde_json::from_str(&plain).unwrap()).unwrap();
    if (plain_f64_first.to_bits() != bits) != midpoint || (text != plain) != midpoint {
        return Err(format!(
            "{bits:#010x}: serde_json writes {plain}, core writes {text}"
        ));
    }
    let direct: f32 = serde_json::from_str(&text).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    let reprinted: f32 = serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
    let f64_first: f32 = serde_json::from_value(value).unwrap();
    let params: Params = [("x".to_owned(), ParamValue::Float(v))]
        .into_iter()
        .collect();
    let in_params =
        read_param(&json::to_vec(&params).unwrap()).map_err(|e| format!("{bits:#010x}: {e}"))?;
    for (path, got) in [
        ("direct", direct.to_bits()),
        ("reprinted", reprinted.to_bits()),
        ("f64 first", f64_first.to_bits()),
        ("Params", in_params),
    ] {
        if got != bits {
            return Err(format!(
                "{bits:#010x} via {path}: {text} read back as {got:#010x}"
            ));
        }
    }
    Ok(())
}

/// An Int param stays an Int, exactly.
fn int_param_round_trips(v: i64) -> Result<(), String> {
    let params: Params = [("n".to_owned(), ParamValue::Int(v))].into_iter().collect();
    let text = serde_json::to_string(&params).unwrap();
    match serde_json::from_str::<Params>(&text).unwrap().get("n") {
        Some(ParamValue::Int(w)) if *w == v => Ok(()),
        other => Err(format!("Int({v}) read back as {other:?}")),
    }
}

#[test]
fn f32_survives_json() {
    let mut failures = Vec::new();
    let hard = [
        0u32,
        0x8000_0000,
        1,
        0x8000_0001,
        0x007F_FFFF,
        0x0080_0000,
        0x7F7F_FFFF,
        0xFF7F_FFFF,
        0x3F80_0000,
        0x3DCC_CCCD,
        MIDPOINT_F32S[0],
        MIDPOINT_F32S[1],
    ];
    let sampled = (0..1_000_000u64).map(|i| (splitmix(i ^ 0x5EED) >> 32) as u32);
    for bits in hard.into_iter().chain(sampled) {
        if let Err(e) = f32_round_trips(bits) {
            failures.push(e);
        }
    }
    for i in 0..10_000u64 {
        let v = splitmix(i) as i64;
        for v in [v, v >> 11, v >> 40, i64::MIN, i64::MAX, 0, -1] {
            if let Err(e) = int_param_round_trips(v) {
                failures.push(e);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

/// Every one of the 2^32 patterns. Minutes on all cores; run with
/// `cargo test --release -- --ignored f32_survives_json_exhaustive`.
#[test]
#[ignore = "exhaustive: all 2^32 patterns"]
fn f32_survives_json_exhaustive() {
    let threads = std::thread::available_parallelism().map_or(4, usize::from) as u64;
    let span = (1u64 << 32) / threads + 1;
    let failures: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let lo = t * span;
                    let hi = ((t + 1) * span).min(1 << 32);
                    (lo..hi)
                        .filter_map(|b| f32_round_trips(b as u32).err())
                        .take(10)
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    assert!(failures.is_empty(), "{failures:?}");
}

/// The double-rounding patterns survive project.json exactly, keys included,
/// both unmigrated and through a migration. Reading floats out of a
/// `serde_json::Value` had turned each into its neighbour.
#[test]
fn midpoint_f32s_survive_project_json() {
    let mut p = scenario_project();
    let [pos, neg] = MIDPOINT_F32S.map(f32::from_bits);
    p.layers[0].gain_db = neg;
    p.interpretations[0].lens.sliders.mouth_amount = pos;
    if let ControlSource::Lens { edits, .. } = &mut p.interpretations[0].source {
        edits[0].edit = Edit::ForcePitch {
            span: span(480, 960),
            st: neg,
        };
    }
    let key = p.interpretations[0].key().unwrap();
    let r48 = rate(48_000);
    p.layers[0].renders.clear();
    p.layers[0].store_render(render_ref(
        current_inputs(&p.interpretations[0], r48),
        b"wav",
    ));
    let bits = |q: &Project| {
        let ControlSource::Lens { edits, .. } = &q.interpretations[0].source else {
            unreachable!()
        };
        let Edit::ForcePitch { st, .. } = edits[0].edit else {
            unreachable!()
        };
        (
            q.layers[0].gain_db.to_bits(),
            q.interpretations[0].lens.sliders.mouth_amount.to_bits(),
            st.to_bits(),
        )
    };
    let want = (MIDPOINT_F32S[1], MIDPOINT_F32S[0], MIDPOINT_F32S[1]);
    let loaded = Project::from_json(&p.to_json()).unwrap().project;
    assert_eq!(bits(&loaded), want);
    assert_eq!(loaded.interpretations[0].key().unwrap(), key);
    let migrated = Project::from_json_with(&as_v1(&p, false), &[ADD_GLIDE], 2)
        .unwrap()
        .project;
    assert_eq!(bits(&migrated), want);
    assert_eq!(migrated.interpretations[0].key().unwrap(), key);
    let current = current_inputs(&migrated.interpretations[0], r48);
    assert_eq!(migrated.layers[0].status(&current), LayerStatus::Fresh);

    // A Float param in a .tkev header keeps its bits, so the key still checks.
    let mut header = small_header(480);
    header
        .provenance
        .params
        .insert("x".to_owned(), ParamValue::Float(pos));
    header.key = header.provenance.key(header.take).unwrap();
    let ev = Evidence::new(header, Vec::new(), Vec::new()).unwrap();
    let back = Evidence::from_container(&ev.to_container(), ev.header().key).unwrap();
    assert_eq!(back, ev);
    assert!(json::double_rounds(pos) && json::double_rounds(neg));
    assert!(!json::double_rounds(0.1) && !json::double_rounds(f32::NAN));
}

// ---------------------------------------------------------------------------
// Strict loading and migrations
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Seg {
    Key(String),
    Index(usize),
}

/// The path to every JSON object in `v`, the root included.
fn object_paths(v: &Value, path: &mut Vec<Seg>, out: &mut Vec<Vec<Seg>>) {
    match v {
        Value::Object(map) => {
            out.push(path.clone());
            for (k, child) in map {
                path.push(Seg::Key(k.clone()));
                object_paths(child, path, out);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                path.push(Seg::Index(i));
                object_paths(child, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

fn at_mut<'a>(mut v: &'a mut Value, path: &[Seg]) -> &'a mut Value {
    for seg in path {
        v = match seg {
            Seg::Key(k) => &mut v[k.as_str()],
            Seg::Index(i) => &mut v[*i],
        };
    }
    v
}

fn to_bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(v).unwrap()
}

/// Adds an unknown field to every object in `bytes`, one at a time, and an
/// unknown kind to every tagged enum; each must refuse to load. Returns the
/// number of objects tried.
fn refuse_unknowns(bytes: &[u8]) -> usize {
    let doc: Value = serde_json::from_slice(bytes).unwrap();
    assert!(Project::from_json(bytes).is_ok());
    let mut paths = Vec::new();
    object_paths(&doc, &mut Vec::new(), &mut paths);
    for path in &paths {
        let mut bad = doc.clone();
        let Value::Object(map) = at_mut(&mut bad, path) else {
            unreachable!("object_paths returns objects")
        };
        map.insert("zz_unknown".to_owned(), Value::Bool(true));
        let tagged = map.contains_key("kind");
        match Project::from_json(&to_bytes(&bad)) {
            Err(ProjectError::Json(_)) => {}
            other => panic!("an unknown field at {path:?} loaded: {other:?}"),
        }
        if tagged {
            let mut bad = doc.clone();
            at_mut(&mut bad, path)["kind"] = Value::from("transpose");
            assert!(
                matches!(
                    Project::from_json(&to_bytes(&bad)),
                    Err(ProjectError::Json(_))
                ),
                "an unknown kind at {path:?} loaded"
            );
        }
    }
    paths.len()
}

#[test]
fn unknown_fields_refused() {
    let tried = refuse_unknowns(PROJECT_V1.as_bytes());
    assert!(tried > 30, "the fixture has only {tried} objects");
    // An unknown edit kind specifically, and an unknown slider lock.
    let doc: Value = serde_json::from_str(PROJECT_V1).unwrap();
    let mut bad = doc.clone();
    bad["interpretations"][0]["source"]["edits"][0]["edit"]["kind"] = Value::from("transpose");
    let err = Project::from_json(&to_bytes(&bad)).unwrap_err();
    assert!(
        matches!(&err, ProjectError::Json(m) if m.contains("transpose")),
        "{err:?}"
    );
    let mut bad = doc;
    bad["interpretations"][0]["lens"]["locks"] = serde_json::json!(["wobble"]);
    assert!(matches!(
        Project::from_json(&to_bytes(&bad)),
        Err(ProjectError::Json(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]
    #[test]
    fn unknown_fields_refused_in_generated_projects(p in project()) {
        refuse_unknowns(&p.to_json());
    }
}

#[test]
fn newer_and_missing_schema_refused() {
    let doc: Value = serde_json::from_str(PROJECT_V1).unwrap();
    let with_version = |v: Value| {
        let mut d = doc.clone();
        d["schema_version"] = v;
        Project::from_json(&to_bytes(&d))
    };
    assert_eq!(
        with_version(Value::from(SCHEMA_VERSION + 1)),
        Err(ProjectError::Migration(MigrationError::TooNew {
            found: SCHEMA_VERSION + 1,
            supported: SCHEMA_VERSION
        }))
    );
    assert_eq!(
        with_version(Value::from(1u64 << 40)),
        Err(ProjectError::Migration(MigrationError::TooNew {
            found: u32::MAX,
            supported: SCHEMA_VERSION
        }))
    );
    for missing in [
        Value::Null,
        Value::from("1"),
        Value::from(-1),
        Value::from(1.5),
    ] {
        assert_eq!(
            with_version(missing),
            Err(ProjectError::Migration(MigrationError::MissingVersion))
        );
    }
    let mut d = doc.clone();
    d.as_object_mut().unwrap().remove("schema_version");
    assert_eq!(
        Project::from_json(&to_bytes(&d)),
        Err(ProjectError::Migration(MigrationError::MissingVersion))
    );
    // Never read as 0: a literal 0 is a version no step leaves.
    assert_eq!(
        with_version(Value::from(0)),
        Err(ProjectError::Migration(MigrationError::Gap { at: 0 }))
    );
    // A newer document is left untouched by migrate.
    let mut d = doc.clone();
    d["schema_version"] = Value::from(7);
    let before = d.clone();
    assert_eq!(
        migrate::migrate(&mut d, migrate::STEPS, SCHEMA_VERSION),
        Err(MigrationError::TooNew {
            found: 7,
            supported: SCHEMA_VERSION
        })
    );
    assert_eq!(d, before);
    assert!(matches!(
        Project::from_json(b"not json"),
        Err(ProjectError::Json(_))
    ));
    assert_eq!(
        Project::from_json(b"[1, 2]"),
        Err(ProjectError::Migration(MigrationError::MissingVersion))
    );
}

fn sliders_mut(doc: &mut Value) -> Vec<&mut Map<String, Value>> {
    doc["interpretations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .map(|i| i["lens"]["sliders"].as_object_mut().unwrap())
        .collect()
}

fn step_error(from: u32, msg: &str) -> MigrationError {
    MigrationError::Step {
        from,
        msg: msg.to_owned(),
    }
}

/// Test-only v1 -> v2, additive: v1 had no glide/retrigger slider. Writes the
/// reproducing value.
#[allow(clippy::unnecessary_wraps)] // a migrate::StepFn
fn add_glide(doc: &mut Value) -> Result<(), MigrationError> {
    for s in sliders_mut(doc) {
        s.insert("glide_retrigger".to_owned(), Value::from(0.0));
    }
    Ok(())
}

/// Test-only v2 -> v3, a rename with a change of meaning: v2's `swing` is
/// today's `1 - timing_feel`.
fn swing_to_timing_feel(doc: &mut Value) -> Result<(), MigrationError> {
    for s in sliders_mut(doc) {
        let swing = s
            .remove("swing")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| step_error(2, "no swing slider"))?;
        s.insert("timing_feel".to_owned(), Value::from(1.0 - swing));
    }
    Ok(())
}

/// The rename, but it forgets to remove the old field.
fn swing_kept(doc: &mut Value) -> Result<(), MigrationError> {
    for s in sliders_mut(doc) {
        let swing = s["swing"]
            .as_f64()
            .ok_or_else(|| step_error(2, "no swing"))?;
        s.insert("timing_feel".to_owned(), Value::from(1.0 - swing));
    }
    Ok(())
}

/// The additive step, but it forgets to write the new field.
#[allow(clippy::unnecessary_wraps)] // a migrate::StepFn
fn glide_forgotten(_doc: &mut Value) -> Result<(), MigrationError> {
    Ok(())
}

fn failing(_doc: &mut Value) -> Result<(), MigrationError> {
    Err(step_error(1, "refused"))
}

const ADD_GLIDE: Step = Step {
    from: 1,
    apply: add_glide,
};
const RENAME_SWING: Step = Step {
    from: 2,
    apply: swing_to_timing_feel,
};

/// `p` written as a schema-1 file of the test-only history: no glide slider
/// and, with `swing`, timing feel stored as v2's `swing`. Renders keep the
/// keys the old build gave them.
fn as_v1(p: &Project, swing: bool) -> Vec<u8> {
    let mut old = p.clone();
    if swing {
        // The old build keyed `swing`, not today's timing feel; model its key
        // as that of the same interpretation with the swing value.
        for (interp, layer) in old.interpretations.iter_mut().zip(&mut old.layers) {
            interp.lens.sliders.timing_feel = 1.0 - interp.lens.sliders.timing_feel;
            for r in layer.renders.values_mut() {
                r.inputs.interp = interp.key().unwrap();
                r.key = r.inputs.key().unwrap();
            }
        }
    }
    let mut doc: Value = serde_json::from_slice(&old.to_json()).unwrap();
    doc["schema_version"] = Value::from(1);
    for s in sliders_mut(&mut doc) {
        assert_eq!(s.remove("glide_retrigger"), Some(Value::from(0.0)));
        if swing {
            let t = s.remove("timing_feel").unwrap();
            s.insert("swing".to_owned(), t);
        }
    }
    to_bytes(&doc)
}

#[test]
fn migration_chain() {
    // The frozen v1 fixture loads into today's types, and the production
    // chain is total.
    let fixture = Project::from_json(PROJECT_V1.as_bytes()).unwrap();
    assert_eq!(fixture.migrated_from, None);
    assert!(chain_is_total(migrate::STEPS, SCHEMA_VERSION));

    let p = scenario_project();
    let r48 = rate(48_000);
    let current = current_inputs(&p.interpretations[0], r48);
    assert_eq!(p.layers[0].status(&current), LayerStatus::Fresh);

    // v1 -> v2, additive: the step writes the reproducing value, so the
    // render keeps its key and stays fresh.
    let loaded = Project::from_json_with(&as_v1(&p, false), &[ADD_GLIDE], 2).unwrap();
    assert_eq!(loaded.migrated_from, Some(1));
    let mut q = loaded.project;
    assert_eq!(q.schema_version, 2);
    q.schema_version = p.schema_version;
    assert_eq!(q, p);
    assert_eq!(q.layers[0].status(&current), LayerStatus::Fresh);

    // v1 -> v3 through the rename: the meaning changed, so the render is
    // stale, and still there to play.
    let v1 = as_v1(&p, true);
    let loaded = Project::from_json_with(&v1, &[ADD_GLIDE, RENAME_SWING], 3).unwrap();
    assert_eq!(loaded.migrated_from, Some(1));
    let q = loaded.project;
    assert_eq!(q.interpretations, p.interpretations);
    assert_eq!(
        q.layers[0].status(&current),
        LayerStatus::Stale(vec![StaleReason::InterpretationChanged])
    );
    assert_eq!(q.layers[0].renders[&r48].wav, p.layers[0].renders[&r48].wav);
    // Step order in the slice is irrelevant.
    assert_eq!(
        Project::from_json_with(&v1, &[RENAME_SWING, ADD_GLIDE], 3)
            .unwrap()
            .project,
        q
    );

    // A step that forgets to remove a field fails through deny_unknown_fields;
    // one that forgets to write a field fails as missing.
    let forgot_old = Step {
        from: 2,
        apply: swing_kept,
    };
    match Project::from_json_with(&v1, &[ADD_GLIDE, forgot_old], 3) {
        Err(ProjectError::Json(m)) => assert!(m.contains("unknown field `swing`"), "{m}"),
        other => panic!("a forgotten field loaded: {other:?}"),
    }
    let forgot_new = Step {
        from: 1,
        apply: glide_forgotten,
    };
    match Project::from_json_with(&as_v1(&p, false), &[forgot_new], 2) {
        Err(ProjectError::Json(m)) => assert!(m.contains("missing field `glide_retrigger`"), "{m}"),
        other => panic!("a missing field loaded: {other:?}"),
    }
    // A step's own error propagates.
    assert_eq!(
        Project::from_json_with(
            &v1,
            &[Step {
                from: 1,
                apply: failing
            }],
            2
        ),
        Err(ProjectError::Migration(step_error(1, "refused")))
    );

    // A gap is an error.
    let beyond = Step {
        from: 3,
        apply: add_glide,
    };
    assert_eq!(
        Project::from_json_with(&v1, &[ADD_GLIDE, beyond], 4),
        Err(ProjectError::Migration(MigrationError::Gap { at: 2 }))
    );
    assert!(!chain_is_total(&[ADD_GLIDE, beyond], 4));
    assert!(chain_is_total(&[ADD_GLIDE, RENAME_SWING], 3));
    assert!(chain_is_total(&[RENAME_SWING, ADD_GLIDE], 3));
    assert!(!chain_is_total(&[ADD_GLIDE, ADD_GLIDE, RENAME_SWING], 3));
    assert!(!chain_is_total(&[ADD_GLIDE], 3));
    assert!(!chain_is_total(&[ADD_GLIDE, RENAME_SWING], 2));
    assert!(chain_is_total(&[], 1));

    // migrate reports what it did.
    let mut doc: Value = serde_json::from_slice(&as_v1(&p, false)).unwrap();
    assert_eq!(migrate::migrate(&mut doc, &[ADD_GLIDE], 2), Ok(Some(1)));
    assert_eq!(migrate::read_version(&doc), Ok(2));
    assert_eq!(migrate::migrate(&mut doc, &[ADD_GLIDE], 2), Ok(None));
}

/// A test-only document: LensSettings at v1, before `wobble` existed.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LensDocV1 {
    schema_version: u32,
    lens: LensSettings,
}

/// The same document at v2.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LensDocV2 {
    schema_version: u32,
    lens: LensSettingsV2,
}

/// The v1 -> v2 step: writes the frozen reproducing value, never the
/// new-project default.
#[allow(clippy::unnecessary_wraps)] // a migrate::StepFn
fn add_wobble(doc: &mut Value) -> Result<(), MigrationError> {
    doc["lens"]["wobble"] = Value::from(WOBBLE_REPRODUCING);
    Ok(())
}

/// What a new v2 project starts with, under a given new-project default.
fn new_lens_v2(lens: LensSettings, default_wobble: f32) -> LensSettingsV2 {
    LensSettingsV2::from_v1(lens, default_wobble)
}

#[test]
fn default_change_keeps_meaning_and_key() {
    let lens = scenario_project().interpretations[0].lens.clone();
    let v1_key = key_of(KeyKind::Interp, &lens).unwrap();
    let file = serde_json::to_vec(&LensDocV1 {
        schema_version: 1,
        lens: lens.clone(),
    })
    .unwrap();
    let load = |bytes: &[u8]| {
        let mut doc: Value = serde_json::from_slice(bytes).unwrap();
        let from = migrate::migrate(
            &mut doc,
            &[Step {
                from: 1,
                apply: add_wobble,
            }],
            2,
        )
        .unwrap();
        assert_eq!(from, Some(1));
        serde_json::from_value::<LensDocV2>(doc).unwrap()
    };
    // Whatever the new-project default is, 0.25 or anything else, the old
    // file loads with the 0.0 its step wrote and keeps its v1 key.
    for default_wobble in [0.25f32, 0.5, -1.0] {
        let new_project = new_lens_v2(lens.clone(), default_wobble);
        assert_eq!(new_project.wobble.to_bits(), default_wobble.to_bits());
        assert_ne!(key_of(KeyKind::Interp, &new_project).unwrap(), v1_key);
        let loaded = load(&file);
        assert_eq!(loaded.schema_version, 2);
        assert_eq!(loaded.lens.wobble.to_bits(), WOBBLE_REPRODUCING.to_bits());
        assert_eq!(key_of(KeyKind::Interp, &loaded.lens).unwrap(), v1_key);
    }
    // The regression the rule guards against: a step that wrote the default
    // would have changed both meaning and key.
    let mut doc: Value = serde_json::from_slice(&file).unwrap();
    doc["lens"]["wobble"] = Value::from(0.25);
    doc["schema_version"] = Value::from(2);
    let wrong: LensDocV2 = serde_json::from_value(doc).unwrap();
    assert_ne!(key_of(KeyKind::Interp, &wrong.lens).unwrap(), v1_key);
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// `content` followed by its frame hash: a well-framed container.
fn reframe(content: &[u8]) -> Vec<u8> {
    let mut out = content.to_vec();
    out.extend_from_slice(&blake3::derive_key(context::EVIDENCE_CONTAINER, content));
    out
}

fn header_len(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize
}

proptest! {
    #[test]
    fn evidence_container_round_trip(ev in evidence(), at in any::<usize>(), bit in 0u8..8, other in hash32()) {
        let key = ev.header().key;
        let bytes = ev.to_container();
        let back = Evidence::from_container(&bytes, key).unwrap();
        prop_assert_eq!(&back, &ev);
        prop_assert_eq!(back.content_hash(), ev.content_hash());
        prop_assert!(back.to_container() == bytes);
        for c in Column::ALL {
            let (a, b) = (ev.column(c), back.column(c));
            prop_assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
                prop_assert_eq!(bits(a), bits(b));
            }
        }

        // Any flipped byte is detected.
        let mut bad = bytes.clone();
        let i = at % bad.len();
        bad[i] ^= 1 << bit;
        prop_assert_eq!(Evidence::from_container(&bad, key), Err(EvidenceError::Corrupt));
        prop_assert!(Evidence::from_container(&bytes[..bytes.len() - 1], key).is_err());

        // A wrong expected key is never substituted.
        let wrong = EvidenceKey::from_hash(other);
        if wrong != key {
            prop_assert_eq!(Evidence::from_container(&bytes, wrong), Err(EvidenceError::KeyMismatch));
        }

        // Illegal lengths are refused even when well framed.
        let content = &bytes[..bytes.len() - 32];
        let mut longer = content.to_vec();
        longer.extend_from_slice(&[0; 4]);
        prop_assert!(matches!(Evidence::from_container(&reframe(&longer), key), Err(EvidenceError::Container(_))));
        let body_start = 12 + header_len(&bytes);
        if content.len() >= body_start + 4 {
            let shorter = &content[..content.len() - 4];
            prop_assert!(matches!(Evidence::from_container(&reframe(shorter), key), Err(EvidenceError::Container(_))));
        }
        let mut long_header = content.to_vec();
        long_header[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        prop_assert!(matches!(Evidence::from_container(&reframe(&long_header), key), Err(EvidenceError::Container(_))));

        // Another version or magic is a miss.
        let mut v2 = content.to_vec();
        v2[4..8].copy_from_slice(&(CONTAINER_VERSION + 1).to_le_bytes());
        prop_assert!(matches!(Evidence::from_container(&reframe(&v2), key), Err(EvidenceError::Container(_))));
        let mut magic = content.to_vec();
        magic[0] = b'X';
        prop_assert!(matches!(Evidence::from_container(&reframe(&magic), key), Err(EvidenceError::Container(_))));

        // A well-framed edit of the onsets fails the content hash.
        if let Some(first) = ev.onsets().first() {
            let n_cols = ev.columns().count();
            let at = body_start + 4 * ev.n_frames() * n_cols + 8;
            let mut edited = content.to_vec();
            edited[at] ^= 1;
            let strength = f32::from_le_bytes(edited[at..at + 4].try_into().unwrap());
            prop_assert_ne!(strength.to_bits(), first.strength.to_bits());
            prop_assert_eq!(Evidence::from_container(&reframe(&edited), key), Err(EvidenceError::Corrupt));
        }
    }
}

fn small_header(take_frames: u64) -> EvidenceHeader {
    let provenance = Provenance {
        analyzer: "takedown-evidence".to_owned(),
        version: 1,
        models: BTreeMap::new(),
        params: BTreeMap::new(),
    };
    let take = TakeId::from_hash(Hash32([1; 32]));
    EvidenceHeader {
        key: provenance.key(take).unwrap(),
        take,
        take_rate: rate(48_000),
        take_frames,
        provenance,
    }
}

/// Evidence::new's invariants, and NaN as "undefined" in exactly four columns.
#[test]
#[allow(clippy::too_many_lines)] // one check per invariant
fn evidence_invariants_and_nan() {
    let header = small_header(480); // two frames
    let ok = |cols: Vec<(Column, Vec<f32>)>| Evidence::new(header.clone(), cols, Vec::new());
    let payload_nan = f32::from_bits(0xFFC0_1234);
    assert!(payload_nan.is_nan());
    for c in Column::ALL {
        let nan = ok(vec![(c, vec![1.0, payload_nan])]);
        if c.allows_nan() {
            let ev = nan.unwrap();
            assert_eq!(ev.column(c).unwrap()[1].to_bits(), CANONICAL_NAN_BITS);
            // Canonical NaN makes content hashes deterministic.
            let again = ok(vec![(c, vec![1.0, f32::NAN])]).unwrap();
            assert_eq!(ev.content_hash(), again.content_hash());
            assert_eq!(ev, again);
        } else {
            assert_eq!(nan, Err(EvidenceError::NonFinite), "{c:?}");
        }
        for inf in [f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                ok(vec![(c, vec![inf, 0.0])]),
                Err(EvidenceError::NonFinite),
                "{c:?}"
            );
        }
    }
    assert_eq!(
        f32::NAN.to_bits(),
        CANONICAL_NAN_BITS,
        "f32::NAN moved; the frozen constant did not"
    );
    assert_eq!(
        Column::ALL
            .iter()
            .filter(|c| c.allows_nan())
            .copied()
            .collect::<Vec<_>>(),
        vec![
            Column::CentroidHz,
            Column::BrightnessHarm,
            Column::TiltDbOct,
            Column::CppDb
        ]
    );
    // Column names are the serde names.
    for c in Column::ALL {
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            format!("\"{}\"", c.name())
        );
    }
    // -0.0 is kept, and differs from 0.0 in content.
    let neg = ok(vec![(Column::Odf, vec![-0.0, 0.0])]).unwrap();
    let pos = ok(vec![(Column::Odf, vec![0.0, 0.0])]).unwrap();
    assert_ne!(neg.content_hash(), pos.content_hash());
    assert_ne!(neg, pos);

    assert_eq!(
        ok(vec![(Column::Odf, vec![0.0])]),
        Err(EvidenceError::ColumnLength(Column::Odf))
    );
    assert_eq!(
        ok(vec![
            (Column::Odf, vec![0.0; 2]),
            (Column::F0Hz0, vec![0.0; 2])
        ]),
        Err(EvidenceError::ColumnOrder)
    );
    assert_eq!(
        ok(vec![
            (Column::Odf, vec![0.0; 2]),
            (Column::Odf, vec![0.0; 2])
        ]),
        Err(EvidenceError::ColumnOrder)
    );
    let onset = |at: u64, strength: f32| OnsetCandidate {
        at: TimelinePos(at),
        strength,
        rise_db: 3.0,
    };
    let with_onsets = |o: Vec<OnsetCandidate>| Evidence::new(header.clone(), Vec::new(), o);
    assert!(with_onsets(vec![onset(0, 1.0), onset(0, 0.5), onset(479, 1.0)]).is_ok());
    assert_eq!(
        with_onsets(vec![onset(480, 1.0)]),
        Err(EvidenceError::Onsets)
    );
    assert_eq!(
        with_onsets(vec![onset(10, 1.0), onset(9, 1.0)]),
        Err(EvidenceError::Onsets)
    );
    assert_eq!(
        with_onsets(vec![onset(10, f32::NAN)]),
        Err(EvidenceError::NonFinite)
    );
    let mut lying = header.clone();
    lying.key = EvidenceKey::from_hash(Hash32([0; 32]));
    assert_eq!(
        Evidence::new(lying, Vec::new(), Vec::new()),
        Err(EvidenceError::KeyMismatch)
    );
    let mut bad_params = header.clone();
    bad_params
        .provenance
        .params
        .insert("x".to_owned(), ParamValue::Float(f32::INFINITY));
    assert_eq!(
        Evidence::new(bad_params, Vec::new(), Vec::new()),
        Err(EvidenceError::Key(KeyError::NonFinite))
    );
    // An empty take has no frames and no onsets.
    let empty = Evidence::new(small_header(0), Vec::new(), Vec::new()).unwrap();
    assert_eq!(empty.n_frames(), 0);
    assert_eq!(
        Evidence::from_container(&empty.to_container(), empty.header().key),
        Ok(empty)
    );
}

/// Provenance JSON with every object's keys in reverse order.
fn reversed_json(p: &Provenance) -> String {
    let obj = |entries: Vec<(String, String)>| {
        let body: Vec<String> = entries
            .into_iter()
            .rev()
            .map(|(k, v)| format!("{}:{v}", serde_json::to_string(&k).unwrap()))
            .collect();
        format!("{{{}}}", body.join(","))
    };
    let models = obj(p
        .models
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::to_string(v).unwrap()))
        .collect());
    let params = obj(p
        .params
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::to_string(v).unwrap()))
        .collect());
    obj(vec![
        (
            "analyzer".to_owned(),
            serde_json::to_string(&p.analyzer).unwrap(),
        ),
        ("version".to_owned(), p.version.to_string()),
        ("models".to_owned(), models),
        ("params".to_owned(), params),
    ])
}

proptest! {
    #[test]
    fn evidence_resolution_rules(p in provenance(), q in provenance(), take in hash32(), other in hash32()) {
        use EvidenceResolution::{NeedsUpgrade, RecomputeSame, UseCached};
        let take = TakeId::from_hash(take);
        let pinned = p.key(take).unwrap();
        // Cached: load it, whatever is running.
        prop_assert_eq!(resolve_evidence(pinned, true, take, &q), Ok(UseCached));
        prop_assert_eq!(resolve_evidence(pinned, true, take, &p), Ok(UseCached));
        // Not cached, same provenance: recompute without asking.
        prop_assert_eq!(resolve_evidence(pinned, false, take, &p), Ok(RecomputeSame));
        // Params order is irrelevant, in memory and in text.
        let reordered: Provenance = serde_json::from_str(&reversed_json(&p)).unwrap();
        prop_assert_eq!(&reordered, &p);
        prop_assert_eq!(resolve_evidence(pinned, false, take, &reordered), Ok(RecomputeSame));
        // Anything else: ask.
        let upgrade = |running: &Provenance, t: TakeId| -> Result<EvidenceResolution, KeyError> {
            Ok(NeedsUpgrade { pinned, available: running.key(t)? })
        };
        let qk = q.key(take).unwrap();
        let expect = if qk == pinned { Ok(RecomputeSame) } else { upgrade(&q, take) };
        prop_assert_eq!(resolve_evidence(pinned, false, take, &q), expect);
        let mut changes: Vec<Provenance> = Vec::new();
        let mut c = p.clone(); c.version += 1; changes.push(c);
        let mut c = p.clone(); c.analyzer.push('2'); changes.push(c);
        let mut c = p.clone(); c.models.insert("zz_new_9".to_owned(), Hash32([3; 32])); changes.push(c);
        let mut c = p.clone(); c.params.insert("zz_new_9".to_owned(), ParamValue::Int(1)); changes.push(c);
        let mut c = p.clone(); c.params.insert("zz_new_9".to_owned(), ParamValue::Float(1.0)); changes.push(c);
        for running in &changes {
            let r = resolve_evidence(pinned, false, take, running);
            prop_assert!(matches!(r, Ok(NeedsUpgrade { .. })), "{:?}", r);
            prop_assert_eq!(r, upgrade(running, take));
        }
        // A different take is different evidence.
        let other = TakeId::from_hash(other);
        if other != take {
            prop_assert_eq!(resolve_evidence(pinned, false, other, &p), upgrade(&p, other));
        }
        // A non-finite param cannot be keyed.
        let mut bad = p.clone();
        bad.params.insert("zz".to_owned(), ParamValue::Float(f32::NAN));
        prop_assert_eq!(resolve_evidence(pinned, false, take, &bad), Err(KeyError::NonFinite));
    }
}

// ---------------------------------------------------------------------------
// Takes
// ---------------------------------------------------------------------------

/// Feeds `samples` cut at `cuts` (positions modulo the length).
fn hash_chunks(
    rate: SampleRate,
    channels: u16,
    samples: &[f32],
    cuts: &[usize],
) -> Result<(TakeId, u64), TakeError> {
    let mut at: Vec<usize> = cuts.iter().map(|c| c % (samples.len() + 1)).collect();
    at.push(0);
    at.push(samples.len());
    at.sort_unstable();
    let mut h = TakeHasher::new(rate, channels);
    let mut first = Ok(());
    for w in at.windows(2) {
        let r = h.update(&samples[w[0]..w[1]]);
        if first.is_ok() {
            first = r;
        }
    }
    first?;
    h.finish()
}

/// A finite sample by bit pattern, with -0.0 and subnormals.
fn sample(r: u64) -> f32 {
    match r % 6 {
        0 => -0.0,
        1 => f32::from_bits((r >> 32) as u32 & 0x807F_FFFF),
        _ => {
            let v = f32::from_bits((r >> 16) as u32);
            if v.is_finite() { v } else { 0.25 }
        }
    }
}

proptest! {
    #[test]
    fn take_id_chunk_invariant(
        r in std_rate(),
        channels in 1u16..=4,
        frames in 0usize..600,
        salt in any::<u64>(),
        cuts in vec(any::<usize>(), 0..6),
        bad_at in any::<usize>(),
        bad_kind in 0usize..4,
        other in std_rate(),
    ) {
        let n = frames * usize::from(channels);
        let samples: Vec<f32> = (0..n as u64).map(|i| sample(splitmix(salt ^ i))).collect();
        let whole = hash_chunks(r, channels, &samples, &[]).unwrap();
        prop_assert_eq!(whole.1, frames as u64);
        prop_assert_eq!(hash_chunks(r, channels, &samples, &cuts).unwrap(), whole);
        let one_by_one: Vec<usize> = (0..n).collect();
        prop_assert_eq!(hash_chunks(r, channels, &samples, &one_by_one).unwrap(), whole);

        // The id depends on rate and channels.
        if other != r {
            prop_assert_ne!(hash_chunks(other, channels, &samples, &[]).unwrap().0, whole.0);
        }
        if channels > 1 {
            prop_assert_ne!(hash_chunks(r, 1, &samples, &[]).unwrap().0, whole.0);
        }
        if n % 2 == 0 && channels == 1 {
            prop_assert_ne!(hash_chunks(r, 2, &samples, &[]).unwrap().0, whole.0);
        }

        // NaN and infinities are rejected at their index, and stay rejected.
        if n > 0 {
            let i = bad_at % n;
            let mut bad = samples.clone();
            bad[i] = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::from_bits(0xFF80_0001)][bad_kind];
            let err = TakeError::NonFinite { sample: i as u64 };
            prop_assert_eq!(hash_chunks(r, channels, &bad, &cuts), Err(err));
            let mut h = TakeHasher::new(r, channels);
            prop_assert_eq!(h.update(&bad), Err(err));
            prop_assert_eq!(h.update(&samples), Err(err));
            prop_assert_eq!(h.finish(), Err(err));
        }

        // A partial frame and zero channels are refused.
        if channels > 1 {
            let mut h = TakeHasher::new(r, channels);
            h.update(&samples).unwrap();
            h.update(&[0.0]).unwrap();
            prop_assert_eq!(h.finish(), Err(TakeError::PartialFrame));
        }
        prop_assert_eq!(hash_chunks(r, 0, &samples, &cuts), Err(TakeError::Channels));
    }
}

#[test]
fn identical_pcm_dedupes() {
    let r = rate(44_100);
    let samples = pcm(8_820, 42);
    let (a, frames) = take_id_of(r, 2, &samples);
    let (b, _) = hash_chunks(r, 2, &samples, &[1, 777, 4_000]).unwrap();
    assert_eq!(a, b);
    let first = Take {
        name: "first".to_owned(),
        ..take_of(r, 2, &samples, "")
    };
    let second = Take {
        id: b,
        audio: AudioInfo {
            rate: r,
            channels: 2,
            frames,
        },
        source: CaptureSource::Dropped {
            original_name: "loop.wav".to_owned(),
            source_format: "wav f32".to_owned(),
        },
        anchor: None,
        name: "second".to_owned(),
        captured_at: "2026-10-22T10:00:00Z".to_owned(),
    };
    let mut p = Project::new(ProjectId::from_hash(Hash32([1; 32])), r);
    assert_eq!(p.add_take(first.clone()), TakeAdded::New);
    assert_eq!(p.add_take(second), TakeAdded::Existing);
    assert_eq!(p.takes.len(), 1);
    assert_eq!(p.takes[&a], first);
    // Different PCM is a different take.
    let mut other = samples.clone();
    other[0] = -other[0];
    assert_eq!(p.add_take(take_of(r, 2, &other, "third")), TakeAdded::New);
    assert_eq!(p.takes.len(), 2);
}

// ---------------------------------------------------------------------------
// Rates, renders and the project directory
// ---------------------------------------------------------------------------

#[test]
fn sample_rate_change_mid_project() {
    let dir = TempDir::new("rate-change");
    let (r48, r44, r96) = (rate(48_000), rate(44_100), rate(96_000));
    let mut p = scenario_project();
    let interps_before = p.interpretations.clone();
    let key_before = p.interpretations[0].key().unwrap();
    let render48 = p.layers[0].renders[&r48].clone();
    let at = |rate| current_inputs(&p.interpretations[0], rate);
    assert_eq!(p.layers[0].status(&at(r48)), LayerStatus::Fresh);

    // Output switched to 44.1 kHz: nothing stored moves; the new slot is empty.
    p.output_rate = r44;
    let at44 = current_inputs(&p.interpretations[0], r44);
    assert_eq!(p.layers[0].status(&at44), LayerStatus::NotRendered);
    p.layers[0].store_render(render_ref(at44.clone(), b"render at 44.1 kHz, lens 1"));
    assert_eq!(p.layers[0].status(&at44), LayerStatus::Fresh);
    assert_eq!(p.layers[0].renders.len(), 2);
    assert_eq!(p.layers[0].renders[&r48], render48);

    // A 96 kHz take arrives, with its own interpretation.
    let take96 = take_of(r96, 1, &pcm(9_600, 96), "hum at 96k");
    let take96_id = take96.id;
    assert_eq!(p.add_take(take96), TakeAdded::New);
    let mut interp96 = p.interpretations[0].clone();
    interp96.id = InterpId(p.alloc_id());
    interp96.take = Some(take96_id);
    interp96.source = ControlSource::Lens {
        evidence: EvidenceKey::from_hash(Hash32([0x96; 32])),
        hits: None,
        edits: vec![EditEntry {
            id: EditId(p.alloc_id()),
            edit: Edit::SplitAt {
                at: TimelinePos(4_800),
                articulation: Articulation::Retrigger,
            },
        }],
    };
    p.interpretations.push(interp96);
    p.validate().unwrap();
    assert_eq!(p.interpretations[0], interps_before[0]);
    assert_eq!(p.interpretations[0].key().unwrap(), key_before);

    // Switching back to 48 kHz costs nothing: the old render is still fresh.
    p.output_rate = r48;
    assert_eq!(
        p.layers[0].status(&current_inputs(&p.interpretations[0], r48)),
        LayerStatus::Fresh
    );
    assert_eq!(p.layers[0].renders[&r48], render48);

    // All of it survives save and reload.
    p.save(dir.path(), None).unwrap();
    let q = Project::from_json(&fs::read(dir.path().join(paths::PROJECT)).unwrap())
        .unwrap()
        .project;
    assert_eq!(q, p);
    assert_eq!(q.interpretations[0].key().unwrap(), key_before);
    assert_eq!(
        q.layers[0].status(&current_inputs(&q.interpretations[0], r48)),
        LayerStatus::Fresh
    );
    assert_eq!(q.layers[0].status(&at44), LayerStatus::Fresh);
}

proptest! {
    #[test]
    fn render_key_tracks_running_versions(
        base in render_inputs(),
        wav in hash32(),
        other in hash32(),
        other_rate in std_rate(),
    ) {
        let k0 = base.key().unwrap();
        let mut layer = Layer {
            id: LayerId(1),
            interp: InterpId(0),
            gain_db: 0.0,
            mute: false,
            renders: BTreeMap::new(),
        };
        layer.store_render(RenderRef {
            key: k0,
            inputs: base.clone(),
            wav: WavId::from_hash(wav),
            frames: 1,
            platform: PlatformClass { arch: "x86_64".to_owned(), os: "windows".to_owned() },
        });
        prop_assert_eq!(layer.status(&base), LayerStatus::Fresh);

        // Platform class, gain and mute change nothing.
        let mut moved = layer.clone();
        moved.renders.get_mut(&base.rate).unwrap().platform = PlatformClass { arch: "aarch64".to_owned(), os: "macos".to_owned() };
        moved.gain_db = -6.0;
        moved.mute = true;
        prop_assert_eq!(moved.status(&base), LayerStatus::Fresh);
        // Backend order is irrelevant.
        let mut reordered = base.clone();
        reordered.backends.reverse();
        prop_assert_eq!(reordered.key().unwrap(), k0);
        prop_assert_eq!(layer.status(&reordered), LayerStatus::Fresh);

        let sorted = |mut b: Vec<BackendStamp>| { b.sort(); b };
        let mut cases: Vec<(RenderInputs, StaleReason)> = Vec::new();
        let mut c = base.clone();
        c.interp = InterpKey::from_hash(flip(c.interp.hash32()));
        cases.push((c, StaleReason::InterpretationChanged));
        let mut c = base.clone();
        c.evidence = Some(c.evidence.map_or(other, flip));
        cases.push((c, StaleReason::EvidenceContentChanged));
        let mut c = base.clone();
        c.hits = match c.hits { Some(_) => None, None => Some(other) };
        cases.push((c, StaleReason::HitsChanged));
        let mut c = base.clone();
        c.lens_version += 1;
        cases.push((c, StaleReason::LensUpgraded { from: base.lens_version, to: base.lens_version + 1 }));
        let mut c = base.clone();
        c.backends.push(BackendStamp { id: "zz_new_9".to_owned(), version: 1 });
        let to = sorted(c.backends.clone());
        cases.push((c, StaleReason::BackendsChanged { from: base.backends.clone(), to }));
        if !base.backends.is_empty() {
            let mut c = base.clone();
            c.backends[0].version += 10;
            let to = sorted(c.backends.clone());
            cases.push((c, StaleReason::BackendsChanged { from: base.backends.clone(), to }));
        }
        let mut c = base.clone();
        c.contract += 1;
        cases.push((c, StaleReason::ContractUpgraded { from: base.contract, to: base.contract + 1 }));
        for (current, reason) in cases {
            prop_assert_ne!(current.key().unwrap(), k0);
            prop_assert_eq!(layer.status(&current), LayerStatus::Stale(vec![reason]));
        }

        // Several changes report every reason, in field order.
        let mut both = base.clone();
        both.lens_version += 1;
        both.contract += 2;
        prop_assert_eq!(layer.status(&both), LayerStatus::Stale(vec![
            StaleReason::LensUpgraded { from: base.lens_version, to: base.lens_version + 1 },
            StaleReason::ContractUpgraded { from: base.contract, to: base.contract + 2 },
        ]));

        // The rate is in the key and picks the slot.
        if other_rate != base.rate {
            let mut c = base.clone();
            c.rate = other_rate;
            prop_assert_ne!(c.key().unwrap(), k0);
            prop_assert_eq!(layer.status(&c), LayerStatus::NotRendered);
        }
        // Duplicate backends cannot be keyed.
        if let Some(first) = base.backends.first() {
            let mut c = base.clone();
            c.backends.push(first.clone());
            prop_assert_eq!(c.key(), Err(KeyError::DuplicateKey));
        }
    }
}

/// A minimal mono f32 WAV.
fn wav_bytes(samples: &[f32], rate: SampleRate) -> Vec<u8> {
    let data = 4 * samples.len() as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.hz().to_le_bytes());
    out.extend_from_slice(&(rate.hz() * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn write(dir: &Path, rel: &Path, bytes: &[u8]) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn render_files_are_content_addressed() {
    let dir = TempDir::new("content-addressed");
    let r48 = rate(48_000);
    let mut p = scenario_project();
    let interp = p.interpretations[0].clone();
    // Two renders that differ only in the lens version that made them.
    let v1 = current_inputs(&interp, r48);
    let v2 = RenderInputs {
        lens_version: 2,
        ..v1.clone()
    };
    assert_ne!(v1.key().unwrap(), v2.key().unwrap());
    let audio1 = wav_bytes(&pcm(480, 1), r48);
    let audio2 = wav_bytes(&pcm(480, 2), r48);
    let (wav1, wav2) = (WavId::of_bytes(&audio1), WavId::of_bytes(&audio2));
    assert_ne!(wav1, wav2);
    let mut r1 = render_ref(v1.clone(), &audio1);
    r1.wav = wav1;
    let mut r2 = render_ref(v2.clone(), &audio2);
    r2.wav = wav2;
    p.layers[0].renders.clear();
    p.layers[0].store_render(r1.clone());
    let mut second = p.layers[0].clone();
    second.id = LayerId(p.alloc_id());
    second.renders.clear();
    second.store_render(r2.clone());
    p.layers.push(second);
    assert_eq!(p.layers[0].status(&v1), LayerStatus::Fresh);
    assert_eq!(p.layers[1].status(&v2), LayerStatus::Fresh);

    // Both live under renders/, outside cache/.
    for (id, bytes) in [(wav1, &audio1), (wav2, &audio2)] {
        let rel = paths::render(id);
        assert!(rel.starts_with("renders"), "{rel:?}");
        assert!(!rel.starts_with(paths::CACHE));
        write(dir.path(), &rel, bytes);
    }
    // The cache holds only what can be recomputed.
    let take = p.interpretations[0].take.unwrap();
    for rel in [
        paths::evidence_cache(EvidenceKey::from_hash(Hash32([5; 32]))),
        paths::hits_cache(HitSetKey::from_hash(Hash32([6; 32]))),
        paths::peaks(take),
    ] {
        assert!(rel.starts_with(paths::CACHE), "{rel:?}");
        write(dir.path(), &rel, b"recomputable");
    }
    for rel in [
        paths::audio(take),
        paths::control(ControlId::from_hash(Hash32([1; 32]))),
        paths::midi(MidiFileId::from_hash(Hash32([1; 32]))),
        paths::palette(p.interpretations[0].lens.palette.id),
        paths::teach(TeachSetId::from_hash(Hash32([1; 32]))),
    ] {
        assert!(!rel.starts_with(paths::CACHE), "{rel:?}");
    }
    p.save(dir.path(), None).unwrap();

    // Deleting cache/ loses no render.
    fs::remove_dir_all(dir.path().join(paths::CACHE)).unwrap();
    let q = Project::from_json(&fs::read(dir.path().join(paths::PROJECT)).unwrap())
        .unwrap()
        .project;
    assert_eq!(q, p);
    for layer in &q.layers {
        for r in layer.renders.values() {
            let bytes = fs::read(dir.path().join(paths::render(r.wav))).unwrap();
            assert_eq!(WavId::of_bytes(&bytes), r.wav);
        }
    }
    assert_eq!(q.layers[0].renders[&r48], r1);
    assert_eq!(q.layers[1].renders[&r48], r2);
}

#[test]
fn atomic_save_and_backup() {
    let p = scenario_project();
    let dir = TempDir::new("atomic");
    let target = dir.path().join(paths::PROJECT);
    let backup = dir.path().join(paths::backup(1));

    // A file migrated from schema 1.
    let original = as_v1(&p, false);
    fs::write(&target, &original).unwrap();
    let loaded = Project::from_json_with(&fs::read(&target).unwrap(), &[ADD_GLIDE], 2).unwrap();
    assert_eq!(loaded.migrated_from, Some(1));
    let mut q = loaded.project;

    // The first save backs the original up, then replaces it.
    q.save(dir.path(), loaded.migrated_from).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert_eq!(fs::read(&target).unwrap(), q.to_json());
    assert_eq!(
        dir.entries(),
        vec![paths::PROJECT.to_owned(), "project.v1.json.bak".to_owned()]
    );

    // Later saves never touch the backup.
    q.layers[0].gain_db = -9.0;
    q.save(dir.path(), loaded.migrated_from).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert_eq!(fs::read(&target).unwrap(), q.to_json());
    assert_eq!(dir.entries().len(), 2);

    // An invalid project is refused, and nothing is written.
    let before = fs::read(&target).unwrap();
    let mut bad = q.clone();
    bad.layers[0].gain_db = f32::NAN;
    let err = bad.save(dir.path(), None).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&target).unwrap(), before);
    assert_eq!(dir.entries().len(), 2);

    // An unmigrated save writes no backup; a migrated save with no original
    // has nothing to back up.
    let fresh = TempDir::new("atomic-fresh");
    p.save(fresh.path(), None).unwrap();
    assert_eq!(fresh.entries(), vec![paths::PROJECT.to_owned()]);
    let empty = TempDir::new("atomic-empty");
    p.save(empty.path(), Some(1)).unwrap();
    assert_eq!(empty.entries(), vec![paths::PROJECT.to_owned()]);

    // A failed rename leaves no temp file behind.
    let blocked = TempDir::new("atomic-blocked");
    fs::create_dir(blocked.path().join(paths::PROJECT)).unwrap();
    fs::write(blocked.path().join(paths::PROJECT).join("keep"), b"x").unwrap();
    assert!(p.save(blocked.path(), None).is_err());
    assert_eq!(blocked.entries(), vec![paths::PROJECT.to_owned()]);
}

// ---------------------------------------------------------------------------
// Validation and the frozen fixture
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // one case per refusal
fn validate_refuses_broken_projects() {
    let p = scenario_project();
    // Refused by validate, and so by from_json (where a non-finite value is
    // already a JSON error) and by save.
    let check = |f: &dyn Fn(&mut Project), want: ProjectError| {
        let mut q = p.clone();
        f(&mut q);
        assert_eq!(q.validate(), Err(want.clone()));
        assert!(Project::from_json(&q.to_json()).is_err(), "{want:?} loaded");
    };
    let iid = p.interpretations[0].id;
    let lid = p.layers[0].id;
    let take = p.interpretations[0].take.unwrap();
    let ghost = TakeId::from_hash(Hash32([0xAA; 32]));
    check(
        &|q| q.layers[0].gain_db = f32::INFINITY,
        ProjectError::NonFinite(lid),
    );
    check(
        &|q| q.layers[0].id = LayerId(iid.0),
        ProjectError::DuplicateId(iid.0),
    );
    check(&|q| q.next_id = 1, ProjectError::DuplicateId(1));
    check(
        &|q| q.layers[0].interp = InterpId(77),
        ProjectError::UnknownInterp(InterpId(77)),
    );
    check(
        &|q| q.interpretations[0].take = Some(ghost),
        ProjectError::UnknownTake(ghost),
    );
    let palette = PaletteId::from_hash(Hash32([0xBB; 32]));
    check(
        &|q| q.interpretations[0].lens.palette.id = palette,
        ProjectError::UnknownPalette(palette),
    );
    check(
        &|q| q.interpretations[0].take = None,
        ProjectError::Interp(iid, InterpError::MissingTake),
    );
    check(
        &|q| q.interpretations[0].lens.sliders.glide_retrigger = 1.5,
        ProjectError::Interp(
            iid,
            InterpError::Lens(LensError::OutOfRange(Slider::GlideRetrigger)),
        ),
    );
    check(
        &|q| {
            if let ControlSource::Lens { edits, .. } = &mut q.interpretations[0].source {
                edits[0].edit = Edit::ForcePitch {
                    span: span(0, 1),
                    st: f32::NAN,
                };
            }
        },
        ProjectError::Interp(iid, InterpError::Key(KeyError::NonFinite)),
    );
    check(
        &|q| {
            q.layers[0]
                .renders
                .values_mut()
                .next()
                .unwrap()
                .inputs
                .lens_version = 9;
        },
        ProjectError::RenderKey(lid),
    );
    check(
        &|q| {
            let r = q.layers[0].renders.values().next().unwrap().clone();
            q.layers[0].renders.insert(rate(96_000), r);
        },
        ProjectError::RenderKey(lid),
    );
    check(
        &|q| {
            let t = q.takes.get_mut(&take).unwrap();
            t.anchor = Some(HostAnchor {
                host_rate: rate(48_000),
                host_sample_at_start: 0,
                ppq_at_start: f64::NAN,
                tempo_bpm: 120.0,
                time_sig: (4, 4),
            });
        },
        ProjectError::InvalidTake(take),
    );
    // The interpretation-level checks that need data outside the project.
    let interp = &p.interpretations[0];
    let header = EvidenceHeader {
        key: EvidenceKey::from_hash(Hash32([5; 32])),
        take,
        ..small_header(10)
    };
    assert_eq!(interp.check_evidence(&header), Ok(()));
    let elsewhere = EvidenceHeader {
        take: ghost,
        ..header
    };
    assert_eq!(
        interp.check_evidence(&elsewhere),
        Err(InterpError::EvidenceTake)
    );
}

#[test]
fn v1_fixture_loads_with_frozen_keys() {
    let loaded = Project::from_json(PROJECT_V1.as_bytes()).unwrap();
    assert_eq!(loaded.migrated_from, None);
    let p = loaded.project;
    let mut out = Vec::new();
    for i in &p.interpretations {
        out.push((
            format!("fixture/project.v1/interp/{}", i.id.0),
            i.key().unwrap().to_string(),
        ));
    }
    for l in &p.layers {
        for (r, render) in &l.renders {
            out.push((
                format!("fixture/project.v1/render/{}/{}", l.id.0, r.hz()),
                render.inputs.key().unwrap().to_string(),
            ));
        }
    }
    check_frozen("fixture/", &out);
    // It covers what the format can hold.
    let kinds: BTreeSet<&str> = p
        .interpretations
        .iter()
        .flat_map(|i| match &i.source {
            ControlSource::Lens { edits, .. } => edits
                .iter()
                .map(|e| match e.edit {
                    Edit::ForcePitch { .. } => "force_pitch",
                    Edit::SplitAt { .. } => "split_at",
                    Edit::JoinAt { .. } => "join_at",
                    Edit::ForceRole { .. } => "force_role",
                    Edit::DeleteHit { .. } => "delete_hit",
                    Edit::AddHit { .. } => "add_hit",
                    Edit::MuteSpan { .. } => "mute_span",
                })
                .collect::<Vec<_>>(),
            ControlSource::Imported { .. } => vec!["imported"],
        })
        .collect();
    assert_eq!(kinds.len(), 8, "{kinds:?}");
    assert!(p.takes.values().any(|t| t.anchor.is_some()));
    assert!(p.layers.iter().any(|l| l.renders.len() == 2));
    // And it is stable through today's code.
    let again = Project::from_json(&p.to_json()).unwrap().project;
    assert_eq!(again, p);
}
