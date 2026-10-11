//! helix-testkit — shared test substrate (TST-11..14). Dev-only: never a
//! runtime dependency of any engine crate. One fixture lifecycle, one
//! manifest, intrinsic gates.
pub mod fixtures;
pub mod gates;
pub mod manifest;

pub use fixtures::{FixtureGuard, Mode, Outcome};

/// Repo root (testkit sits at <root>/crates/helix-testkit).
pub fn root_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("testkit is two levels below the repo root")
        .to_path_buf()
}

pub fn pass(tier: &str, name: &str, spec: &str) {
    println!("[{tier}] {name} … ok [{spec}]");
}

pub fn skip(tier: &str, name: &str, why: &str) {
    println!("[{tier}] {name} … SKIP ({why})");
}
