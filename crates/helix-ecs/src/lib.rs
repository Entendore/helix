//! helix-ecs — the deterministic simulation kernel (spec: Core profile).
//! Depends only on helix-core: no renderer, no winit (ARCH-1/ARCH-6).
pub mod engine;
pub mod events;
pub mod hierarchy;
pub mod registry;
pub mod rng;
pub mod save;
pub mod scheduler;
pub mod transform;
pub mod world;

pub use engine::{Assembled, EngineBuilder, EngineModule};
pub use events::{Channel, CollisionEvent, Events};
pub use registry::{Flags, Registry};
pub use rng::{WorldRng, DEFAULT_SEED};
pub use scheduler::{ActionState, Ctx, Hooks, Scheduler, Side, Stage, SystemFn};
pub use transform::{GlobalTransform, Name, Parent, PrevGlobal, Transform};
pub use world::{Component, Entity, World};
