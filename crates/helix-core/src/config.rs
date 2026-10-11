//! Startup config (CFG-1..3): RON with per-field defaults; absent file is
//! valid; parse errors warn and fall back to defaults (user files degrade,
//! they never abort boot).
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Config {
    pub vsync: bool,
    pub interpolation: bool,
    pub max_sim_steps: u32,
    pub window: (u32, u32),
    /// RNG-4: the WorldRng seed is configuration.
    pub seed: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vsync: true,
            interpolation: true,
            max_sim_steps: 5,
            window: (1280, 720),
            seed: 0x5EED_0000_0000_0001,
        }
    }
}

impl Config {
    pub fn load() -> Config {
        match std::fs::read_to_string("helix.ron") {
            Ok(s) => ron::from_str(&s).unwrap_or_else(|e| {
                log::warn!("helix.ron parse error ({e}); using defaults");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }
}
