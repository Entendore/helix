//! One fixture lifecycle (TST-14): UPDATE_GOLDENS=1 records; otherwise
//! verifies. Verify+missing records AND demands a re-run. Drift output is
//! greppable and names the regeneration command.
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Verify,
    Record,
}

pub fn mode() -> Mode {
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        Mode::Record
    } else {
        Mode::Verify
    }
}

pub enum Outcome {
    Ok,
    Recorded,
    Drift { expected: String, actual: String },
}

impl Outcome {
    pub fn log(&self, path: &Path, spec: &str) {
        let p = path.display();
        match self {
            Outcome::Ok => println!("fixture: {p} ok [{spec}]"),
            Outcome::Recorded => {
                println!("fixture: {p} RECORDED (review + commit) [{spec}]")
            }
            Outcome::Drift { expected, actual } => println!(
                "fixture: {p} DRIFT [{spec}]\n  expected: {expected}\n  actual:   {actual}\n  \
                 fix: UPDATE_GOLDENS=1 cargo test, review the diff, commit"
            ),
        }
    }

    pub fn into_result(self, path: &Path) -> Result<(), String> {
        match self {
            Outcome::Ok | Outcome::Recorded => Ok(()),
            Outcome::Drift { .. } => Err(format!("fixture drifted: {}", path.display())),
        }
    }
}

pub struct FixtureGuard {
    pub path: PathBuf,
    pub spec: &'static str,
}

impl FixtureGuard {
    pub fn new(rel: &str, spec: &'static str) -> Self {
        Self { path: crate::root_dir().join(rel), spec }
    }

    fn read(&self) -> Option<Vec<u8>> {
        std::fs::read(&self.path).ok()
    }

    fn write(&self, bytes: &[u8]) {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(&self.path, bytes).unwrap();
    }

    /// Exact-text fixture (digest hex lines, folds).
    pub fn text(&self, current: &str) -> Outcome {
        let current = current.trim_end().to_string();
        match (mode(), self.read()) {
            (Mode::Record, _) => {
                self.write(current.as_bytes());
                Outcome::Recorded
            }
            (Mode::Verify, None) => {
                self.write(current.as_bytes());
                Outcome::Drift {
                    expected: "<missing — recorded now, RE-RUN TO VERIFY>".into(),
                    actual: current,
                }
            }
            (Mode::Verify, Some(stored)) => {
                let stored = String::from_utf8_lossy(&stored).trim_end().to_string();
                if stored == current {
                    Outcome::Ok
                } else {
                    Outcome::Drift { expected: stored, actual: current }
                }
            }
        }
    }
}
