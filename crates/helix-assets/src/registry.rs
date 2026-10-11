//! Name->mesh map with bound-sphere radius (REN-13 input).
use std::collections::HashMap;

use crate::types::MeshId;

pub struct MeshEntry {
    pub id: MeshId,
    pub radius: f32,
}

#[derive(Default)]
pub struct MeshAssets {
    by_name: HashMap<String, MeshEntry>,
}

impl MeshAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: &str, id: MeshId, radius: f32) {
        self.by_name.insert(name.to_string(), MeshEntry { id, radius });
    }

    /// Unknown mesh at load = hard error (SCN-4).
    pub fn get(&self, name: &str) -> Result<(MeshId, f32), String> {
        self.by_name
            .get(name)
            .map(|e| (e.id, e.radius))
            .ok_or_else(|| format!("unknown mesh '{name}' (not registered)"))
    }
}
