//! core-types §2.9, the part phase 1 implements: `MidiBinding` validation.
//! `quantise` and its properties arrive with the midi crate.

use takedown_core::canonical::{
    MAX_BEND_RANGE_ST, MIN_PHRASE_CHANNELS, MidiBinding, MpeProfile, MpeProfileError, RESERVED_CCS,
    RoleMap,
};
use takedown_core::control::Lane;
use takedown_core::hit::RoleName;

fn role(name: &str) -> RoleName {
    RoleName::new(name).expect("a valid role name")
}

fn with(f: impl FnOnce(&mut MpeProfile)) -> Result<(), MpeProfileError> {
    let mut p = MpeProfile::default();
    f(&mut p);
    p.validate()
}

#[test]
fn defaults_are_valid() {
    assert_eq!(RoleMap::gm_default().validate(), Ok(()));
    assert_eq!(MpeProfile::default().validate(), Ok(()));
    assert_eq!(MidiBinding::default_export().validate(), Ok(()));
}

#[test]
fn role_map_is_injective_with_a_mapped_fallback() {
    assert_eq!(
        with(|p| {
            p.roles.keys.insert(role("kick"), 128);
        }),
        Err(MpeProfileError::KeyRange(role("kick")))
    );
    assert_eq!(
        with(|p| {
            p.roles.keys.insert(role("sub"), 36);
        }),
        Err(MpeProfileError::SharedKey(36))
    );
    assert_eq!(
        with(|p| p.roles.fallback = role("cowbell")),
        Err(MpeProfileError::Fallback(role("cowbell")))
    );
    assert_eq!(
        with(|p| {
            p.roles.keys.insert(role("cowbell"), 127);
            p.roles.fallback = role("cowbell");
        }),
        Ok(())
    );
}

/// Every pair of channel ranges in 0..=16, against the rule written out
/// independently: both ordered inside the member channels 1..=15, disjoint,
/// and the phrase lane at least three wide.
#[test]
fn channel_ranges_exhaustively() {
    let mut accepted = 0;
    for a in 0..=16u8 {
        for b in 0..=16u8 {
            for c in 0..=16u8 {
                for d in 0..=16u8 {
                    let got = with(|p| {
                        p.phrase_channels = (a, b);
                        p.hit_channels = (c, d);
                    });
                    let member = |lo: u8, hi: u8| 1 <= lo && lo <= hi && hi <= 15;
                    let disjoint = b < c || d < a;
                    let wide = b >= a + MIN_PHRASE_CHANNELS - 1;
                    let ok = member(a, b) && member(c, d) && disjoint && wide;
                    assert_eq!(got.is_ok(), ok, "phrase {a}..={b}, hits {c}..={d}: {got:?}");
                    accepted += usize::from(ok);
                }
            }
        }
    }
    assert!(accepted > 1_000, "only {accepted} splits accepted");
    assert_eq!(
        with(|p| p.phrase_channels = (0, 3)),
        Err(MpeProfileError::Channels {
            lane: Lane::Phrase,
            range: (0, 3)
        }),
        "channel 0 is the zone's master channel"
    );
    assert_eq!(
        with(|p| p.hit_channels = (15, 4)),
        Err(MpeProfileError::Channels {
            lane: Lane::Hits,
            range: (15, 4)
        })
    );
    assert_eq!(
        with(|p| p.hit_channels = (3, 15)),
        Err(MpeProfileError::LanesOverlap)
    );
    assert_eq!(
        with(|p| p.phrase_channels = (1, 2)),
        Err(MpeProfileError::PhraseTooNarrow)
    );
    // The lanes may sit either way round, and the hit lane may be one channel.
    assert_eq!(
        with(|p| {
            p.phrase_channels = (13, 15);
            p.hit_channels = (1, 1);
        }),
        Ok(())
    );
}

#[test]
fn expression_ccs_are_distinct_plain_controllers() {
    for cc in 0..=255u8 {
        let got = with(|p| p.cc_bright = cc);
        let taken = [2, 16, 17].contains(&cc);
        let ok = cc <= 127 && !RESERVED_CCS.contains(&cc) && !taken;
        assert_eq!(got.is_ok(), ok, "CC {cc}: {got:?}");
        if taken {
            assert_eq!(got, Err(MpeProfileError::SharedCc(cc)));
        } else if !ok {
            assert_eq!(got, Err(MpeProfileError::Cc(cc)));
        }
    }
    assert_eq!(
        with(|p| p.cc_vowel = (16, 16)),
        Err(MpeProfileError::SharedCc(16))
    );
    assert_eq!(
        with(|p| p.cc_breath = 6),
        Err(MpeProfileError::Cc(6)),
        "data entry belongs to the RPN setup"
    );
}

#[test]
fn bend_range_fits_one_data_byte() {
    for b in 0..=255u8 {
        let got = with(|p| p.bend_range_st = b);
        if (1..=MAX_BEND_RANGE_ST).contains(&b) {
            assert_eq!(got, Ok(()));
        } else {
            assert_eq!(got, Err(MpeProfileError::BendRange(b)));
        }
    }
}
