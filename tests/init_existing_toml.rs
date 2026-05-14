use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn init_with_existing_toml_scaffolds_into_root() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();

    let root = dir.path().join("web");
    assert!(root.join("index.html").exists(), "index.html missing");
    assert!(
        root.join("src").join("app.js").exists(),
        "src/app.js missing"
    );
    assert!(
        root.join("src").join("routes").join("home.js").exists(),
        "src/routes/home.js missing"
    );
    assert!(
        root.join("styles").join("app.css").exists(),
        "styles/app.css missing"
    );
}

#[test]
fn init_refuses_non_empty_root() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
    std::fs::create_dir(dir.path().join("web")).unwrap();
    std::fs::write(dir.path().join("web").join("preexisting"), "stay away").unwrap();

    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("not empty"));
}
