//! Integration tests for `zero init --yes` confirmation flow.

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn init_yes_skips_prompt_and_writes_files() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(dir.path())
        .assert()
        .success();

    let web = dir.path().join("web");
    assert!(web.join("index.html").exists(), "index.html missing");
    assert!(
        web.join(".zero/zero.d.ts").exists(),
        ".zero/zero.d.ts missing"
    );
}

#[test]
fn init_yes_without_toml_uses_defaults_and_never_prompts() {
    // No zero.toml and no TTY (assert_cmd pipes stdio): `--yes` must take
    // the wizard defaults instead of erroring with "not a terminal".
    let dir = tempdir().unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(dir.path())
        .assert()
        .success();

    let toml = std::fs::read_to_string(dir.path().join("zero.toml")).unwrap();
    assert!(toml.contains("root = \"web\""), "default root: {toml}");
    assert!(toml.contains("port = 3000"), "default port: {toml}");
    assert!(toml.contains("out = \"dist\""), "default out: {toml}");
    assert!(
        dir.path().join("web").join("index.html").exists(),
        "scaffold missing"
    );
}

#[test]
fn init_refuses_non_empty_root_before_prompt() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
    std::fs::create_dir(dir.path().join("web")).unwrap();
    std::fs::write(dir.path().join("web").join("preexisting"), "stay away").unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("is not empty"));
}
