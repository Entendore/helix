//! Presentation-side module (MOD-1): registers render cvars.
use helix_core::cvars::Value;
use helix_ecs::{EngineBuilder, EngineModule};

pub struct RenderModule;
impl EngineModule for RenderModule {
    fn id(&self) -> &'static str {
        "render"
    }
    fn deps(&self) -> &'static [&'static str] {
        &["core-sim"]
    }
    fn build(&self, b: &mut EngineBuilder) {
        b.cvar("render.exposure", Value::F32(0.0), "exposure in EV (POST-2)");
    }
}
