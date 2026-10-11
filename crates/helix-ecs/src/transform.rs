//! Transform family (HIER-1): local Transform, computed GlobalTransform,
//! PrevGlobal for interpolation (INTP-1a), hierarchy link, debug name.
//! POD components are #[repr(C)] for bytemuck hashing/saving (ECS-13/FDET-4).
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use crate::world::Entity;

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        // SCN-2: default scale is ONE, never zero.
        Self { position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalTransform(pub Mat4);

#[derive(Clone, Copy, Debug)]
pub struct PrevGlobal(pub Mat4);

#[derive(Clone, Copy, Debug)]
pub struct Parent(pub Entity);

#[derive(Clone, Debug)]
pub struct Name(pub String);
