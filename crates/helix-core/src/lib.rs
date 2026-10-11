//! helix-core — Tier 0 substrate (spec §A2): math re-export, hash
//! primitives, config, cvars. Depends on nothing engine-side; every crate
//! may use it. API tier of every item here: 0 or 1 (API-4/DOC-1).
pub mod cvars;
pub mod config;
pub mod hash;

/// TIME-1: the one place the fixed step is defined.
pub const FIXED_DT: f32 = 1.0 / 60.0;

pub use glam;
pub use hash::{fnv1a, mix_u64};
