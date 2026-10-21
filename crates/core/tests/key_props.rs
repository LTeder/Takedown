//! core-types §3.3, the key half: the canonical key encoding and one frozen
//! key of every kind. The persistence half is `persistence_props.rs`.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{check_frozen, rate};
use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use takedown_core::canonical::MidiBinding;
use takedown_core::control::{
    ControlTrack, Curves, GateSpan, HitEvent, Link, TrackParts, U7, Velocity,
};
use takedown_core::edit::{Articulation, Edit, EditEntry};
use takedown_core::evidence::{Column, Evidence, EvidenceHeader, OnsetCandidate, Provenance};
use takedown_core::hit::{
    ClassifierStamp, Hit, HitAnchor, HitSet, RoleName, TeachLabel, TeachSet, hit_set_key,
};
use takedown_core::ids::{
    ControlId, EditId, EvidenceKey, Hash32, HitSetKey, InterpId, InterpKey, MidiFileId, PaletteId,
    TakeId,
};
use takedown_core::interp::{ControlSource, Interpretation};
use takedown_core::key::{KeyError, KeyHash, KeyKind, KeyWriter, Record, key_of};
use takedown_core::layer::{BackendStamp, RenderInputs};
use takedown_core::lens::{LensSettings, PaletteRef, Slider, Sliders};
use takedown_core::params::{ParamValue, Params};
use takedown_core::seed::{Aspect, Seed, SeedTree};
use takedown_core::take::TakeHasher;
use takedown_core::time::{TimelinePos, TimelineSpan};

fn enc<T: KeyHash + ?Sized>(v: &T) -> Result<Vec<u8>, KeyError> {
    KeyWriter::encode(v)
}

/// A value whose encoding is written by a closure.
struct With<F>(F);

impl<F: Fn(&mut KeyWriter) -> Result<(), KeyError>> KeyHash for With<F> {
    fn write_key(&self, w: &mut KeyWriter) -> Result<(), KeyError> {
        (self.0)(w)
    }
}

fn shuffled<T: Clone>(v: &[T], salt: u64) -> Vec<T> {
    let mut out = v.to_vec();
    for i in (1..out.len()).rev() {
        let j = (common::splitmix(salt ^ i as u64) % (i as u64 + 1)) as usize;
        out.swap(i, j);
    }
    out
}

