//! Headless (HEAD-1..3): the same modules minus the render module (HEAD-2);
//! timeline driven; digest-verified. Runs in `cargo test` with no GPU.
use helix_assets::{MeshAssets, MeshId};
use helix_core::FIXED_DT;
use helix_ecs::{
    rng, ActionState, Assembled, Channel, CollisionEvent, Ctx, EngineBuilder, Hooks, Name,
    Registry, Scheduler, Stage, World,
};
use helix_scene::level;
use serde::Deserialize;

#[derive(Default)]
pub struct Timeline {
    pub entries: Vec<(u64, String, f32)>,
}

impl Timeline {
    pub fn push(&mut self, step: u64, action: &str, value: f32) {
        self.entries.push((step, action.to_string(), value));
    }
    pub fn apply(&self, step: u64, actions: &mut ActionState) {
        for (s, a, v) in &self.entries {
            if *s == step {
                actions.set(a, *v);
            }
        }
    }
}

pub struct Headless {
    pub world: World,
    pub registry: Registry,
    scheduler: Scheduler,
    hooks: Hooks,
    pub events: Channel<CollisionEvent>,
    pub actions: ActionState,
    pub cvars: helix_core::cvars::Cvars,
    step: u64,
}

impl Headless {
    pub fn from_level(source: &str) -> Result<Self, String> {
        Self::from_level_with_seed(source, rng::DEFAULT_SEED)
    }

    pub fn from_level_with_seed(source: &str, seed: u64) -> Result<Self, String> {
        let mut stub = MeshAssets::new();
        stub.insert("cube", MeshId(0), 0.87);
        let mut b = EngineBuilder::new().seed(seed);
        b.module(Box::new(crate::modules::CoreSimModule));
        b.module(Box::new(crate::modules::PillarPlazaModule));
        let Assembled { mut world, registry, scheduler, hooks, cvars, .. } =
            b.finish().map_err(|e| e.to_string())?;
        level::load(&mut world, &stub, source)?;
        let e = world.spawn(); // engine-owned RNG entity (RNG-1)
        world.insert(e, WorldRng::new(seed));
        world.insert(e, Name("__rng".into()));
        Ok(Self {
            world,
            registry,
            scheduler,
            hooks,
            events: Channel::new(),
            actions: ActionState::default(),
            cvars,
            step: 0,
        })
    }

    pub fn set_axis(&mut self, name: &str, value: f32) {
        self.actions.set(name, value);
    }

    /// One fixed step; events update per step here (EVT-7, headless side).
    pub fn step(&mut self) {
        let mut ctx = Ctx {
            world: &mut self.world,
            actions: &self.actions,
            events: &mut self.events,
            cvars: &mut self.cvars,
            dt: FIXED_DT,
            alpha: 1.0,
            interp: true,
        };
        self.scheduler.run_stage(Stage::Input, &mut ctx, &self.hooks);
        self.scheduler.run_stage(Stage::Sim, &mut ctx, &self.hooks);
        self.events.update();
        self.scheduler.run_stage(Stage::Late, &mut ctx, &self.hooks);
        self.world.tick += 1;
        self.step += 1;
    }

    pub fn steps(&mut self, n: u32, timeline: &Timeline) {
        for _ in 0..n {
            timeline.apply(self.step, &mut self.actions);
            self.step();
        }
    }

    pub fn hash(&self) -> u64 {
        self.registry.state_hash(&self.world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_scene::level::DEFAULT_LEVEL;

    /// [H] ECS-12/DETERM — identical timelines produce identical digests;
    /// divergent input diverges. No GPU required.
    #[test]
    fn determinism_and_divergence() {
        let mut t = Timeline::default();
        for s in 0..120 {
            t.push(s, "move_x", 1.0);
        }
        let (mut a, mut b) = (
            Headless::from_level(DEFAULT_LEVEL).unwrap(),
            Headless::from_level(DEFAULT_LEVEL).unwrap(),
        );
        a.steps(120, &t);
        b.steps(120, &t);
        assert_eq!(a.hash(), b.hash(), "same seed+timeline => identical worlds");

        let mut t2 = Timeline::default();
        for s in 0..120 {
            t2.push(s, "move_x", 0.75);
        }
        let mut c = Headless::from_level(DEFAULT_LEVEL).unwrap();
        c.steps(120, &t2);
        assert_ne!(a.hash(), c.hash(), "divergent input must diverge");
    }

    /// [H] FX-1/RNG-1 — VFX bursts are digest-visible and deterministic.
    #[test]
    fn particles_are_digest_deterministic() {
        let mut t = Timeline::default();
        for s in 0..180 {
            t.push(s, "move_x", 1.0);
            t.push(s, "move_y", 1.0);
        }
        let (mut a, mut b) = (
            Headless::from_level(DEFAULT_LEVEL).unwrap(),
            Headless::from_level(DEFAULT_LEVEL).unwrap(),
        );
        a.steps(180, &t);
        b.steps(180, &t);
        assert_eq!(a.hash(), b.hash(), "same seed+timeline => identical VFX state");
        let mut c = Headless::from_level_with_seed(DEFAULT_LEVEL, rng::DEFAULT_SEED + 1).unwrap();
        c.steps(180, &t);
        assert_ne!(a.hash(), c.hash(), "rng state must be digest-visible");
    }
}
