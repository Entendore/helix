//! Engine assembly (MOD-1..3, BOOT-1): modules are the ONLY extension path.
//! Topological build by deps; duplicate ids, unknown deps and cycles are
//! boot errors naming the module (ERR-4 style). Each phase logs one line.
use helix_core::cvars::Cvars;

use crate::registry::Registry;
use crate::rng::DEFAULT_SEED;
use crate::scheduler::{Hooks, Scheduler, Side, Stage, SystemFn};
use crate::world::World;

pub trait EngineModule {
    fn id(&self) -> &'static str;
    fn deps(&self) -> &'static [&'static str] {
        &[]
    }
    fn build(&self, b: &mut EngineBuilder);
}

pub struct EngineBuilder {
    pub world: World,
    registry: Registry,
    pub scheduler: Scheduler,
    pub hooks: Hooks,
    pub cvars: Cvars,
    pub seed: u64,
    ids: Vec<&'static str>,
    pending: Vec<Box<dyn EngineModule>>,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            registry: Registry::new(),
            scheduler: Scheduler::new(),
            hooks: Hooks::default(),
            cvars: Cvars::default(),
            seed: DEFAULT_SEED,
            ids: vec![],
            pending: vec![],
        }
    }

    /// Declares a module; build is deferred to `finish` so the dependency
    /// graph is validated before any side effects (MOD-3).
    pub fn module(&mut self, m: Box<dyn EngineModule>) -> &mut Self {
        self.ids.push(m.id());
        self.pending.push(m);
        self
    }

    pub fn seed(&mut self, seed: u64) -> &mut Self {
        self.seed = seed;
        self
    }
    pub fn system(&mut self, stage: Stage, f: SystemFn) -> &mut Self {
        self.scheduler.add(stage, f);
        self
    }
    pub fn hook(&mut self, stage: Stage, side: Side, f: SystemFn) -> &mut Self {
        self.hooks.add(stage, side, f);
        self
    }
    pub fn cvar(&mut self, name: &str, v: helix_core::cvars::Value, doc: &'static str) -> &mut Self {
        self.cvars.register(name, v, doc);
        self
    }
    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }
    pub fn cvars_mut(&mut self) -> &mut Cvars {
        &mut self.cvars
    }

    /// BOOT-1: ordered phases, one log line each; aborts before dependents.
    pub fn finish(mut self) -> Result<Assembled, String> {
        log::info!("boot: {} modules declared", self.ids.len());
        let mut seen = std::collections::HashSet::new();
        for id in &self.ids {
            if !seen.insert(*id) {
                return Err(format!("boot: duplicate module id '{id}'"));
            }
        }
        let order = topo(&self.ids, &self.pending)?;
        for i in order {
            let m = &self.pending[i];
            log::info!("boot: module '{}' building", m.id());
            m.build(&mut self);
        }
        log::info!("boot: schedule built ({} systems)", self.scheduler.len());
        log::info!("boot: registry holds {} components", self.registry.len());
        Ok(Assembled {
            world: self.world,
            registry: self.registry,
            scheduler: self.scheduler,
            hooks: self.hooks,
            cvars: self.cvars,
            seed: self.seed,
        })
    }
}

pub struct Assembled {
    pub world: World,
    pub registry: Registry,
    pub scheduler: Scheduler,
    pub hooks: Hooks,
    pub cvars: Cvars,
    pub seed: u64,
}

fn topo(ids: &[&'static str], modules: &[Box<dyn EngineModule>]) -> Result<Vec<usize>, String> {
    #[derive(Clone, Copy, PartialEq)]
    enum S {
        New,
        Visiting,
        Done,
    }
    let mut state = vec![S::New; ids.len()];
    let mut order = Vec::with_capacity(ids.len());

    fn visit(
        i: usize,
        ids: &[&'static str],
        modules: &[Box<dyn EngineModule>],
        state: &mut Vec<S>,
        order: &mut Vec<usize>,
    ) -> Result<(), String> {
        match state[i] {
            S::Done => Ok(()),
            S::Visiting => Err(format!("boot: module dependency cycle involving '{}'", ids[i])),
            S::New => {
                state[i] = S::Visiting;
                for dep in modules[i].deps() {
                    let j = ids
                        .iter()
                        .position(|x| x == dep)
                        .ok_or_else(|| {
                            format!("boot: module '{}' depends on unknown module '{dep}'", ids[i])
                        })?;
                    visit(j, ids, modules, state, order)?;
                }
                state[i] = S::Done;
                order.push(i);
                Ok(())
            }
        }
    }

    for i in 0..ids.len() {
        visit(i, ids, modules, &mut state, &mut order)?;
    }
    Ok(order) // deps finish before dependents => build order
}

#[cfg(test)]
mod tests {
    use super::*;

    struct A;
    impl EngineModule for A {
        fn id(&self) -> &'static str {
            "a"
        }
        fn build(&self, _: &mut EngineBuilder) {}
    }
    struct B;
    impl EngineModule for B {
        fn id(&self) -> &'static str {
            "b"
        }
        fn deps(&self) -> &'static [&'static str] {
            &["a"]
        }
        fn build(&self, _: &mut EngineBuilder) {}
    }
    struct LoopM;
    impl EngineModule for LoopM {
        fn id(&self) -> &'static str {
            "looped"
        }
        fn deps(&self) -> &'static [&'static str] {
            &["looped"]
        }
        fn build(&self, _: &mut EngineBuilder) {}
    }
    struct Unknown;
    impl EngineModule for Unknown {
        fn id(&self) -> &'static str {
            "u"
        }
        fn deps(&self) -> &'static [&'static str] {
            &["ghost"]
        }
        fn build(&self, _: &mut EngineBuilder) {}
    }

    /// [T] MOD-3 — cycles and unknown deps are boot errors; valid assembles.
    #[test]
    fn mod_graph_validation() {
        let mut ok = EngineBuilder::new();
        ok.module(Box::new(B)).module(Box::new(A));
        assert!(ok.finish().is_ok());

        let mut bad = EngineBuilder::new();
        bad.module(Box::new(LoopM));
        assert!(bad.finish().is_err());

        let mut ghost = EngineBuilder::new();
        ghost.module(Box::new(Unknown));
        assert!(ghost.finish().is_err());
    }
}
