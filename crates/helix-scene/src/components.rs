//! Level-authorable components: everything a level document can attach.
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct MeshColor(pub [f32; 4]);

/// Flash decay restores toward this (EVT consumer output path).
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct BaseColor(pub [f32; 4]);

/// Mesh bound-sphere radius, world-scaled at extraction (REN-13).
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct MeshRadius(pub f32);

/// LIT-1 (directional): direction points surface->sun; normalized at load.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
}

// ---- gameplay tags authored by the level format (see crate docs) ----

#[derive(Clone, Copy, Debug, Default)]
pub struct Player;

#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Velocity {
    pub linear: Vec3,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Spin {
    pub axis: Vec3,
    pub speed: f32,
}

/// Static xz circle (COL-1; superseded by helix-physics at P2).
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Collider {
    pub radius: f32,
}
