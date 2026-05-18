//! Smoke tests for `zero lint`: a hand-rolled minimal project exercises
//! the CLI end-to-end (exit code + stderr contents).

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn write_zero_toml(tmp: &std::path::Path) {
    std::fs::write(
        tmp.join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.join("web/styles")).unwrap();
}

#[test]
fn lint_flags_raw_font_weight() {
    let tmp = tempdir().unwrap();
    write_zero_toml(tmp.path());
    std::fs::write(
        tmp.path().join("web/styles/app.scss"),
        ".x { font-weight: 600; }\n",
    )
    .unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("L01"))
        .stderr(predicate::str::contains("--weight-semi"));
}

#[test]
fn lint_quiet_suppresses_snippet() {
    let tmp = tempdir().unwrap();
    write_zero_toml(tmp.path());
    std::fs::write(
        tmp.path().join("web/styles/app.scss"),
        ".x { font-weight: 600; }\n",
    )
    .unwrap();

    let out = Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .arg("--quiet")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("L01"), "stderr = {stderr}");
    assert!(
        !stderr.contains("^"),
        "quiet mode should drop the caret line: {stderr}"
    );
}

#[test]
fn lint_clean_project_exits_zero() {
    let tmp = tempdir().unwrap();
    write_zero_toml(tmp.path());
    std::fs::write(
        tmp.path().join("web/styles/app.scss"),
        ".x { font-weight: var(--weight-semi); }\n",
    )
    .unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .success();
}
