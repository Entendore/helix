//! helix-scene — content components + the RON level format (spec Part V).
//! All level-authorable components (including gameplay tags like Player)
//! are DEFINED here so the loader can insert them while the dependency
//! arrow stays strictly sim -> scene (ARCH-3, one-way).
pub mod components;
pub mod level;

pub use components::{
    BaseColor, Collider, DirectionalLight, MeshColor, MeshRadius, Player, Spin, Velocity,
};
