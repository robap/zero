//! Smoke tests for the JS/TS pass of `zero lint`.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn write_zero_toml(tmp: &std::path::Path) {
    std::fs::write(
        tmp.join("zero.toml"),
        "[project]\nroot = \".\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.join("src/components")).unwrap();
}

#[test]
fn lint_flags_r01_template_val_read() {
    let tmp = tempdir().unwrap();
    write_zero_toml(tmp.path());
    std::fs::write(
        tmp.path().join("src/components/Bad.ts"),
        "import { html, signal } from \"zero\";\nexport function B(){ const c = signal(0); return html`${c.val}`; }\n",
    )
    .unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("R01"));
}

#[test]
fn lint_quiet_suppresses_snippet_in_js_diags() {
    let tmp = tempdir().unwrap();
    write_zero_toml(tmp.path());
    std::fs::write(
        tmp.path().join("src/components/Bad.ts"),
        "import { html, signal } from \"zero\";\nexport function B(){ const c = signal(0); return html`${c.val}`; }\n",
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
    assert!(stderr.contains("R01"), "stderr = {stderr}");
    assert!(
        !stderr.contains("^"),
        "quiet mode should drop the caret line: {stderr}"
    );
}

#[test]
fn lint_clean_js_project_exits_zero() {
    let tmp = tempdir().unwrap();
    write_zero_toml(tmp.path());
    std::fs::write(
        tmp.path().join("src/components/Ok.ts"),
        "import { html, signal } from \"zero\";\nexport function B(){ const c = signal(0); return html`${c}`; }\n",
    )
    .unwrap();

    let out = Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "expected clean: stdout={stdout}");
    assert!(
        stdout.contains("zero lint — clean"),
        "expected 'clean' message; stdout={stdout}"
    );
}
