//! Game modules (MOD-1): the ONLY registration path.
use helix_core::cvars::Value;
use helix_ecs::{EngineBuilder, EngineModule, Stage};

use crate::registry::register_all;
use crate::systems;

/// Engine-level sim: registry, hierarchy, prev-pose capture, engine cvars.
pub struct CoreSimModule;
impl EngineModule for CoreSimModule {
    fn id(&self) -> &'static str {
        "core-sim"
    }
    fn build(&self, b: &mut EngineBuilder) {
        register_all(b.registry_mut());
        b.system(Stage::Sim, systems::capture_prev); // INTP-1a: FIRST in Sim
        b.system(Stage::Late, helix_ecs::hierarchy::propagate_transforms);
        b.cvar("fx.max_particles", Value::U32(256), "particle cap per pool");
    }
}

/// The demo game: input, movement, collision, reactions, spin, FX decay.
pub struct PillarPlazaModule;
impl EngineModule for PillarPlazaModule {
    fn id(&self) -> &'static str {
        "pillar-plaza"
    }
    fn deps(&self) -> &'static [&'static str] {
        &["core-sim"]
    }
    fn build(&self, b: &mut EngineBuilder) {
        b.system(Stage::Input, systems::player_input);
        b.system(Stage::Sim, systems::integrate_movement);
        b.system(Stage::Sim, systems::resolve_collisions);
        b.system(Stage::Sim, systems::collision_reactions);
        b.system(Stage::Sim, systems::spin);
        b.system(Stage::Sim, systems::update_flashes);
        b.system(Stage::Sim, systems::update_particles);
    }
}