proptest! {
    #[test]
    fn key_encoding_rules(
        entries in btree_map("[a-z]{0,6}", any::<u32>(), 0..12),
        salt in any::<u64>(),
        small in any::<u8>(),
        neg in any::<i8>(),
        x in any::<f32>(),
        s in "[a-c]{0,8}",
        split_a in 0usize..9,
        split_b in 0usize..9,
        ints in vec(any::<u16>(), 0..6),
    ) {
        // Map and set insertion order do not change keys.
        let pairs: Vec<(String, u32)> = entries.clone().into_iter().collect();
        let mixed = shuffled(&pairs, salt);
        let in_order = enc(&With(|w: &mut KeyWriter| w.map(pairs.iter().map(|(k, v)| (k, v))))).unwrap();
        let out_of_order = enc(&With(|w: &mut KeyWriter| w.map(mixed.iter().map(|(k, v)| (k, v))))).unwrap();
        prop_assert_eq!(&in_order, &out_of_order);
        prop_assert_eq!(&in_order, &enc(&entries).unwrap());
        let keys: Vec<String> = pairs.iter().map(|p| p.0.clone()).collect();
        let set_a = enc(&With(|w: &mut KeyWriter| w.set(keys.iter()))).unwrap();
        let set_b = enc(&With(|w: &mut KeyWriter| w.set(shuffled(&keys, !salt).iter()))).unwrap();
        prop_assert_eq!(&set_a, &set_b);
        prop_assert_eq!(&set_a, &enc(&keys.iter().cloned().collect::<BTreeSet<_>>()).unwrap());
        if let Some(k) = keys.first() {
            let dup = [k.clone(), k.clone()];
            prop_assert_eq!(enc(&With(|w: &mut KeyWriter| w.set(dup.iter()))), Err(KeyError::DuplicateKey));
            let dup_map = [(k.clone(), 1u8), (k.clone(), 2u8)];
            prop_assert_eq!(
                enc(&With(|w: &mut KeyWriter| w.map(dup_map.iter().map(|(k, v)| (k, v))))),
                Err(KeyError::DuplicateKey)
            );
        }
        // A list keeps order.
        let mut reversed = ints.clone();
        reversed.reverse();
        prop_assert_eq!(enc(&ints) == enc(&reversed), ints == reversed);

        // Integer width does not change keys.
        let w = enc(&small).unwrap();
        prop_assert_eq!(&w, &enc(&u16::from(small)).unwrap());
        prop_assert_eq!(&w, &enc(&u32::from(small)).unwrap());
        prop_assert_eq!(&w, &enc(&u64::from(small)).unwrap());
        prop_assert_eq!(&w, &enc(&i16::from(small)).unwrap());
        prop_assert_eq!(&w, &enc(&i64::from(small)).unwrap());
        prop_assert_eq!(&w, &enc(&usize::from(small)).unwrap());
        prop_assert_eq!(&w, &enc(&i128::from(small)).unwrap());
        prop_assert_eq!(enc(&neg).unwrap(), enc(&i64::from(neg)).unwrap());

        // Floats: -0.0 equals 0.0; NaN and infinities are refused, also nested.
        prop_assert_eq!(enc(&-0.0f32).unwrap(), enc(&0.0f32).unwrap());
        if x.is_finite() {
            let e = enc(&x).unwrap();
            prop_assert_eq!(e == enc(&0.0f32).unwrap(), x == 0.0);
        } else {
            prop_assert_eq!(enc(&x), Err(KeyError::NonFinite));
        }
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            prop_assert_eq!(enc(&bad), Err(KeyError::NonFinite));
            prop_assert_eq!(enc(&vec![1.0f32, bad]), Err(KeyError::NonFinite));
            prop_assert_eq!(
                enc(&With(|w: &mut KeyWriter| w.record(|r| r.field("x", &bad)))),
                Err(KeyError::NonFinite)
            );
            prop_assert_eq!(enc(&ParamValue::Float(bad)), Err(KeyError::NonFinite));
        }

        // Strings with shifted boundaries do not collide.
        let (a, b) = (split_a.min(s.len()), split_b.min(s.len()));
        let one = [&s[..a], &s[a..]];
        let two = [&s[..b], &s[b..]];
        prop_assert_eq!(enc(&one[..]) == enc(&two[..]), a == b);
        let rec = |p: [&str; 2]| enc(&With(move |w: &mut KeyWriter| w.record(|r| {
            r.field("a", p[0])?;
            r.field("b", p[1])
        })));
        prop_assert_eq!(rec(one) == rec(two), a == b);

        // Record fields are sorted by name; a repeated name is refused.
        let fwd = enc(&With(|w: &mut KeyWriter| w.record(|r| {
            r.field("alpha", &1u8)?;
            r.field("beta", "x")
        }))).unwrap();
        let back = enc(&With(|w: &mut KeyWriter| w.record(|r| {
            r.field("beta", "x")?;
            r.field("alpha", &1u8)
        }))).unwrap();
        prop_assert_eq!(&fwd, &back);
        prop_assert_eq!(
            enc(&With(|w: &mut KeyWriter| w.record(|r| {
                r.field("a", &1u8)?;
                r.field("a", &2u8)
            }))),
            Err(KeyError::DuplicateKey)
        );

        // Variants are identified by name, and differ from records.
        let fields = |r: &mut Record<'_>| r.field("v", &small);
        let va = enc(&With(|w: &mut KeyWriter| w.variant("alpha", fields))).unwrap();
        let vb = enc(&With(|w: &mut KeyWriter| w.variant("beta", fields))).unwrap();
        let rc = enc(&With(|w: &mut KeyWriter| w.record(fields))).unwrap();
        prop_assert_ne!(&va, &vb);
        prop_assert_ne!(&va, &rc);
        prop_assert_ne!(enc(&ParamValue::Int(1)).unwrap(), enc(&ParamValue::Float(1.0)).unwrap());

        // None is omitted; a reproducing value is omitted; anything else is written.
        let opt = |v: Option<u32>| enc(&With(move |w: &mut KeyWriter| w.record(|r| r.opt("o", v.as_ref()))));
        prop_assert_eq!(opt(None).unwrap(), enc(&With(|w: &mut KeyWriter| w.record(|_| Ok(())))).unwrap());
        prop_assert_ne!(opt(None).unwrap(), opt(Some(0)).unwrap());
        let unless = |v: f32| enc(&With(move |w: &mut KeyWriter| w.record(|r| r.unless("g", &v, &0.0))));
        prop_assert_eq!(unless(0.0).unwrap(), opt(None).unwrap());
        prop_assert_eq!(unless(-0.0).unwrap(), opt(None).unwrap());
        prop_assert_ne!(unless(0.25).unwrap(), opt(None).unwrap());

        // Kinds hash under separate contexts.
        let v = (small, &s);
        let both = With(move |w: &mut KeyWriter| w.record(|r| {
            r.field("n", &v.0)?;
            r.field("s", v.1)
        }));
        let kinds = [KeyKind::Evidence, KeyKind::HitSet, KeyKind::TeachSet, KeyKind::Interp, KeyKind::Render];
        let keys: BTreeSet<Hash32> = kinds.iter().map(|k| key_of(*k, &both).unwrap()).collect();
        prop_assert_eq!(keys.len(), kinds.len());
    }

    #[test]
    fn seed_tree_keys_ignore_locks_and_zero_rolls(root in any::<u64>(), a in common::aspect()) {
        let mut t = SeedTree::new(Seed(root));
        let k0 = enc(&t).unwrap();
        t.locked.insert(a);
        prop_assert_eq!(&enc(&t).unwrap(), &k0);
        t.rolls.insert(a, 0);
        prop_assert_eq!(&enc(&t).unwrap(), &k0);
        t.rolls.insert(a, 1);
        prop_assert_ne!(&enc(&t).unwrap(), &k0);
    }

    #[test]
    fn teach_set_ids_ignore_label_order(n in 0usize..8, salt in any::<u64>()) {
        let labels: Vec<TeachLabel> = (0..n)
            .map(|i| TeachLabel {
                take: TakeId::from_hash(Hash32([i as u8; 32])),
                at: TimelinePos(i as u64 * 1_000),
                role: RoleName::new(["kick", "snare", "hat_closed"][i % 3]).unwrap(),
            })
            .collect();
        let a = TeachSet { labels: labels.clone() }.id().unwrap();
        let b = TeachSet { labels: shuffled(&labels, salt) }.id().unwrap();
        prop_assert_eq!(a, b);
        if let Some(l) = labels.first() {
            let mut dup = labels.clone();
            dup.push(l.clone());
            prop_assert_eq!(TeachSet { labels: dup }.id(), Err(KeyError::DuplicateKey));
        }
    }
}

