//! tests/manifest.ron is load-bearing (TST-12): the [H] runner iterates
//! scenarios; expectations (digests) live in fixture files, not test source.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub scenarios: Vec<ScenarioEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ScenarioEntry {
    pub name: String,
    pub file: String,
    #[serde(default)]
    pub gate: GateDoc,
    pub spec: String,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum GateDoc {
    #[default]
    All,
    LinuxOnly,
    Gpu,
}

impl GateDoc {
    pub fn open(self) -> bool {
        match self {
            GateDoc::All => true,
            GateDoc::LinuxOnly => cfg!(target_os = "linux"),
            GateDoc::Gpu => crate::gates::gpu_available(),
        }
    }
}

impl Manifest {
    pub fn load() -> Manifest {
        let path = crate::root_dir().join("tests/manifest.ron");
        ron::from_str(&std::fs::read_to_string(&path).expect("tests/manifest.ron"))
            .expect("tests/manifest.ron parses")
    }
}
