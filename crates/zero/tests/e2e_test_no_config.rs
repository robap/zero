//! End-to-end test: `zero test` runs in a directory with no `zero.toml`,
//! falling back to built-in defaults (discovery root = cwd).

use assert_cmd::Command;
use predicates::str::contains;

const PASSING_TEST: &str = "import { it, expect } from 'zero/test';\n\
it('runs with no zero.toml', () => { expect(1 + 1).toBe(2); });\n";

#[test]
#[ignore = "slow"]
fn runs_green_without_zero_toml() {
    let tmp = tempfile::tempdir().unwrap();
    // No zero.toml, no `zero init` — just a bare test file at the root.
    std::fs::write(tmp.path().join("app.test.ts"), PASSING_TEST).unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(contains("passed"));
}

#[test]
#[ignore = "slow"]
fn runs_cwd_relative_file_arg_without_zero_toml() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub").join("app.test.ts"), PASSING_TEST).unwrap();
    // A second test file that must NOT run when the file arg is given.
    std::fs::write(tmp.path().join("other.test.ts"), PASSING_TEST).unwrap();

    let output = Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .arg("sub/app.test.ts")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "should exit 0: {stdout}");
    assert!(
        stdout.contains("sub/app.test.ts"),
        "should run the cwd-relative file: {stdout}"
    );
    assert!(
        !stdout.contains("other.test.ts"),
        "should run only the named file: {stdout}"
    );
}
