//! Integration tests for `zero update`.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn init_project(tmp: &std::path::Path) -> std::path::PathBuf {
    fs::write(
        tmp.join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();
    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(tmp)
        .assert()
        .success();
    tmp.join("web")
}

#[test]
fn update_restores_modified_recreates_deleted_removes_stray() {
    let tmp = tempdir().unwrap();
    let web = init_project(tmp.path());

    let app_scss = fs::read(web.join("styles/app.scss")).unwrap();
    let index_html = fs::read(web.join("index.html")).unwrap();
    let app_ts = fs::read(web.join("src/app.ts")).unwrap();
    let tsconfig = fs::read(web.join("tsconfig.json")).unwrap();
    let agents = fs::read(web.join("AGENTS.md")).unwrap();

    fs::write(web.join(".zero/styles/_tokens.scss"), b"/* MUTATED */\n").unwrap();
    fs::remove_file(web.join(".zero/styles/_utilities.scss")).unwrap();
    fs::write(web.join(".zero/styles/_extra.scss"), b"// stray\n").unwrap();

    // `zero update` is invoked from the workspace root (where zero.toml
    // lives), not from inside the scaffolded `web/` directory.
    Command::cargo_bin("zero")
        .unwrap()
        .arg("update")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();

    let tokens_after = fs::read(web.join(".zero/styles/_tokens.scss")).unwrap();
    let tokens_str = std::str::from_utf8(&tokens_after).unwrap();
    assert!(
        tokens_str.contains("--color-primary:"),
        "tokens not restored: {tokens_str}"
    );
    assert!(
        !tokens_str.contains("/* MUTATED */"),
        "mutated marker still present: {tokens_str}"
    );

    assert!(web.join(".zero/styles/_utilities.scss").exists());
    assert!(!web.join(".zero/styles/_extra.scss").exists());

    assert_eq!(fs::read(web.join("styles/app.scss")).unwrap(), app_scss);
    assert_eq!(fs::read(web.join("index.html")).unwrap(), index_html);
    assert_eq!(fs::read(web.join("src/app.ts")).unwrap(), app_ts);
    assert_eq!(fs::read(web.join("tsconfig.json")).unwrap(), tsconfig);
    assert_eq!(fs::read(web.join("AGENTS.md")).unwrap(), agents);
}

#[test]
fn update_on_clean_project_is_noop() {
    let tmp = tempdir().unwrap();
    let _web = init_project(tmp.path());
    let assert = Command::cargo_bin("zero")
        .unwrap()
        .arg("update")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = std::str::from_utf8(&assert.get_output().stdout)
        .unwrap()
        .to_string();
    assert!(
        out.contains("already up to date"),
        "expected up-to-date message, got: {out}"
    );
}
