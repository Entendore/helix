//! Scenario DSL (SCEN-1 mini): RON drives Headless; asserts component fields
//! at the final step; returns the final digest for fixture pinning.
use serde::Deserialize;

use crate::components::Velocity;
use crate::headless::{Headless, Timeline};
use helix_ecs::{rng::DEFAULT_SEED, Name, Transform, World};
use helix_scene::level::DEFAULT_LEVEL;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    #[serde(default = "d_level")]
    pub level: String,
    pub steps: u64,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub timeline: Vec<(u64, String, f32)>,
    #[serde(default)]
    pub asserts: Vec<Assert>,
}
fn d_level() -> String {
    "default".into()
}

#[derive(Debug, Deserialize)]
pub struct Assert {
    pub at_step: Option<u64>,
    pub check: Check,
}

#[derive(Debug, Deserialize)]
pub struct Check {
    pub path: String,   // "transform.position.x" | "transform.scale.z" | "velocity.linear.z"
    pub entity: String, // RON `name`
    pub op: Op,
    pub value: f32,
    #[serde(default = "d_eps")]
    pub eps: f32,
}
fn d_eps() -> f32 {
    1e-3
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl Op {
    fn hold(&self, got: f32, want: f32, eps: f32) -> bool {
        match self {
            Op::Eq => (got - want).abs() <= eps,
            Op::Ne => (got - want).abs() > eps,
            Op::Lt => got < want,
            Op::Gt => got > want,
            Op::Le => got <= want,
            Op::Ge => got >= want,
        }
    }
    fn sym(&self) -> &'static str {
        match self {
            Op::Eq => "==",
            Op::Ne => "!=",
            Op::Lt => "<",
            Op::Gt => ">",
            Op::Le => "<=",
            Op::Ge => ">=",
        }
    }
}

/// Runs one scenario; returns per-assert report lines and the final digest.
pub fn run_with_digest(name: &str, src: &str) -> Result<(Vec<String>, u64), String> {
    let sc: Scenario = ron::from_str(src).map_err(|e| format!("{name}: parse error: {e}"))?;
    let level_src = match sc.level.as_str() {
        "default" => DEFAULT_LEVEL.to_string(),
        other => std::fs::read_to_string(other)
            .map_err(|e| format!("{name}: level '{other}': {e}"))?,
    };
    let mut sim = Headless::from_level_with_seed(&level_src, sc.seed.unwrap_or(DEFAULT_SEED))?;
    let mut tl = Timeline::default();
    for (s, a, v) in &sc.timeline {
        tl.push(*s, a, *v);
    }
    sim.steps(sc.steps as u32, &tl);

    let mut report = Vec::new();
    for a in &sc.asserts {
        if let Some(at) = a.at_step {
            if at != sc.steps {
                return Err(format!(
                    "{name}: at_step {at} != steps {} (mini evaluates at end)",
                    sc.steps
                ));
            }
        }
        let got = read_field(&sim.world, &a.check.entity, &a.check.path)?;
        let c = &a.check;
        if !c.op.hold(got, c.value, c.eps) {
            return Err(format!(
                "{name}: {} {} {} FAIL (got {got:.4})",
                c.path,
                c.op.sym(),
                c.value
            ));
        }
        report.push(format!("{name}: {} {} {} ok", c.path, c.op.sym(), c.value));
    }
    Ok((report, sim.hash()))
}

pub fn read_field(world: &World, entity_name: &str, path: &str) -> Result<f32, String> {
    let entity = world
        .query1::<Name>()
        .find(|(_, n)| n.0 == entity_name)
        .map(|(e, _)| e)
        .ok_or_else(|| format!("unknown entity '{entity_name}'"))?;
    let mut parts = path.split('.');
    let comp = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    let axis = |v: glam::Vec3, f: &str| match f {
        "x" => Some(v.x),
        "y" => Some(v.y),
        "z" => Some(v.z),
        _ => None,
    };
    match (comp, rest.as_slice()) {
        ("transform", ["position", f]) | ("transform", ["scale", f]) => {
            let t = world.get::<Transform>(entity).ok_or("no transform")?;
            axis(if rest[0] == "position" { t.position } else { t.scale }, f)
        }
        ("velocity", ["linear", f]) => {
            let v = world.get::<Velocity>(entity).ok_or("no velocity")?;
            axis(v.linear, f)
        }
        _ => Err(format!("unknown field path '{path}'")),
    }
    .ok_or_else(|| format!("unknown field path '{path}'"))
}
