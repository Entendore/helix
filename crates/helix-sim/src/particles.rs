//! Deterministic CPU particles (FX-1..2): entropy ONLY from WorldRng; fixed
//! cap with steal-oldest; allocation-free update. The render side consumes
//! `slots` (GPU instancing is roadmap; pools are digest-covered today).
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use helix_ecs::WorldRng;

pub const MAX_PARTICLES: usize = 512;
pub const LIFE_MAX: f32 = 0.7;

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub _pad: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Particles {
    pub slots: Vec<Particle>,
}

impl Particles {
    /// `samples`: 4 pre-drawn f32 per particle, drawn by the CALLER from
    /// Ctx's WorldRng (snapshot-mutate, ECS-7).
    pub fn burst(&mut self, at: Vec3, samples: &[[f32; 4]], cap: usize) {
        for &[dx, dy, dz, spd] in samples {
            let dir = Vec3::new(dx, dy * 0.8 + 0.2, dz).normalize_or_zero();
            let p = Particle {
                pos: at,
                vel: dir * (4.0 + 3.0 * spd),
                life: LIFE_MAX * (0.55 + 0.45 * spd),
                _pad: 0.0,
            };
            if self.slots.len() < cap {
                self.slots.push(p);
            } else if let Some(oldest) = self
                .slots
                .iter_mut()
                .min_by(|a, b| a.life.partial_cmp(&b.life).unwrap())
            {
                *oldest = p; // FX-2: steal-oldest, never grow
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.slots.iter_mut() {
            p.vel.y -= 9.81 * dt * 0.6;
            p.pos += p.vel * dt;
            if p.pos.y < 0.05 {
                p.pos.y = 0.05;
                p.vel.y *= -0.4;
                p.vel.x *= 0.8;
                p.vel.z *= 0.8;
            }
            p.life -= dt;
        }
        self.slots.retain(|p| p.life > 0.0); // in-place: no allocation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [T] FX-2 — capped pool, steal-oldest, expiry empties.
    #[test]
    fn cap_and_steal() {
        let mut rng = WorldRng::new(1);
        let s: Vec<[f32; 4]> = (0..16)
            .map(|_| [rng.signed_f32(), rng.f32(), rng.signed_f32(), rng.f32()])
            .collect();
        let mut pool = Particles::default();
        pool.burst(Vec3::ZERO, &s[..8], 8);
        pool.burst(Vec3::ONE, &s[8..], 8); // overflow -> steal
        assert_eq!(pool.slots.len(), 8);
        pool.update(LIFE_MAX);
        assert!(pool.slots.is_empty());
    }
}
