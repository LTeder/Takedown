//! Canonical objects for Takedown: time, ids, takes, evidence, control tracks,
//! interpretations, layers, projects and the seed tree.
//!
//! See `docs/MASTER-PLAN.md` section 4.1 and `docs/design/core-types.md`.

#![forbid(unsafe_code)]

pub mod canonical;
pub mod control;
pub mod edit;
pub mod evidence;
pub mod hit;
pub mod ids;
pub mod interp;
pub mod key;
pub mod layer;
pub mod lens;
pub mod migrate;
pub mod musical;
pub mod params;
pub mod project;
pub mod seed;
pub mod strict;
pub mod take;
pub mod time;
pub mod views;
