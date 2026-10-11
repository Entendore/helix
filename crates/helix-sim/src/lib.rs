//! helix-sim — the demo game's simulation. ZERO presentation code in this
//! tree (no wgpu, no winit): `cargo tree -p helix-sim` proves ARCH-6 in CI.
pub mod components;
pub mod headless;
pub mod modules;
pub mod particles;
pub mod registry;
pub mod scen;
pub mod systems;