/// One value exercising every encoding rule.
fn sample_value() -> impl KeyHash {
    With(|w: &mut KeyWriter| {
        w.record(|r| {
            r.field("int", &-42i64)?;
            r.field("float", &-0.0f32)?;
            r.field("bool", &true)?;
            r.field("str", "takedown")?;
            r.field("bytes", &Hash32([7; 32]))?;
            r.field("list", &[1u8, 2, 3][..])?;
            r.field(
                "set",
                &["b", "a"]
                    .into_iter()
                    .map(String::from)
                    .collect::<BTreeSet<_>>(),
            )?;
            r.field(
                "map",
                &[
                    ("x".to_owned(), ParamValue::Float(0.5)),
                    ("n".to_owned(), ParamValue::Int(3)),
                ]
                .into_iter()
                .collect::<Params>(),
            )?;
            r.opt("absent", None::<&u8>)?;
            r.unless("reproducing", &0u32, &0)?;
            r.field(
                "variant",
                &With(|w: &mut KeyWriter| w.variant("splat", |r| r.field("k", &9u8))),
            )
        })
    })
}

fn frozen_classifier() -> ClassifierStamp {
    let mut params = BTreeMap::new();
    params.insert("k".to_owned(), ParamValue::Int(3));
    params.insert("refractory_ms".to_owned(), ParamValue::Float(45.0));
    params.insert("augment".to_owned(), ParamValue::Bool(true));
    ClassifierStamp {
        id: "takedown-drums-knn".to_owned(),
        version: 1,
        params,
    }
}

