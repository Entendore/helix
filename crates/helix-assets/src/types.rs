//! Shared vertex/draw types. helix-render defines the GPU vertex LAYOUT for
//! this Vertex; assets stays free of wgpu types (ARCH-1/ARCH-2).
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// Opaque mesh handle — components hold this, never GPU state (ARCH-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(pub u32);

/// One draw — the extraction snapshot unit (REN-9, plain data, Send).
#[derive(Clone, Copy, Debug)]
pub struct DrawCall {
    pub mesh: MeshId,
    pub model: Mat4,
    pub color: [f32; 4],
}
