//! Component registry (REG-1..3): name -> position + flags + byte accessors.
//! One source of truth for state_hash (ECS-12) and saves (SAVE-1). Entries
//! are kept name-sorted so iteration is deterministic; schema ids are
//! registration positions — never TypeId-derived (ADR-021).
use std::sync::Arc;

use crate::world::{Entity, World};

#[derive(Clone, Copy)]
pub struct Flags(pub u8);

impl Flags {
    pub const POD: Flags = Flags(1 << 0);
    pub const HASHED: Flags = Flags(1 << 1);
    pub const SAVE: Flags = Flags(1 << 2);
    pub const TRANSIENT: Flags = Flags(1 << 3);
    pub fn has(self, o: Flags) -> bool {
        self.0 & o.0 != 0
    }
}

pub type WriteFn = Arc<dyn Fn(&World, Entity, &mut Vec<u8>) -> bool + Send + Sync>;
pub type ReadFn = Arc<dyn Fn(&mut World, Entity, &[u8]) -> Result<(), String> + Send + Sync>;

struct Entry {
    name: &'static str,
    flags: Flags,
    write: WriteFn,
    read: ReadFn,
}

#[derive(Default)]
pub struct Registry {
    entries: Vec<Entry>, // sorted by name
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: &'static str,
        flags: Flags,
        write: impl Fn(&World, Entity, &mut Vec<u8>) -> bool + Send + Sync + 'static,
        read: impl Fn(&mut World, Entity, &[u8]) -> Result<(), String> + Send + Sync + 'static,
    ) {
        match self.entries.binary_search_by(|e| e.name.cmp(name)) {
            Ok(_) => panic!("registry: component '{name}' registered twice (REG-2)"),
            Err(at) => self.entries.insert(
                at,
                Entry { name, flags, write: Arc::new(write), read: Arc::new(read) },
            ),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// ECS-12/REG-3: XOR of per-entity hashes over the HASHED set.
    /// XOR => iteration-order independent; presence and generation are
    /// folded in; computed components are simply not registered as HASHED.
    pub fn state_hash(&self, world: &World) -> u64 {
        let mut acc = 0u64;
        let mut count = 0u64;
        let mut scratch: Vec<u8> = Vec::with_capacity(128);
        for entity in world.iter_entities() {
            let mut h = helix_core::fnv1a(0xcbf2_9ce4_8422_2325, &entity.index.to_le_bytes());
            h = helix_core::fnv1a(h, &entity.generation.to_le_bytes());
            for entry in &self.entries {
                if !entry.flags.has(Flags::HASHED) {
                    continue;
                }
                scratch.clear();
                let present = (entry.write)(world, entity, &mut scratch);
                h = helix_core::fnv1a(h, entry.name.as_bytes());
                h = helix_core::mix_u64(h, present as u64);
                if present {
                    h = helix_core::fnv1a(h, &scratch);
                }
            }
            acc ^= h;
            count += 1;
        }
        helix_core::mix_u64(acc ^ 0xdead_beef, count)
    }

    /// SAVE-flagged entries in registration order (deterministic payload).
    pub fn save_entries(&self) -> Vec<(u32, WriteFn, ReadFn)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.flags.has(Flags::SAVE))
            .map(|(i, e)| (i as u32, e.write.clone(), e.read.clone()))
            .collect()
    }

    pub fn read_into(
        &self,
        world: &mut World,
        e: Entity,
        schema: u32,
        bytes: &[u8],
    ) -> Result<(), String> {
        let entry = self.entries.get(schema as usize).ok_or("bad schema id")?;
        (entry.read)(world, e, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Transform;

    /// [T] REG/ECS-12 — unregistered types are digest-neutral; hidden
    /// registered state diverges; XOR survives slot churn.
    #[test]
    fn digest_basics() {
        let mut r = Registry::new();
        r.register::<Transform>(
            "transform",
            Flags(Flags::POD.0 | Flags::HASHED.0),
            |w, e, out| match w.get::<Transform>(e) {
                Some(t) => {
                    out.extend_from_slice(bytemuck::bytes_of(t));
                    true
                }
                None => false,
            },
            |w, e, b| {
                w.insert(e, *bytemuck::pod_read_unaligned::<Transform>(b));
                Ok(())
            },
        );
        r.register::<i64>("secret", Flags(Flags::HASHED.0), |w, e, out| match w.get::<i64>(e) {
            Some(v) => {
                out.extend_from_slice(&v.to_le_bytes());
                true
            }
            None => false,
        },
        |w, e, b| {
            let arr: [u8; 8] = b.try_into().map_err(|_| "short")?;
            w.insert(e, i64::from_le_bytes(arr));
            Ok(())
        });

        let (mut a, mut b) = (World::new(), World::new());
        let (ea, eb) = (a.spawn(), b.spawn());
        a.insert(ea, Transform::default());
        b.insert(eb, Transform::default());
        b.insert(eb, 3.5f32); // unregistered type: invisible
        assert_eq!(r.state_hash(&a), r.state_hash(&b));
        b.insert(eb, 42i64); // registered: visible
        assert_ne!(r.state_hash(&a), r.state_hash(&b));
    }
}
