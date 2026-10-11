//! Camera service (presentation): lives in app state, feeds extraction.
use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    pub look_at: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        Self { position: Vec3::new(0.0, 7.0, 10.0), look_at: Vec3::ZERO }
    }

    /// RH view, WebGPU 0..1 depth, depth-direction aware (ZB-3).
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        crate::projection::view_proj(50f32.to_radians(), aspect, 0.1, 200.0, crate::DEPTH_REVERSED)
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}
