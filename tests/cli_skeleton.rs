use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_init_subcommand() {
    Command::cargo_bin("zero")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("init"));
}

#[test]
fn help_lists_dev_subcommand() {
    Command::cargo_bin("zero")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("dev"));
}

#[test]
fn help_lists_build_subcommand() {
    Command::cargo_bin("zero")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("build"));
}

#[test]
fn version_prints_zero_and_semver() {
    Command::cargo_bin("zero")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^zero \d+\.\d+\.\d+\s*$").unwrap());
}

#[test]
fn no_args_exits_non_zero() {
    Command::cargo_bin("zero").unwrap().assert().failure();
}

#[test]
fn unknown_subcommand_exits_non_zero() {
    Command::cargo_bin("zero")
        .unwrap()
        .arg("frobnicate")
        .assert()
        .failure();
}
