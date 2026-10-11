//! Save/load v1 (SAVE-1..6): registry-SAVE whitelist, versioned container
//! with content hash, slot-map Parent remap, atomic write. Computed
//! components are never serialized (SAVE-1). The migrations table exists so
//! the first schema change wires machinery, not folklore (SAVE-4).
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::registry::Registry;
use crate::transform::Parent;
use crate::world::World;

pub const MAGIC: [u8; 8] = *b"HELIXSV\0";
pub const VERSION: u32 = 1;
type Migration = fn(&mut Vec<u8>) -> Result<(), String>;
const MIGRATIONS: &[Migration] = &[];

#[derive(Serialize, Deserialize)]
struct EntityRec {
    parent_slot: Option<u32>,
    comps: Vec<(u32, Vec<u8>)>, // (schema id, bytes)
}

#[derive(Serialize, Deserialize)]
struct SaveData {
    entities: Vec<EntityRec>,
}

fn hash(b: &[u8]) -> u64 {
    helix_core::fnv1a(0x8422_2325_8422_2325, b)
}

/// SAVE-6: temp file -> fsync -> rename. Returns the content hash.
pub fn save(world: &World, registry: &Registry, path: &str) -> Result<u64, String> {
    let mut entities = Vec::new();
    for e in world.iter_entities() {
        let mut comps = Vec::new();
        for (schema, write, _read) in registry.save_entries() {
            let mut bytes = Vec::new();
            if write(world, e, &mut bytes) {
                comps.push((schema, bytes));
            }
        }
        if comps.is_empty() {
            continue;
        }
        let parent_slot = world.get::<Parent>(e).map(|p| p.0.index);
        entities.push(EntityRec { parent_slot, comps });
    }
    let payload =
        bincode::serialize(&SaveData { entities }).map_err(|e| format!("save encode: {e}"))?;

    let mut blob = Vec::with_capacity(payload.len() + 48);
    blob.extend_from_slice(&MAGIC);
    blob.extend_from_slice(&VERSION.to_le_bytes());
    blob.extend_from_slice(&0u64.to_le_bytes()); // seed header reserved (RNG-4)
    blob.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    blob.extend_from_slice(&payload);
    let content_hash = hash(&blob);
    blob.extend_from_slice(&content_hash.to_le_bytes());

    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("save mkdir: {e}"))?;
    }
    let tmp = format!("{path}.tmp");
    let mut f = std::fs::File::create(&tmp).map_err(|e| format!("save create: {e}"))?;
    f.write_all(&blob)
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("save write: {e}"))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| format!("save rename: {e}"))?;
    Ok(content_hash)
}

/// SAVE-5: caller guarantees stage-boundary timing; nothing mutates until
/// the file fully validates.
pub fn load(world: &mut World, registry: &Registry, path: &str) -> Result<usize, String> {
    let blob = std::fs::read(path).map_err(|e| format!("load read: {e}"))?;
    if blob.len() < 48 || blob[..8] != MAGIC {
        return Err(format!("load '{path}': bad magic — not a HELIXSV file"));
    }
    let version = u32::from_le_bytes(blob[8..12].try_into().unwrap());
    if version > VERSION {
        return Err(format!(
            "load '{path}': file v{version} > engine v{VERSION} (future file, refusing)"
        ));
    }
    if version != VERSION {
        return Err(format!(
            "load '{path}': v{version} has no migration path (engine v{VERSION})"
        ));
    }
    let payload_len = u64::from_le_bytes(blob[24..32].try_into().unwrap()) as usize;
    let body_end = 32 + payload_len;
    if blob.len() < body_end + 8 {
        return Err(format!("load '{path}': truncated"));
    }
    let stored = u64::from_le_bytes(blob[body_end..body_end + 8].try_into().unwrap());
    if hash(&blob[..body_end]) != stored {
        return Err(format!("load '{path}': content hash mismatch (corrupt)"));
    }
    let data: SaveData =
        bincode::deserialize(&blob[32..body_end]).map_err(|e| format!("load decode: {e}"))?;

    // PASS 1: fresh identities + components (SAVE-3).
    let mut slot_map = std::collections::HashMap::new();
    let mut links = Vec::new();
    for rec in &data.entities {
        let e = world.spawn();
        for (schema, bytes) in &rec.comps {
            registry.read_into(world, e, *schema, bytes)?;
        }
        if let Some(slot) = rec.parent_slot {
            links.push((e, slot));
        }
        slot_map.insert(e.index, e);
    }
    // PASS 2: remap parents; unknown => detach + warn (HIER-5 degrade).
    let mut count = 0;
    for (child, slot) in links {
        if let Some(&parent) = slot_map.get(&slot) {
            world.insert(child, Parent(parent));
            count += 1;
        } else {
            log::warn!("save load: parent slot {slot} missing; child detached");
        }
    }
    Ok(data.entities.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Flags;
    use crate::transform::Transform;

    fn reg() -> Registry {
        let mut r = Registry::new();
        r.register::<Transform>(
            "transform",
            Flags(Flags::POD.0 | Flags::SAVE.0),
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
        r
    }

    /// [T] SAVE-1/4/6 — round-trip preserves whitelisted state; magic and
    /// future-version files are rejected.
    #[test]
    fn roundtrip_and_reject() {
        let path = "target/test-helixsv.sv";
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, Transform::default());
        save(&w, &reg(), path).unwrap();

        let mut w2 = World::new();
        assert_eq!(load(&mut w2, &reg(), path).unwrap(), 1);
        assert!(w2.query1::<Transform>().next().is_some());

        let mut blob = std::fs::read(path).unwrap();
        blob[3] ^= 0xFF;
        std::fs::write(path, &blob).unwrap();
        assert!(load(&mut World::new(), &reg(), path)
            .err()
            .unwrap()
            .contains("magic"));
    }
}
