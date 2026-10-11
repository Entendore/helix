//! Gameplay systems (staged, snapshot-mutate per ECS-7).
use glam::{Quat, Vec2, Vec3};
use helix_ecs::{
    Channel, CollisionEvent, Ctx, Entity, GlobalTransform, PrevGlobal, Transform, World, WorldRng,
};
use helix_scene::{BaseColor, Collider, MeshColor, Player, Spin, Velocity};

use crate::components::{FLASH_TIME, LEVEL_HALF_EXTENT, PLAYER_SPEED};
use crate::particles::Particles;

/// Stage::Input — ACT-4: actions only, never devices.
pub fn player_input(ctx: &mut Ctx) {
    let mut dir = Vec3::new(ctx.actions.axis("move_x"), 0.0, ctx.actions.axis("move_y"));
    if dir.length_squared() > 1.0 {
        dir = dir.normalize();
    }
    let dir = dir * PLAYER_SPEED;
    let players: Vec<Entity> = ctx.world.query1::<Player>().map(|(e, _)| e).collect();
    for e in players {
        if let Some(v) = ctx.world.get_mut::<Velocity>(e) {
            v.linear = dir;
        }
    }
}

/// Stage::Sim (FIRST) — INTP-1a: snapshot start-of-step poses.
pub fn capture_prev(ctx: &mut Ctx) {
    let snap: Vec<(Entity, glam::Mat4)> =
        ctx.world.query1::<GlobalTransform>().map(|(e, g)| (e, g.0)).collect();
    for (e, g) in snap {
        ctx.world.insert(e, PrevGlobal(g));
    }
}

/// Stage::Sim — integrate local transforms + level clamp.
pub fn integrate_movement(ctx: &mut Ctx) {
    let movers: Vec<(Entity, Vec3)> = ctx
        .world
        .query2::<Transform, Velocity>()
        .map(|(e, _t, v)| (e, v.linear))
        .collect();
    for (e, vel) in movers {
        if vel == Vec3::ZERO {
            continue;
        }
        if let Some(t) = ctx.world.get_mut::<Transform>(e) {
            t.position += vel * ctx.dt;
            t.position.x = t.position.x.clamp(-LEVEL_HALF_EXTENT, LEVEL_HALF_EXTENT);
            t.position.z = t.position.z.clamp(-LEVEL_HALF_EXTENT, LEVEL_HALF_EXTENT);
        }
    }
}

/// Stage::Sim — COL-2 depenetration + slide; deterministic event order.
pub fn resolve_collisions(ctx: &mut Ctx) {
    let Ctx { world, events, .. } = ctx;
    resolve(world, events);
}

pub fn resolve(world: &mut World, events: &mut Channel<CollisionEvent>) {
    let obstacles: Vec<(Entity, Vec3, f32)> = world
        .query2::<Transform, Collider>()
        .map(|(e, t, c)| (e, t.position, c.radius))
        .collect();
    let movers: Vec<(Entity, Vec3, Vec3, f32)> = world
        .query3::<Transform, Velocity, Collider>()
        .map(|(e, t, v, c)| (e, t.position, v.linear, c.radius))
        .collect();

    let mut hits: Vec<(u32, u32)> = Vec::new();
    for (entity, pos, vel, radius) in movers {
        let (mut pos, mut vel) = (pos, vel);
        for (oe, opos, orad) in &obstacles {
            if oe.index == entity.index {
                continue; // don't collide with yourself
            }
            let d = Vec2::new(pos.x - opos.x, pos.z - opos.z);
            let min = radius + *orad;
            let dsq = d.length_squared();
            if dsq >= min * min {
                continue;
            }
            let (n, dist) = if dsq > 1e-8 {
                let dd = dsq.sqrt();
                (d / dd, dd)
            } else {
                (Vec2::X, 0.0) // COL-3: degenerate push along +x
            };
            let push = min - dist;
            pos.x += n.x * push;
            pos.z += n.z * push;
            let into = vel.x * n.x + vel.z * n.z;
            if into < 0.0 {
                vel.x -= n.x * into; // slide: kill into-surface velocity
                vel.z -= n.z * into;
            }
            hits.push((entity.index, oe.index));
        }
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = pos;
        }
        if let Some(v) = world.get_mut::<Velocity>(entity) {
            v.linear = vel;
        }
    }
    hits.sort_unstable(); // DETERM-1: explicit order for the event stream
    hits.dedup();
    for (mi, oi) in hits {
        events.send(CollisionEvent {
            mover: Entity { index: mi, generation: 0 },
            obstacle: Entity { index: oi, generation: 0 },
        });
    }
}

