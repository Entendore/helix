//! WorldRng (RNG-1..3): xoshiro256++, SplitMix-seeded. STREAM SEMANTICS ARE
//! FROZEN (RNG-2, ADR-010): replays, digests and saves depend on it. Lives
//! as a component on the engine-owned "__rng" entity, so the registry's
//! HASHED flag makes it digest-visible and SAVE makes it persist.
#[derive(Clone, Debug)]
pub struct WorldRng {
    pub s: [u64; 4], // frozen serialization surface
}

pub const DEFAULT_SEED: u64 = 0x5EED_0000_0000_0001;

impl WorldRng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let mut next = move || {
            sm = sm.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = sm;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        WorldRng { s: [next(), next(), next(), next()] }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[0].wrapping_add(self.s[3]).rotate_left(23).wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform [0, 1), 24-bit mantissa.
    pub fn f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) * (1.0 / (1u64 << 24) as f32)
    }

    /// Uniform [-1, 1).
    pub fn signed_f32(&mut self) -> f32 {
        self.f32() * 2.0 - 1.0
    }

    /// RNG-3: derived stream; does NOT advance the main stream.
    pub fn branch(&mut self, entity_index: u32) -> WorldRng {
        let mut r = self.clone();
        for _ in 0..(entity_index % 16) + 1 {
            r.jump();
        }
        r
    }

    fn jump(&mut self) {
        const JUMP: [u64; 4] = [
            0x180e_c6d3_3cfd_0aba,
            0xd5a6_1266_f0c9_392c,
            0xa958_2618_e03f_c9aa,
            0x39ab_dc45_29b1_661c,
        ];
        let mut s = [0u64; 4];
        for j in JUMP {
            for b in 0..64 {
                if j & (1u64 << b) != 0 {
                    s[0] ^= self.s[0];
                    s[1] ^= self.s[1];
                    s[2] ^= self.s[2];
                    s[3] ^= self.s[3];
                }
                self.next_u64();
            }
        }
        self.s = s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [T] RNG-2 — same seed => same stream; different seed diverges.
    #[test]
    fn seed_determinism() {
        let (mut a, mut b, mut c) = (WorldRng::new(7), WorldRng::new(7), WorldRng::new(8));
        let va: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let vb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        let vc: Vec<u64> = (0..8).map(|_| c.next_u64()).collect();
        assert_eq!(va, vb);
        assert_ne!(va, vc);
    }

    /// [T] RNG-3 — branches are disjoint and side-effect-free.
    #[test]
    fn branch_disjoint() {
        let mut r = WorldRng::new(9);
        let before = r.clone();
        let (mut b0, mut b1) = (r.branch(0), r.branch(1));
        let x0: Vec<u64> = (0..4).map(|_| b0.next_u64()).collect();
        let x1: Vec<u64> = (0..4).map(|_| b1.next_u64()).collect();
        assert_ne!(x0, x1);
        assert_eq!(r.s, before.s);
    }
}
