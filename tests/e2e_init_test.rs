//! End-to-end test: `zero init` + `zero test` full test-runner flow.

use assert_cmd::Command;
use predicates::str::contains;

fn scaffold_temp_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp
}

#[test]
fn scaffolded_home_test_passes() {
    let tmp = scaffold_temp_project();
    Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(contains("2 passed, 0 failed"));
}

#[test]
fn failing_test_produces_nonzero_exit_and_fail_output() {
    let tmp = scaffold_temp_project();

    // Break one assertion in home.test.js.
    let test_path = tmp.path().join("web/src/routes/home.test.js");
    let content = std::fs::read_to_string(&test_path).unwrap();
    let broken = content.replace(r#"toBe("Count: 0")"#, r#"toBe("Count: 99")"#);
    std::fs::write(&test_path, broken).unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stdout(contains("FAIL"));
}

#[test]
fn broken_file_reports_error_and_other_tests_still_run() {
    let tmp = scaffold_temp_project();

    // Add a file with a top-level throw.
    let broken_path = tmp.path().join("web/src/broken.test.js");
    std::fs::write(&broken_path, r#"throw new Error("nope");"#).unwrap();

    let output = Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit non-zero");
    assert!(
        stdout.contains("ERROR loading") || stdout.contains("nope"),
        "should report the broken file: {stdout}"
    );
    // The good tests must still have run.
    assert!(
        stdout.contains("passed"),
        "good tests should still run: {stdout}"
    );
}

#[test]
fn target_filter_runs_only_matched_file() {
    let tmp = scaffold_temp_project();

    // Add a second test file.
    let other_path = tmp.path().join("web/src/other.test.js");
    std::fs::write(
        &other_path,
        r#"import { describe, it, expect } from "zero/test";
describe("Other", () => { it("works", () => expect(1).toBe(1)); });
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .arg("home.test.js")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("Other"),
        "filter should exclude Other: {stdout}"
    );
    assert!(
        stdout.contains("Home"),
        "filter should include Home: {stdout}"
    );
}
