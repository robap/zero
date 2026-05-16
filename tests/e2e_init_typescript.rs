//! End-to-end test: `zero init` (TS scaffold) → `zero test` → `zero build --sourcemap`.

#[test]
fn ts_scaffold_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();

    let web = tmp.path().join("web");
    assert!(web.join("src/app.ts").exists());
    assert!(web.join("src/routes/home.ts").exists());
    assert!(web.join("src/routes/home.test.ts").exists());
    assert!(web.join("tsconfig.json").exists());
    assert!(web.join(".zero/zero.d.ts").exists());
    assert!(web.join(".zero/zero-test.d.ts").exists());

    // `zero test` runs the TS test and passes 2 assertions.
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("0 failed"));

    // `zero build --sourcemap` emits the bundle and a .map file.
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .arg("--sourcemap")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets = tmp.path().join("dist/assets");
    let entries: Vec<std::path::PathBuf> = std::fs::read_dir(&assets)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    assert!(
        entries.iter().any(|p| p
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("app.") && n.ends_with(".js.map"))
            .unwrap_or(false)),
        "expected app.*.js.map in {assets:?}",
    );
}
