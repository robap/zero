//! Lint a hand-crafted project containing every failure mode from
//! `improved_agent_usage.md` and the `zero_demo` agent output. Asserts
//! that representative rules (L01, L02, L06, L08, L11, L12, L13) fire.

use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::tempdir;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("agent_failures")
}

#[test]
fn agent_failure_patterns_fire_expected_rules() {
    let tmp = tempdir().unwrap();
    std::fs::write(
        tmp.path().join("zero.toml"),
        "[project]\nroot = \".\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();
    // Minimal `.zero/styles/_tokens.scss` so L13 has a known-vars set;
    // declare just `--space-md` so `var(--radius-pill)` is flagged as
    // undefined.
    std::fs::create_dir_all(tmp.path().join(".zero/styles")).unwrap();
    std::fs::write(
        tmp.path().join(".zero/styles/_tokens.scss"),
        ":root { --space-md: 1rem; }\n",
    )
    .unwrap();
    let src = fixture_root().join("styles/app.scss");
    let dst_dir = tmp.path().join("styles");
    std::fs::create_dir_all(&dst_dir).unwrap();
    std::fs::copy(&src, dst_dir.join("app.scss")).unwrap();

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
        "expected lint to fail on agent failure patterns; stderr={stderr}"
    );
    for rule in ["L01", "L02", "L06", "L08", "L11", "L12", "L13"] {
        assert!(
            stderr.contains(rule),
            "expected rule {rule} to fire; stderr={stderr}"
        );
    }
}
