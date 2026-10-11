//! Runtime settings registry (CVAR-1..3): typed named cvars, idempotent
//! registration, read-only probes (CAP-1), change records drained at the
//! stage boundary. Console (CVAR-2) and settings UI consume this registry.
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    F32(f32),
    U32(u32),
    Bool(bool),
    Str(String),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::F32(_) => "f32",
            Value::U32(_) => "u32",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SettingChanged {
    pub name: String,
    pub value: Value,
}

struct Entry {
    value: Value,
    #[allow(dead_code)]
    doc: &'static str,
    read_only: bool,
}

#[derive(Default)]
pub struct Cvars {
    entries: HashMap<String, Entry>,
    changes: Vec<SettingChanged>,
}

impl Cvars {
    /// Idempotent per type; conflicting re-registration is a boot error.
    pub fn register(&mut self, name: &str, default: Value, doc: &'static str) {
        self.register_inner(name, default, doc, false);
    }

    pub fn register_ro(&mut self, name: &str, value: Value, doc: &'static str) {
        self.register_inner(name, value, doc, true);
    }

    fn register_inner(&mut self, name: &str, value: Value, doc: &'static str, ro: bool) {
        match self.entries.get(name) {
            Some(e) => assert!(
                e.value.type_name() == value.type_name() && !e.read_only,
                "cvar '{name}' re-registered with a different signature"
            ),
            None => {
                self.entries.insert(name.into(), Entry { value, doc, read_only: ro });
            }
        }
    }

    /// Type-checked set; read-only probes reject writes; unchanged values
    /// emit no change record.
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        let e = self
            .entries
            .get_mut(name)
            .ok_or_else(|| format!("unknown cvar '{name}'"))?;
        if e.read_only {
            return Err(format!("cvar '{name}' is read-only"));
        }
        if e.value.type_name() != value.type_name() {
            return Err(format!(
                "cvar '{name}' is {}, got {}",
                e.value.type_name(),
                value.type_name()
            ));
        }
        if e.value != value {
            e.value = value.clone();
            self.changes.push(SettingChanged { name: name.into(), value });
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.entries.get(name).map(|e| &e.value)
    }

    pub fn f32(&self, name: &str) -> f32 {
        match self.get(name) {
            Some(Value::F32(v)) => *v,
            _ => 0.0,
        }
    }

    pub fn list(&self) -> Vec<String> {
        let mut v: Vec<_> = self.entries.keys().cloned().collect();
        v.sort();
        v
    }

    /// EVT-9: consumers react to changes at the boundary; they never poll.
    pub fn drain_changes(&mut self) -> std::vec::IntoIter<SettingChanged> {
        std::mem::take(&mut self.changes).into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [T] CVAR-1/3 — types, read-only, change records.
    #[test]
    fn types_readonly_and_changes() {
        let mut c = Cvars::default();
        c.register("render.scale", Value::F32(1.0), "scale");
        c.register_ro("gpu.tier", Value::Str("Two".into()), "probed");
        assert!(c.set("gpu.tier", Value::Str("Zero".into())).is_err());
        assert!(c.set("render.scale", Value::Bool(true)).is_err());
        assert!(c.set("nope", Value::F32(1.0)).is_err());
        c.set("render.scale", Value::F32(0.5)).unwrap();
        c.set("render.scale", Value::F32(0.5)).unwrap(); // no-op
        assert_eq!(c.drain_changes().count(), 1);
        assert_eq!(c.f32("render.scale"), 0.5);
    }
}