fn frozen_teach_set() -> TeachSet {
    TeachSet {
        labels: vec![
            TeachLabel {
                take: TakeId::from_hash(Hash32([1; 32])),
                at: TimelinePos(44_100),
                role: RoleName::new("kick").unwrap(),
            },
            TeachLabel {
                take: TakeId::from_hash(Hash32([1; 32])),
                at: TimelinePos(66_150),
                role: RoleName::new("snare").unwrap(),
            },
        ],
    }
}

fn frozen_track() -> ControlTrack {
    let r = rate(48_000);
    let len = 4_800; // 100 ms: 20 frames
    let frames = 20;
    let ramp = |lo: f32, step: f32| {
        (0..frames)
            .map(|i| lo + step * i as f32)
            .collect::<Vec<_>>()
    };
    ControlTrack::new(TrackParts {
        rate: r,
        len,
        take: Some(TakeId::from_hash(Hash32([1; 32]))),
        roles: vec![
            RoleName::new("kick").unwrap(),
            RoleName::new("zap").unwrap(),
        ],
        curves: Curves {
            pitch_st: ramp(57.0, 0.25),
            amp_db: ramp(-60.0, 2.0),
            bright: ramp(0.0, 0.05),
            breath: vec![-0.0; frames],
            vowel_x: ramp(-1.0, 0.1),
            vowel_y: vec![f32::from_bits(1); frames],
        },
        spans: vec![
            GateSpan {
                on: TimelinePos(240),
                off: TimelinePos(2_400),
                anchor: TimelinePos(200),
                vel: Velocity::new(90).unwrap(),
                link: Link::Detached,
            },
            GateSpan {
                on: TimelinePos(2_400),
                off: TimelinePos(4_800),
                anchor: TimelinePos(2_380),
                vel: Velocity::new(70).unwrap(),
                link: Link::Retrigger,
            },
        ],
        hits: vec![HitEvent {
            at: TimelinePos(100),
            anchor: TimelinePos(120),
            dur: 2_000,
            role: 1,
            vel: Velocity::new(127).unwrap(),
            bright: U7::new(64).unwrap(),
            breath: U7::new(0).unwrap(),
            mouth: U7::new(127).unwrap(),
        }],
    })
    .unwrap()
}

