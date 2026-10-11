//! Scheduler (ECS-9/10) + slim Ctx (ARCH-1: simulation services only — no
//! camera, no devices, no presentation types). Hooks (FRM-1/2) run outside
//! the stage's system list and never count in ECS-9 ordering.
use helix_core::cvars::Cvars;

use crate::events::{Channel, CollisionEvent};
use crate::world::World;

/// ACT-4: pure action data. Devices parse it in helix-app; headless
/// timelines write it directly (REP-1). Sim never sees raw keys.
#[derive(Default, Clone)]
pub struct ActionState {
    pub values: std::collections::HashMap<String, f32>,
}

impl ActionState {
    pub fn set(&mut self, name: &str, value: f32) {
        self.values.insert(name.to_string(), value.clamp(-1.0, 1.0));
    }
    /// ACT-3: a missing axis reads 0 — never panics.
    pub fn axis(&self, name: &str) -> f32 {
        self.values.get(name).copied().unwrap_or(0.0)
    }
}

pub struct Ctx<'a> {
    pub world: &'a mut World,
    pub actions: &'a ActionState,
    pub events: &'a mut Channel<CollisionEvent>,
    pub cvars: &'a mut Cvars,
    /// FIXED_DT in Input/Sim; the measured frame delta in Late (TIME-1/2).
    pub dt: f32,
    pub alpha: f32,
    pub interp: bool,
}

pub type SystemFn = fn(&mut Ctx);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Input,
    Sim,
    Late,
}

/// FRM-1/2: before/after-stage hooks.
#[derive(Default)]
pub struct Hooks {
    before: [Vec<SystemFn>; 3],
    after: [Vec<SystemFn>; 3],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

impl Hooks {
    pub fn add(&mut self, stage: Stage, side: Side, f: SystemFn) {
        let list = match (stage, side) {
            (Stage::Input, Side::Before) => &mut self.before[0],
            (Stage::Sim, Side::Before) => &mut self.before[1],
            (Stage::Late, Side::Before) => &mut self.before[2],
            (Stage::Input, Side::After) => &mut self.after[0],
            (Stage::Sim, Side::After) => &mut self.after[1],
            (Stage::Late, Side::After) => &mut self.after[2],
        };
        list.push(f);
    }
    fn run(&self, stage: Stage, side: Side, ctx: &mut Ctx) {
        let list = match (stage, side) {
            (Stage::Input, Side::Before) => &self.before[0],
            (Stage::Sim, Side::Before) => &self.before[1],
            (Stage::Late, Side::Before) => &self.before[2],
            (Stage::Input, Side::After) => &self.after[0],
            (Stage::Sim, Side::After) => &self.after[1],
            (Stage::Late, Side::After) => &self.after[2],
        };
        for f in list {
            f(ctx);
        }
    }
}

#[derive(Default)]
pub struct Scheduler {
    input: Vec<SystemFn>,
    sim: Vec<SystemFn>,
    late: Vec<SystemFn>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, stage: Stage, f: SystemFn) {
        match stage {
            Stage::Input => self.input.push(f),
            Stage::Sim => self.sim.push(f),
            Stage::Late => self.late.push(f),
        }
    }

    pub fn len(&self) -> usize {
        self.input.len() + self.sim.len() + self.late.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ECS-9: registration order within a stage; hooks run outside the list.
    pub fn run_stage(&self, stage: Stage, ctx: &mut Ctx, hooks: &Hooks) {
        hooks.run(stage, Side::Before, ctx);
        let list = match stage {
            Stage::Input => &self.input,
            Stage::Sim => &self.sim,
            Stage::Late => &self.late,
        };
        for f in list {
            f(ctx);
        }
        hooks.run(stage, Side::After, ctx);
    }
}
