//! Hierarchy (HIER-3..6): depth-sorted propagation with graceful degrade.
//! Interpolation (INTP-2/3a): decomposed lerp/slerp with teleport snap.
use std::collections::{HashMap, HashSet};

use glam::Mat4;

use crate::scheduler::Ctx;
use crate::transform::{GlobalTransform, Parent, Transform};
use crate::world::World;

/// Stage::Late — recomputes every GlobalTransform; parents compose first.
pub fn propagate_transforms(ctx: &mut Ctx) {
    propagate(ctx.world);
}

pub fn propagate(world: &mut World) {
    let locals: Vec<(crate::world::Entity, Transform)> =
        world.query1::<Transform>().map(|(e, t)| (e, *t)).collect();
    let parents: HashMap<u32, crate::world::Entity> =
        world.query1::<Parent>().map(|(e, p)| (e.index, p.0)).collect();

    fn depth(
        start: u32,
        parents: &HashMap<u32, crate::world::Entity>,
        path: &mut HashSet<u32>,
    ) -> usize {
        let mut d = 0;
        let mut cur = Some(start);
        while let Some(i) = cur {
            if !path.insert(i) {
                break; // cycle guard (loader rejects cycles; defense in depth)
            }
            cur = parents.get(&i).map(|p| p.index);
            if cur.is_some() {
                d += 1;
            }
        }
        d
    }

    let mut order: Vec<(usize, crate::world::Entity, Transform)> = locals
        .into_iter()
        .map(|(e, t)| (depth(e.index, &parents, &mut HashSet::new()), e, t))
        .collect();
    order.sort_by_key(|(d, _, _)| *d); // stable: parents before children

    for (_, entity, local) in order {
        let local_mat =
            Mat4::from_scale_rotation_translation(local.scale, local.rotation, local.position);
        let global = parents
            .get(&entity.index)
            .copied()
            .filter(|p| world.is_alive(*p)) // HIER-5: dangling parent => root
            .and_then(|p| world.get::<GlobalTransform>(p).map(|g| g.0))
            .map(|pm| pm * local_mat)
            .unwrap_or(local_mat);
        world.insert(entity, GlobalTransform(global));
    }
}

/// INTP-2/3a: translation lerp + shortest-arc slerp + scale lerp; a missing
/// prev pose or a translation delta >= snap_dist snaps to `curr`.
pub fn interpolate_pose(prev: Option<Mat4>, curr: Mat4, alpha: f32, snap_dist: f32) -> Mat4 {
    let Some(prev) = prev else { return curr };
    if (curr.w_axis.truncate() - prev.w_axis.truncate()).length() >= snap_dist {
        return curr;
    }
    let (ps, pr, pt) = prev.to_scale_rotation_translation();
    let (cs, cr, ct) = curr.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(ps.lerp(cs, alpha), pr.slerp(cr, alpha), pt.lerp(ct, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Entity as E;
    use glam::Vec3;

    /// [T] HIER-3/4 — out-of-order chain composes parent-before-child.
    #[test]
    fn out_of_order_chain() {
        let mut w = World::new();
        let (gc, c, p) = (w.spawn(), w.spawn(), w.spawn());
        for (e, x) in [(p, 10.0f32), (c, 1.0), (gc, 1.0)] {
            w.insert(e, Transform { position: Vec3::new(x, 0.0, 0.0), ..Default::default() });
        }
        w.insert(c, Parent(p));
        w.insert(gc, Parent(c));
        propagate(&mut w);
        let g = w.get::<GlobalTransform>(gc).unwrap().0.w_axis.truncate();
        assert_eq!(g, Vec3::new(12.0, 0.0, 0.0));
    }

    /// [T] HIER-5 — dangling parent degrades to root, no panic.
    #[test]
    fn dangling_parent_is_root() {
        let mut w = World::new();
        let dead = w.spawn();
        let child = w.spawn();
        w.insert(child, Transform { position: Vec3::X, ..Default::default() });
        w.insert(child, Parent(dead));
        w.despawn(dead);
        propagate(&mut w);
        let g = w.get::<GlobalTransform>(child).unwrap().0.w_axis.truncate();
        assert_eq!(g, Vec3::X);
        let _ = E { index: 0, generation: 0 };
    }

    /// [T] INTP-3a — teleport snaps; missing prev snaps.
    #[test]
    fn interp_snap() {
        let a = Mat4::from_translation(Vec3::ZERO);
        let b = Mat4::from_translation(Vec3::new(50.0, 0.0, 0.0));
        assert_eq!(interpolate_pose(Some(a), b, 0.5, 5.0), b);
        assert_eq!(interpolate_pose(None, a, 0.5, 5.0), a);
    }
}
