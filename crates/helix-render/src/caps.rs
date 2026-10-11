//! CAP-1 (mini): boot probe -> quality tier. helix-app exposes results as
//! read-only cvars. The heuristic is deliberate + documented; a full
//! capability matrix (limits, features, timestamps) is P2.
use wgpu::Adapter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Zero,
    One,
    Two,
}

impl Tier {
    /// CAP-2: tier default render scale (consumer: RES-1 in CoreRenderer).
    pub fn render_scale(self) -> f32 {
        match self {
            Tier::Zero => 0.75,
            Tier::One | Tier::Two => 1.0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Tier::Zero => "Zero",
            Tier::One => "One",
            Tier::Two => "Two",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Capabilities {
    pub tier: Tier,
    pub backend: String,
    pub adapter: String,
    pub software: bool,
}

pub fn probe(adapter: &Adapter) -> Capabilities {
    let info = adapter.get_info();
    use wgpu::DeviceType as T;
    let tier = match info.device_type {
        T::Cpu => Tier::Zero,
        T::Integrated | T::Virtual | T::Other => Tier::One,
        T::Discrete => Tier::Two,
    };
    Capabilities {
        tier,
        backend: format!("{:?}", info.backend),
        adapter: info.name,
        software: info.device_type == T::Cpu,
    }
}
