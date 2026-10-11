//! CPU geometry: unit cube (24 verts / 36 idx, CCW, hard normals) + bounds.
use crate::types::Vertex;

/// Bound-sphere radius (max vertex length) — culling input (REN-13).
pub fn bounds(vertices: &[Vertex]) -> f32 {
    vertices
        .iter()
        .map(|v| glam::Vec3::from(v.position).length())
        .fold(0.0f32, f32::max)
}

pub fn cube() -> (Vec<Vertex>, Vec<u16>) {
    const FACES: [([[f32; 3]; 4], [f32; 3]); 6] = [
        ([[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]], [0.0, 0.0, 1.0]),
        ([[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5]], [0.0, 0.0, -1.0]),
        ([[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5]], [-1.0, 0.0, 0.0]),
        ([[0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5]], [1.0, 0.0, 0.0]),
        ([[-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5]], [0.0, 1.0, 0.0]),
        ([[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]),
    ];
    let (mut vertices, mut indices) = (Vec::with_capacity(24), Vec::with_capacity(36));
    for (corners, normal) in FACES {
        let base = vertices.len() as u16;
        for c in corners {
            vertices.push(Vertex { position: c, normal });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}