#[test]
fn frozen_keys() {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| out.push((format!("key/{k}"), v));
    let sample = sample_value();
    for (name, kind) in [
        ("evidence", KeyKind::Evidence),
        ("hit_set", KeyKind::HitSet),
        ("teach_set", KeyKind::TeachSet),
        ("interp", KeyKind::Interp),
        ("render", KeyKind::Render),
    ] {
        put(
            &format!("encoding/{name}"),
            key_of(kind, &sample).unwrap().to_string(),
        );
    }
    let teach = frozen_teach_set();
    let teach_id = teach.id().unwrap();
    put("teach_set", teach_id.to_string());
    let evidence = EvidenceKey::from_hash(Hash32([2; 32]));
    put(
        "hit_set",
        hit_set_key(evidence, &frozen_classifier(), Some(teach_id))
            .unwrap()
            .to_string(),
    );
    put(
        "hit_set/untaught",
        hit_set_key(evidence, &frozen_classifier(), None)
            .unwrap()
            .to_string(),
    );
    let mut tree = SeedTree::new(Seed(0x0123_4567_89AB_CDEF));
    tree.rolls.insert(Aspect::Mangle, 2);
    tree.roll_region(
        Aspect::Body,
        TimelineSpan::new(TimelinePos(0), TimelinePos(4_800)).unwrap(),
    );
    put(
        "seed_tree/encoding",
        key_of(KeyKind::Interp, &tree).unwrap().to_string(),
    );
    let track = frozen_track();
    put("content/control", track.content_id().to_string());
    let hits = HitSet {
        key: HitSetKey::from_hash(Hash32([3; 32])),
        roles: track.roles().to_vec(),
        hits: vec![Hit {
            onset: TimelinePos(120),
            span: TimelineSpan::new(TimelinePos(120), TimelinePos(2_120)).unwrap(),
            loudness_db: -12.5,
            brightness: 0.75,
            decay_db_per_s: -30.0,
            scores: vec![0.1, -0.0],
        }],
    };
    put("content/hit_set", hits.content_hash().to_string());

    // Evidence: what was asked for (the key) and what was produced (content).
    let take = TakeId::from_hash(Hash32([1; 32]));
    let provenance = frozen_provenance();
    let evidence_key = provenance.key(take).unwrap();
    put("evidence", evidence_key.to_string());
    put(
        "content/evidence",
        frozen_evidence(&provenance, take)
            .content_hash()
            .to_string(),
    );
    put("content/take", frozen_take_id().to_string());

    // Interp: one Lens source with every edit kind, one Imported source.
    let lens = frozen_lens_interp(take, evidence_key);
    put("interp/lens", lens.key().unwrap().to_string());
    put(
        "interp/imported",
        frozen_imported_interp(take).key().unwrap().to_string(),
    );

    // Render: the inputs a fresh render of the Lens source records.
    put(
        "render",
        frozen_render_inputs(lens.key().unwrap())
            .key()
            .unwrap()
            .to_string(),
    );
    check_frozen("key/", &out);
}

fn frozen_provenance() -> Provenance {
    let mut params = BTreeMap::new();
    params.insert("hop".to_owned(), ParamValue::Int(240));
    params.insert("fmin_hz".to_owned(), ParamValue::Float(50.0));
    params.insert("fmax_hz".to_owned(), ParamValue::Float(1_600.0));
    params.insert("median".to_owned(), ParamValue::Bool(true));
    params.insert("window".to_owned(), ParamValue::Text("hann".to_owned()));
    let mut models = BTreeMap::new();
    models.insert("pitch".to_owned(), Hash32([4; 32]));
    Provenance {
        analyzer: "takedown-evidence".to_owned(),
        version: 1,
        models,
        params,
    }
}

/// Two frames at 48 kHz: -0.0, a subnormal, NaN where it is allowed, a
/// missing column, and one onset.
fn frozen_evidence(provenance: &Provenance, take: TakeId) -> Evidence {
    let header = EvidenceHeader {
        key: provenance.key(take).unwrap(),
        take,
        take_rate: rate(48_000),
        take_frames: 480,
        provenance: provenance.clone(),
    };
    Evidence::new(
        header,
        vec![
            (Column::F0Hz0, vec![220.0, 0.0]),
            (Column::VoicingP, vec![0.9, -0.0]),
            (Column::CentroidHz, vec![1_500.0, f32::NAN]),
            (Column::CppDb, vec![f32::from_bits(1), f32::NAN]),
        ],
        vec![OnsetCandidate {
            at: TimelinePos(100),
            strength: 0.8,
            rise_db: 12.0,
        }],
    )
    .unwrap()
}

