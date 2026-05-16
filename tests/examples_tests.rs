//! Integration test: every shipped `examples/<name>/` project's own
//! `zero test` suite passes. Each example uses the test runner the same
//! way users do; this guards against regressions in the runner or the
//! example tests themselves.

use predicates::Predicate;
use predicates::str::contains;

mod common;

#[test]
fn counter_tests_pass() {
    run_example_tests("counter");
}

#[test]
fn todos_tests_pass() {
    run_example_tests("todos");
}

#[test]
fn tracker_tests_pass() {
    run_example_tests("tracker");
}

fn run_example_tests(name: &str) {
    let tmp = common::prepare_example(name);

    let output = assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "example `{name}`: zero test must exit cleanly:\n{stdout}"
    );
    assert!(
        contains("0 failed").eval(&stdout),
        "example `{name}`: expected `0 failed` in test report:\n{stdout}"
    );
}
