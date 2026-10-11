//! Gameplay re-exports + constants. Component types live in helix-scene
//! (level-authorable); sim consumes them.
pub use helix_scene::{BaseColor, Collider, DirectionalLight, MeshColor, MeshRadius, Player, Spin, Velocity};

pub const PLAYER_SPEED: f32 = 5.0;
pub const LEVEL_HALF_EXTENT: f32 = 10.5;
pub const FLASH_TIME: f32 = 0.30;
/// INTP-3a teleport snap distance.
pub const SNAP: f32 = 5.0;
