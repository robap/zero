//! Lint a hand-crafted project containing every JS/TS framework idiom
//! failure mode. Asserts that each rule ID (R01, R02, T01–T04, C01,
//! C02, I01, I02, S01) fires AND that R03 stays silent (the fixture's
//! one would-be R03 violation lives under `src/stores/`, which is
//! exempt).

mod common;

use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::tempdir;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("js_agent_failures")
}

#[test]
fn js_failure_patterns_fire_expected_rules() {
    let tmp = tempdir().unwrap();
    common::copy_dir_filtered(&fixture_root(), tmp.path(), &["dist", "node_modules"]);

    let out = Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .arg("--quiet")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected lint to fail; stderr={stderr}"
    );
    for rule in [
        "R01", "R02", "T01", "T02", "T03", "T04", "C01", "C02", "I01", "I02", "S01",
    ] {
        assert!(
            stderr.contains(rule),
            "expected rule {rule} to fire; stderr=\n{stderr}"
        );
    }
    assert!(
        !stderr.contains("R03"),
        "R03 should not fire — the only top-level signal() is under src/stores/ (exempt). stderr=\n{stderr}"
    );
}
