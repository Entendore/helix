//! simlink — gameplay TAG components needed by the level format, defined at
//! the scene layer so the dependency arrow stays sim -> scene (one-way).
//! helix-sim uses these types directly for its systems.
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct Player;

#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Velocity { pub linear: Vec3 }

#[derive(Clone, Copy, Debug)]
pub struct Spin { pub axis: Vec3, pub speed: f32 }

#[derive(Clone, Copy, Debug)]
pub struct Collider { pub radius: f32 }
