//! RON level loading (SCN-1..5): declarative doc -> spawns. Pass 1 spawns +
//! attaches; pass 2 resolves parents BY NAME with forward references.
//! Hard-error taxonomy (SCN-4): parse failure, unknown mesh, duplicate
//! name, unknown parent, self-parent, cycle.
use std::collections::HashMap;

use glam::{Quat, Vec3};
use helix_assets::{MeshAssets, MeshId};
use helix_ecs::{Parent, Transform, World};
use serde::Deserialize;

use crate::components::{
    BaseColor, Collider, DirectionalLight, MeshColor, MeshRadius, Player, Spin, Velocity,
};

pub const DEFAULT_LEVEL: &str = include_str!("../../../assets/level.ron");

#[derive(Debug, Deserialize)]
pub struct LevelDoc {
    #[serde(default)]
    pub name: String,
    pub entities: Vec<EntityDoc>,
}

#[derive(Debug, Deserialize)]
pub struct EntityDoc {
    pub name: Option<String>,
    pub parent: Option<String>,
    pub transform: Option<TransformDoc>,
    pub mesh: Option<String>,
    pub color: Option<[f32; 4]>,
    pub player: Option<()>,
    pub velocity: Option<()>,
    pub spin: Option<SpinDoc>,
    pub collider: Option<ColliderDoc>,
    pub light: Option<LightDoc>,
}

#[derive(Debug, Deserialize)]
pub struct TransformDoc {
    #[serde(default)]
    pub pos: [f32; 3],
    pub rot: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

#[derive(Debug, Deserialize)]
pub struct SpinDoc {
    pub axis: [f32; 3],
    pub speed: f32,
}

#[derive(Debug, Deserialize)]
pub struct ColliderDoc {
    pub radius: f32,
}

#[derive(Debug, Deserialize)]
pub struct LightDoc {
    pub direction: [f32; 3],
    #[serde(default = "d_one")]
    pub intensity: f32,
    #[serde(default = "d_white")]
    pub color: [f32; 3],
}
fn d_one() -> f32 {
    1.0
}
fn d_white() -> [f32; 3] {
    [1.0; 3]
}

pub fn load(world: &mut World, assets: &MeshAssets, source: &str) -> Result<(), String> {
    let doc: LevelDoc = ron::from_str(source).map_err(|e| format!("level parse error: {e}"))?;
    let total = doc.entities.len();
    let mut named: HashMap<String, helix_ecs::Entity> = HashMap::new();
    let mut refs: Vec<(helix_ecs::Entity, String)> = Vec::new();

    for edoc in &doc.entities {
        // ---- PASS 1: spawn + attach ----
        let e = world.spawn();
        let (pos, rot, scale) = match &edoc.transform {
            Some(t) => (
                Vec3::from(t.pos),
                t.rot.map(Quat::from_array).unwrap_or(Quat::IDENTITY),
                t.scale.map(Vec3::from).unwrap_or(Vec3::ONE),
            ),
            None => Default::default(),
        };
        world.insert(e, Transform { position: pos, rotation: rot, scale });

        if let Some(mesh) = &edoc.mesh {
            let (id, radius) = assets.get(mesh)?;
            world.insert(e, id);
            world.insert(e, MeshRadius(radius));
        }
        if let Some(c) = edoc.color {
            world.insert(e, MeshColor(c));
            world.insert(e, BaseColor(c));
        }
        if edoc.player.is_some() {
            world.insert(e, Player);
        }
        if edoc.velocity.is_some() {
            world.insert(e, Velocity::default());
        }
        if let Some(s) = &edoc.spin {
            world.insert(e, Spin { axis: Vec3::from(s.axis), speed: s.speed });
        }
        if let Some(c) = &edoc.collider {
            world.insert(e, Collider { radius: c.radius });
        }
        if let Some(l) = &edoc.light {
            world.insert(e, DirectionalLight {
                direction: Vec3::from(l.direction).normalize_or_zero(),
                color: l.color,
                intensity: l.intensity,
            });
        }
        if let Some(n) = &edoc.name {
            if named.insert(n.clone(), e).is_some() {
                return Err(format!("duplicate entity name '{n}'"));
            }
            world.insert(e, helix_ecs::Name(n.clone()));
        }
        if let Some(p) = &edoc.parent {
            refs.push((e, p.clone()));
        }
    }

    // ---- PASS 2: resolve parents (forward refs OK) ----
    let links = refs
        .into_iter()
        .map(|(child, n)| {
            let parent = named
                .get(&n)
                .copied()
                .ok_or_else(|| format!("unknown parent '{n}'"))?;
            if parent == child {
                return Err("an entity cannot be its own parent".to_string());
            }
            Ok((child, parent))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Cycle guard: walk every chain to its root before inserting links (HIER-6).
    {
        let map: HashMap<u32, u32> = links.iter().map(|(c, p)| (c.index, p.index)).collect();
        for (child, _) in &links {
            let (mut path, mut cur) = (vec![child.index], child.index);
            while let Some(&next) = map.get(&cur) {
                if path.contains(&next) {
                    return Err("parent cycle detected in level".to_string());
                }
                path.push(next);
                cur = next;
            }
        }
    }
    for (child, parent) in links {
        world.insert(child, Parent(parent));
    }

    log::info!("loaded level '{}': {total} entities", doc.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use helix_assets::MeshAssets;

    /// [T] SCN-1..4 — default level loads; parents resolve; counts hold.
    #[test]
    fn default_level_loads() {
        let mut stub = MeshAssets::new();
        stub.insert("cube", MeshId(0), 0.87);
        let mut w = World::new();
        crate::level::load(&mut w, &stub, crate::level::DEFAULT_LEVEL).unwrap();
        assert_eq!(w.query1::<Transform>().count(), 12);
        assert_eq!(w.query1::<Parent>().count(), 5);
        assert_eq!(w.query1::<DirectionalLight>().count(), 1);
        assert_eq!(w.query1::<Player>().count(), 1);
        assert_eq!(w.query1::<Collider>().count(), 5); // 4 pillars + player
    }
}
