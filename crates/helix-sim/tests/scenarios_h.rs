//! T3 [H] — hermetic determinism suite, data-driven from tests/manifest.ron.
//! Scenario files carry behavior; the manifest carries gates + specs; the
//! fixture files carry expectations. First run RECORDS missing digests and
//! demands a re-run; afterwards any sim drift fails with the delta.

#[test]
fn manifest_scenarios_hold() {
    let manifest = helix_testkit::manifest::Manifest::load();
    let mut failed = false;
    for s in &manifest.scenarios {
        if !s.gate.open() {
            helix_testkit::skip("H", &s.name, "gate closed");
            continue;
        }
        let src = std::fs::read_to_string(
            helix_testkit::root_dir().join("scenarios").join(&s.file),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", s.file));
        let (lines, digest) = helix_sim::scen::run_with_digest(&s.name, &src)
            .unwrap_or_else(|e| panic!("{e}"));
        for l in lines {
            println!("{l}");
        }
        helix_testkit::pass("H", &s.name, s.spec);

        let guard = helix_testkit::FixtureGuard::new(
            &format!("tests/digests/{}.hex", s.name),
            s.spec,
        );
        let outcome = guard.text(&format!("{digest:016x}"));
        outcome.log(&guard.path, s.spec);
        if outcome.into_result(&guard.path).is_err() {
            failed = true;
        }
    }
    assert!(
        !failed,
        "digest drift — run UPDATE_GOLDENS=1 cargo test, review the diff, commit"
    );
}
