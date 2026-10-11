//! Input (INP-1/2) + mini action bindings (ACT-1..3): axis pairs from RON,
//! unknown key names warn + skip; Sim only ever sees ActionState (ACT-4),
//! which is what lets headless timelines drive the sim (REP-1).
use std::collections::HashSet;

use serde::Deserialize;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

pub const DEFAULT_BINDINGS: &str = include_str!("../../assets/bindings.ron");

#[derive(Default)]
pub struct Input {
    pressed: HashSet<PhysicalKey>,
}

impl Input {
    pub fn set_key(&mut self, key: PhysicalKey, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.pressed.insert(key);
            }
            ElementState::Released => {
                self.pressed.remove(&key);
            }
        }
    }
    pub fn down(&self, code: KeyCode) -> bool {
        self.pressed.contains(&PhysicalKey::Code(code))
    }
    pub fn set_of(&self, codes: &[KeyCode]) -> usize {
        codes.iter().filter(|c| self.down(**c)).count()
    }
    /// WIN-2: focus loss clears everything.
    pub fn clear(&mut self) {
        self.pressed.clear();
    }
}

#[derive(Debug, Deserialize)]
pub struct BindingsDoc {
    pub bindings: Vec<AxisDoc>,
}

#[derive(Debug, Deserialize)]
pub struct AxisDoc {
    pub name: String,
    #[serde(default)]
    pub neg: Vec<String>,
    #[serde(default)]
    pub pos: Vec<String>,
}

pub struct AxisDef {
    pub name: String,
    pub neg: Vec<KeyCode>,
    pub pos: Vec<KeyCode>,
}

#[derive(Default)]
pub struct Bindings {
    pub axes: Vec<AxisDef>,
}

/// ACT-3: unknown key names warn + skip — never panic.
pub fn parse_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "KeyA" => KeyCode::KeyA,
        "KeyD" => KeyCode::KeyD,
        "KeyW" => KeyCode::KeyW,
        "KeyS" => KeyCode::KeyS,
        "KeyQ" => KeyCode::KeyQ,
        "KeyE" => KeyCode::KeyE,
        "Space" => KeyCode::Space,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        other => {
            log::warn!("unknown key '{other}' in bindings; skipped");
            return None;
        }
    })
}

impl Bindings {
    pub fn from_source(src: &str) -> Bindings {
        let doc: BindingsDoc = match ron::from_str(src) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("bindings parse error ({e}); empty bindings");
                return Bindings::default();
            }
        };
        Bindings {
            axes: doc
                .bindings
                .into_iter()
                .map(|a| AxisDef {
                    name: a.name,
                    neg: a.neg.iter().filter_map(|k| parse_key(k)).collect(),
                    pos: a.pos.iter().filter_map(|k| parse_key(k)).collect(),
                })
                .collect(),
        }
    }
}

/// ACT-1: axis value = pos_pressed - neg_pressed, clamped to [-1, 1].
pub fn build_actions(input: &Input, bindings: &Bindings) -> helix_ecs::ActionState {
    let mut st = helix_ecs::ActionState::default();
    for a in &bindings.axes {
        let v = input.set_of(&a.pos) as f32 - input.set_of(&a.neg) as f32;
        st.set(&a.name, v);
    }
    st
}

#[cfg(test)]
mod tests {
    use helix_ecs::ActionState;

    /// [T] ACT-1/3 — bindings parse; missing axis reads 0; clamping holds.
    #[test]
    fn bindings_parse_and_clamp() {
        let b = Bindings::from_source(DEFAULT_BINDINGS);
        assert!(b.axes.iter().any(|a| a.name == "move_x"));
        let mut st = ActionState::default();
        st.set("move_x", 7.0);
        assert_eq!(st.axis("move_x"), 1.0);
        assert_eq!(st.axis("nope"), 0.0);
    }
}
