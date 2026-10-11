//! helix-render — the presentation kernel (spec: Desktop profile). Consumes
//! ONLY extraction types (ARCH-2). Depth policy (ZB-1/2): reverse-Z +
//! Depth32Float by default (feature `reverse-z`); forward-Z is the tier-0
//! fallback. Nothing outside this crate branches on depth direction (ZB-3).
pub mod caps;
pub mod camera;
pub mod framegraph;
pub mod projection;
pub mod render_thread;
pub mod renderer;

pub use camera::Camera;
pub use framegraph::FrameGraph;
pub use render_thread::{ExtractedView, FrameData, RenderMessage, RenderProxy};
pub use renderer::{CoreRenderer, Frustum, Renderer};

#[cfg(feature = "reverse-z")]
pub const DEPTH_REVERSED: bool = true;
#[cfg(not(feature = "reverse-z"))]
pub const DEPTH_REVERSED: bool = false;

#[cfg(feature = "reverse-z")]
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
#[cfg(not(feature = "reverse-z"))]
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

#[cfg(feature = "reverse-z")]
pub const DEPTH_CLEAR: f32 = 0.0;
#[cfg(not(feature = "reverse-z"))]
pub const DEPTH_CLEAR: f32 = 1.0;

#[cfg(feature = "reverse-z")]
pub const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::GreaterEqual;
#[cfg(not(feature = "reverse-z"))]
pub const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::Less;
