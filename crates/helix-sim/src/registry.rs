//! The game Registry (REG-1): every hashed/saved component, one definition,
//! three consumers (digest, save, console hash). Registration happens in
//! CoreSimModule::build via register_all — the module path is the only path.
use bytemuck::Pod;
use helix_ecs::{Flags, Registry, Transform, WorldRng};
use helix_scene::{BaseColor, Collider, DirectionalLight, MeshColor, Player, Spin, Velocity};

use crate::particles::{Particle, Particles};

pub fn register_all(r: &mut Registry) {
    let pod_h_s = Flags(Flags::POD.0 | Flags::HASHED.0 | Flags::SAVE.0);
    let pod_h = Flags(Flags::POD.0 | Flags::HASHED.0);
    let pod_s = Flags(Flags::POD.0 | Flags::SAVE.0);

    r.register::<Transform>(
        "transform", pod_h_s,
        |w, e, o| match w.get::<Transform>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, *bytemuck::pod_read_unaligned::<Transform>(b));
            Ok(())
        });

    r.register::<Velocity>(
        "velocity", pod_h_s,
        |w, e, o| match w.get::<Velocity>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, *bytemuck::pod_read_unaligned::<Velocity>(b));
            Ok(())
        });

    r.register::<Spin>(
        "spin", pod_h,
        |w, e, o| match w.get::<Spin>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, *bytemuck::pod_read_unaligned::<Spin>(b));
            Ok(())
        });

    r.register::<Collider>(
        "collider", pod_h_s,
        |w, e, o| match w.get::<Collider>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, *bytemuck::pod_read_unaligned::<Collider>(b));
            Ok(())
        });

    // ZST marker: presence is the entire value (ECS-13 presence-only path).
    r.register::<Player>(
        "player", Flags(Flags::HASHED.0 | Flags::SAVE.0),
        |w, e, _o| w.get::<Player>(e).is_some(),
        |w, e, _b| { w.insert(e, Player); Ok(()) });

    r.register::<helix_ecs::Name>(
        "name", pod_s,
        |w, e, o| match w.get::<helix_ecs::Name>(e) {
            Some(v) => { o.extend_from_slice(v.0.as_bytes()); true } None => false },
        |w, e, b| {
            w.insert(e, helix_ecs::Name(
                String::from_utf8(b.to_vec()).map_err(|_| "bad utf8 in name")?));
            Ok(())
        });

    r.register::<MeshColor>(
        "mesh_color", pod_h,
        |w, e, o| match w.get::<MeshColor>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, MeshColor(*bytemuck::pod_read_unaligned::<[f32; 4]>(b)));
            Ok(())
        });

    r.register::<BaseColor>(
        "base_color", pod_s,
        |w, e, o| match w.get::<BaseColor>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, BaseColor(*bytemuck::pod_read_unaligned::<[f32; 4]>(b)));
            Ok(())
        });

    r.register::<WorldRng>(
        "world_rng", pod_h_s,
        |w, e, o| match w.get::<WorldRng>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(&v.s)); true } None => false },
        |w, e, b| {
            let arr: [u64; 4] = *bytemuck::pod_read_unaligned(b);
            w.insert(e, WorldRng { s: arr });
            Ok(())
        });

    r.register::<Particles>(
        "particles", Flags(Flags::HASHED.0 | Flags::TRANSIENT.0),
        |w, e, o| match w.get::<Particles>(e) {
            Some(p) => {
                o.extend_from_slice(&(p.slots.len() as u32).to_le_bytes());
                for s in &p.slots { o.extend_from_slice(bytemuck::bytes_of(s)); }
                true
            }
            None => false },
        |w, e, b| {
            if b.len() < 4 { return Err("particles: short".into()); }
            let n = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
            let ps = std::mem::size_of::<Particle>();
            if b.len() != 4 + n * ps { return Err("particles: size mismatch".into()); }
            let mut slots = Vec::with_capacity(n);
            for i in 0..n {
                slots.push(*bytemuck::pod_read_unaligned::<Particle>(&b[4 + i * ps..4 + (i + 1) * ps]));
            }
            w.insert(e, Particles { slots });
            Ok(())
        });

    r.register::<DirectionalLight>(
        "directional_light", pod_h_s,
        |w, e, o| match w.get::<DirectionalLight>(e) {
            Some(v) => { o.extend_from_slice(bytemuck::bytes_of(v)); true } None => false },
        |w, e, b| {
            w.insert(e, *bytemuck::pod_read_unaligned::<DirectionalLight>(b));
            Ok(())
        });
}

/// Single assembly point (used by modules, tests, and the console).
pub fn assemble() -> Registry {
    let mut r = Registry::new();
    register_all(&mut r);
    r
}
