//! Dev console (CVAR-2 mini): logic here; display uses the window title
//! until a UiPass exists. Commands execute at the stage boundary — never
//! mid-stage. Authoring errors degrade to messages, never panics (ERR-2).
#[derive(Default)]
pub struct Console {
    pub open: bool,
    pub line: String,
    pub queue: Vec<String>,
}

impl Console {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.line.clear();
    }

    pub fn key(&mut self, key: winit::keyboard::PhysicalKey, text: Option<&str>) {
        use winit::keyboard::{KeyCode, PhysicalKey};
        if !self.open {
            return;
        }
        match key {
            PhysicalKey::Code(KeyCode::Enter) => {
                if !self.line.trim().is_empty() {
                    self.queue.push(self.line.trim().to_string());
                }
                self.line.clear();
            }
            PhysicalKey::Code(KeyCode::Backspace) => {
                self.line.pop();
            }
            _ => {
                if let Some(t) = text {
                    self.line.push_str(t);
                }
            }
        }
    }

    pub fn prompt(&self) -> String {
        format!(" [console] > {}", self.line)
    }
}

/// `load` replaces world content: clears all entities, reloads the save,
/// respawns the engine RNG entity with the given seed.
pub fn execute(
    cmd: &str,
    cvars: &mut helix_core::cvars::Cvars,
    world: &mut helix_ecs::World,
    registry: &helix_ecs::Registry,
    seed: u64,
) -> String {
    use helix_core::cvars::Value;
    let mut parts = cmd.split_whitespace();
    match parts.next() {
        Some("set") => {
            let (Some(name), Some(raw)) = (parts.next(), parts.next()) else {
                return "usage: set <name> <value>".into();
            };
            let parsed = match cvars.get(name) {
                Some(Value::F32(_)) => raw.parse::<f32>().map(Value::F32),
                Some(Value::U32(_)) => raw.parse::<u32>().map(Value::U32),
                Some(Value::Bool(_)) => raw.parse::<bool>().map(Value::Bool),
                Some(Value::Str(_)) => Ok(Value::Str(raw.into())),
                None => return format!("unknown cvar '{name}'"),
            };
            match parsed {
                Ok(v) => cvars
                    .set(name, v)
                    .map(|_| format!("{name} = {raw}"))
                    .unwrap_or_else(|e| e),
                Err(_) => format!("type mismatch for '{name}'"),
            }
        }
        Some("get") => match parts.next().and_then(|n| cvars.get(n)) {
            Some(v) => format!("{v:?}"),
            None => "unknown cvar".into(),
        },
        Some("list") => cvars.list().join(", "),
        Some("hash") => format!("state_hash = {:016x}", registry.state_hash(world)),
        Some("save") => helix_ecs::save::save(world, registry, "saves/quicksave.sv")
            .map(|h| format!("saved (hash {h:016x})"))
            .unwrap_or_else(|e| e),
        Some("load") => {
            world.clear();
            let r = helix_ecs::save::load(world, registry, "saves/quicksave.sv");
            let e = world.spawn();
            world.insert(e, helix_ecs::WorldRng::new(seed));
            world.insert(e, helix_ecs::Name("__rng".into()));
            r.map(|n| format!("loaded {n} entities (world replaced)"))
                .unwrap_or_else(|e| e)
        }
        _ => format!("unknown command '{cmd}' (set|get|list|hash|save|load)"),
    }
}