/// Ten stereo frames at 44.1 kHz, fed in uneven chunks.
fn frozen_take_id() -> TakeId {
    let pcm: Vec<f32> = (0..20).map(|i| (i as f32 - 10.0) / 16.0).collect();
    let mut h = TakeHasher::new(rate(44_100), 2);
    h.update(&pcm[..7]).unwrap();
    h.update(&pcm[7..]).unwrap();
    let (id, frames) = h.finish().unwrap();
    assert_eq!(frames, 10);
    id
}

fn frozen_lens_settings() -> LensSettings {
    LensSettings {
        palette: PaletteRef {
            name: "grit".to_owned(),
            id: PaletteId::from_hash(Hash32([5; 32])),
        },
        sliders: Sliders {
            mouth_amount: 0.5,
            fidelity: 1.0,
            mangle: 0.25,
            timing_feel: -0.0,
            register_st: -12,
            glide_retrigger: 0.75,
        },
        locks: [Slider::Mangle].into_iter().collect(),
    }
}

fn frozen_seeds() -> SeedTree {
    let mut tree = SeedTree::new(Seed(0x0123_4567_89AB_CDEF));
    tree.rolls.insert(Aspect::Body, 3);
    tree.locked.insert(Aspect::TimingFeel);
    tree.roll_region(
        Aspect::Mangle,
        TimelineSpan::new(TimelinePos(4_800), TimelinePos(9_600)).unwrap(),
    );
    tree
}

fn frozen_lens_interp(take: TakeId, evidence: EvidenceKey) -> Interpretation {
    let span = TimelineSpan::new(TimelinePos(480), TimelinePos(960)).unwrap();
    let hit = HitAnchor {
        at: TimelinePos(2_400),
        tol: 1_200,
    };
    let edits = [
        Edit::ForcePitch { span, st: 62.5 },
        Edit::SplitAt {
            at: TimelinePos(1_200),
            articulation: Articulation::Legato,
        },
        Edit::JoinAt {
            at: TimelinePos(3_600),
            tol: 960,
        },
        Edit::ForceRole {
            hit,
            role: RoleName::new("snare").unwrap(),
        },
        Edit::DeleteHit { hit },
        Edit::AddHit {
            hit,
            dur: 2_400,
            role: RoleName::new("kick").unwrap(),
            vel: Velocity::new(100).unwrap(),
        },
        Edit::MuteSpan { span },
    ];
    Interpretation {
        id: InterpId(0),
        name: "take 1".to_owned(),
        take: Some(take),
        source: ControlSource::Lens {
            evidence,
            hits: Some(HitSetKey::from_hash(Hash32([3; 32]))),
            edits: edits
                .into_iter()
                .enumerate()
                .map(|(i, edit)| EditEntry {
                    id: EditId(i as u32 + 1),
                    edit,
                })
                .collect(),
        },
        lens: frozen_lens_settings(),
        seeds: frozen_seeds(),
        midi: MidiBinding::default_export(),
    }
}

fn frozen_imported_interp(take: TakeId) -> Interpretation {
    Interpretation {
        id: InterpId(9),
        name: "edited in Reaper".to_owned(),
        take: Some(take),
        source: ControlSource::Imported {
            control: ControlId::from_hash(Hash32([6; 32])),
            file: Some(MidiFileId::from_hash(Hash32([7; 32]))),
        },
        lens: frozen_lens_settings(),
        seeds: frozen_seeds(),
        midi: MidiBinding::default_export(),
    }
}

fn frozen_render_inputs(interp: InterpKey) -> RenderInputs {
    RenderInputs {
        interp,
        evidence: Some(Hash32([8; 32])),
        hits: Some(Hash32([9; 32])),
        lens_version: 1,
        backends: vec![
            BackendStamp {
                id: "plaits".to_owned(),
                version: 1,
            },
            BackendStamp {
                id: "toy".to_owned(),
                version: 2,
            },
        ],
        contract: 1,
        rate: rate(48_000),
    }
}