/// Stage::Sim — EVT consumer: white flash + deterministic FX burst (FX-1).
pub fn collision_reactions(ctx: &mut Ctx) {
    let cap = match ctx.cvars.get("fx.max_particles") {
        Some(helix_core::cvars::Value::U32(n)) => {
            (*n as usize).min(crate::particles::MAX_PARTICLES)
        }
        _ => 256,
    };
    let movers: Vec<Entity> = ctx.events.read().map(|ev| ev.mover).collect();
    let rng_e = ctx.world.query1::<WorldRng>().map(|(e, _)| e).next();
    for e in movers {
        match ctx.world.get_mut::<Flash>(e) {
            Some(f) => f.t = FLASH_TIME,
            None => ctx.world.insert(e, Flash { t: FLASH_TIME }),
        }
        let Some(rng_e) = rng_e else { continue };
        if ctx.world.get::<Particles>(e).is_none() {
            ctx.world.insert(e, Particles::default());
        }
        let at = ctx.world.get::<Transform>(e).map(|t| t.position).unwrap_or_default();
        let samples: Vec<[f32; 4]> = {
            let rng = ctx.world.get_mut::<WorldRng>(rng_e).unwrap();
            (0..24)
                .map(|_| [rng.signed_f32(), rng.f32(), rng.signed_f32(), rng.f32()])
                .collect()
        };
        if let Some(p) = ctx.world.get_mut::<Particles>(e) {
            p.burst(at, &samples, cap);
        }
    }
}

/// Flash timer (transient: not saved, not hashed — event-driven decay).
#[derive(Clone, Copy, Debug)]
pub struct Flash {
    pub t: f32,
}

/// Stage::Sim — flash decay toward base color; removal after iteration.
pub fn update_flashes(ctx: &mut Ctx) {
    let rows: Vec<(Entity, f32, [f32; 4])> = ctx
        .world
        .query2::<Flash, BaseColor>()
        .map(|(e, f, b)| (e, f.t, b.0))
        .collect();
    let mut expired = Vec::new();
    for (e, t, base) in rows {
        let k = (t / FLASH_TIME).clamp(0.0, 1.0);
        let col = [
            base[0] + (1.2 - base[0]) * k,
            base[1] + (1.2 - base[1]) * k,
            base[2] + (1.2 - base[2]) * k,
            base[3],
        ];
        if let Some(c) = ctx.world.get_mut::<MeshColor>(e) {
            c.0 = col;
        }
        let nt = t - ctx.dt;
        if nt <= 0.0 {
            expired.push(e);
        } else if let Some(f) = ctx.world.get_mut::<Flash>(e) {
            f.t = nt;
        }
    }
    for e in expired {
        ctx.world.remove_component::<Flash>(e);
    }
}

/// Stage::Sim — local rotation (composes with parent rotation during Late).
pub fn spin(ctx: &mut Ctx) {
    let spinners: Vec<(Entity, Vec3, f32)> = ctx
        .world
        .query2::<Transform, Spin>()
        .map(|(e, _t, s)| (e, s.axis, s.speed))
        .collect();
    for (e, axis, speed) in spinners {
        if let Some(t) = ctx.world.get_mut::<Transform>(e) {
            t.rotation =
                Quat::from_axis_angle(axis.normalize_or_zero(), speed * ctx.dt) * t.rotation;
        }
    }
}

/// Stage::Sim — particle physics (allocation-free).
pub fn update_particles(ctx: &mut Ctx) {
    let pools: Vec<Entity> = ctx.world.query1::<Particles>().map(|(e, _)| e).collect();
    for e in pools {
        if let Some(p) = ctx.world.get_mut::<Particles>(e) {
            p.update(ctx.dt);
        }
    }
}
