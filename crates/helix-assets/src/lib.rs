//! helix-assets — the Handle bridge (spec bridge 1): pure CPU asset data.
//! No wgpu/winit here; this edge keeps the sim tree presentation-free
//! (ARCH-6) and lets headless builds stub the registry.
pub mod mesh;
pub mod registry;
pub mod types;

pub use mesh::{bounds, cube};
pub use registry::{MeshAssets, MeshEntry};
pub use types::{DrawCall, MeshId, Vertex};
