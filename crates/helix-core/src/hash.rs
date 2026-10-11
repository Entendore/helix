//! Hash primitives (ECS-12 / SAVE-2): FNV-1a fold + u64 mix. The digest
//! combines per-entity hashes with XOR, which is iteration-order independent.
pub fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(seed, |h, b| (h ^ (*b as u64)).wrapping_mul(0x1_0000_0001_b3))
}

pub fn mix_u64(seed: u64, v: u64) -> u64 {
    fnv1a(seed, &v.to_le_bytes())
}