/// JSON forms phase 2 builds on: hex ids and seeds as map keys, rates as map
/// keys, untagged params that keep Int and Float apart, strict newtypes.
#[test]
fn json_forms_of_phase1_types() {
    use takedown_core::canonical::{MidiBinding, MpeProfile};
    use takedown_core::musical::{TempoMap, TimeMap};
    use takedown_core::time::SampleRate;

    let take = TakeId::from_hash(Hash32([0xAB; 32]));
    let by_take: BTreeMap<TakeId, u8> = [(take, 1)].into_iter().collect();
    let text = serde_json::to_string(&by_take).unwrap();
    assert_eq!(text, format!("{{\"{}\":1}}", "ab".repeat(32)));
    assert_eq!(
        serde_json::from_str::<BTreeMap<TakeId, u8>>(&text).unwrap(),
        by_take
    );
    assert!(serde_json::from_str::<TakeId>(&format!("\"{}\"", "AB".repeat(32))).is_err());

    let by_rate: BTreeMap<SampleRate, u8> =
        [(rate(48_000), 1), (rate(44_100), 2)].into_iter().collect();
    let text = serde_json::to_string(&by_rate).unwrap();
    assert_eq!(text, r#"{"44100":2,"48000":1}"#);
    assert_eq!(
        serde_json::from_str::<BTreeMap<SampleRate, u8>>(&text).unwrap(),
        by_rate
    );
    assert!(serde_json::from_str::<BTreeMap<SampleRate, u8>>(r#"{"8":1}"#).is_err());

    let params: Params = [
        ("i".to_owned(), ParamValue::Int(1)),
        ("f".to_owned(), ParamValue::Float(1.0)),
        ("b".to_owned(), ParamValue::Bool(false)),
        ("t".to_owned(), ParamValue::Text("x".to_owned())),
        ("big".to_owned(), ParamValue::Int(i64::MAX)),
        ("tiny".to_owned(), ParamValue::Float(f32::from_bits(1))),
    ]
    .into_iter()
    .collect();
    let text = serde_json::to_string(&params).unwrap();
    assert_eq!(serde_json::from_str::<Params>(&text).unwrap(), params);
    assert!(serde_json::from_str::<ParamValue>("null").is_err());

    assert!(serde_json::from_str::<RoleName>("\"Kick\"").is_err());
    assert!(serde_json::from_str::<RoleName>(&format!("\"{}\"", "a".repeat(33))).is_err());
    assert!(serde_json::from_str::<Velocity>("0").is_err());
    assert!(serde_json::from_str::<U7>("128").is_err());
    assert!(serde_json::from_str::<TimelineSpan>("[5,4]").is_err());
    assert_eq!(
        serde_json::to_string(&TimelineSpan::new(TimelinePos(4), TimelinePos(5)).unwrap()).unwrap(),
        "[4,5]"
    );

    let binding = MidiBinding::default_export();
    let text = serde_json::to_string(&binding).unwrap();
    assert_eq!(serde_json::from_str::<MidiBinding>(&text).unwrap(), binding);
    assert_eq!(binding.profile, MpeProfile::default());
    assert!(serde_json::from_str::<TempoMap>("[]").is_err());
    assert!(serde_json::from_str::<TempoMap>(r#"[{"tick":5,"us_per_quarter":500000}]"#).is_err());
    assert!(
        serde_json::from_str::<TimeMap>(
            r#"{"ppq":0,"tempo":[{"tick":0,"us_per_quarter":500000}],"origin_tick":0}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<TimeMap>(
            r#"{"ppq":960,"tempo":[{"tick":0,"us_per_quarter":500000}],"origin_tick":0,"x":1}"#
        )
        .is_err()
    );
}
