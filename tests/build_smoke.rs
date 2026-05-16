//! Integration smoke test for `zero build`.

use std::process::Command;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn build_produces_hashed_bundle() {
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

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets = tmp.path().join("dist/assets");
    let entries: Vec<_> = std::fs::read_dir(&assets)
        .expect("assets dir should exist")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("app.") && n.ends_with(".js"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(entries.len(), 1, "expected exactly one app.<hash>.js");

    let bundle_path = entries[0].path();
    let bundle = std::fs::read_to_string(&bundle_path).unwrap();
    assert!(!bundle.is_empty(), "bundle should not be empty");
    assert!(
        bundle.contains("function signal("),
        "bundle missing signal()"
    );
    assert!(bundle.contains("class App"), "bundle missing App class");
    assert!(bundle.contains("Home"), "bundle missing Home function");

    // No top-level `import` or `export` (all rewritten to CJS).
    let re_import = regex::Regex::new(r"(?m)^\s*import\s").unwrap();
    let re_export = regex::Regex::new(r"(?m)^\s*export\s").unwrap();
    assert!(
        !re_import.is_match(&bundle),
        "bundle must not have top-level import"
    );
    assert!(
        !re_export.is_match(&bundle),
        "bundle must not have top-level export"
    );

    if node_available() {
        let output = Command::new("node")
            .arg("--check")
            .arg(&bundle_path)
            .output()
            .expect("node --check failed to run");
        assert!(
            output.status.success(),
            "node --check failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
