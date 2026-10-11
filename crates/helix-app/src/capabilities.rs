//! CAP-1 seed: probe results as read-only cvars + tier-default render.scale.
use helix_core::cvars::{Cvars, Value};
pub use helix_render::caps::{probe, Capabilities, Tier};

pub fn seed_cvars(cvars: &mut Cvars, caps: &Capabilities) {
    cvars.register_ro("gpu.tier", Value::Str(caps.tier.name().into()), "probed quality tier");
    cvars.register_ro("gpu.backend", Value::Str(caps.backend.clone()), "wgpu backend");
    cvars.register_ro("gpu.adapter", Value::Str(caps.adapter.clone()), "adapter name");
    cvars.register(
        "render.scale",
        Value::F32(caps.tier.render_scale()),
        "3D render scale (RES-1; UI stays native)",
    );
}
