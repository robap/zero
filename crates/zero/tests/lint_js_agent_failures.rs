//! Lint a hand-crafted project containing every JS/TS framework idiom
//! failure mode. Asserts that each rule ID (R01–R03, T01–T04, C01,
//! C02, I01, I02, S01) fires — R03 via the module-level `effect()` in
//! `src/lib/leaky-effect.ts` — AND that module-level `signal()` /
//! `computed()` stay lint-clean regardless of directory
//! (`src/stores/ok.ts`, `src/features/parts/store.ts`).

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
        "R01", "R02", "R03", "T01", "T02", "T03", "T04", "C01", "C02", "I01", "I02", "S01",
    ] {
        assert!(
            stderr.contains(rule),
            "expected rule {rule} to fire; stderr=\n{stderr}"
        );
    }
    for clean in ["stores/ok.ts", "features/parts/store.ts"] {
        assert!(
            !stderr.contains(clean),
            "module-level signal()/computed() must lint clean in any directory; \
             {clean} was flagged. stderr=\n{stderr}"
        );
    }
}
