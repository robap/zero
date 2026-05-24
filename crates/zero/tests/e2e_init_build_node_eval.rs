//! End-to-end test: `zero init` + `zero build`, with optional Node evaluation.

use std::process::Command;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[ignore = "slow"]
fn init_then_build_produces_valid_output() {
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

    let dist = tmp.path().join("dist");

    // JS bundle exists.
    let js_files: Vec<_> = std::fs::read_dir(dist.join("assets"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("app.") && n.ends_with(".js"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(js_files.len(), 1);
    let bundle_path = js_files[0].path();

    // manifest.json references existing files.
    let manifest_text = std::fs::read_to_string(dist.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    let obj = manifest.as_object().unwrap();
    for (_key, val) in obj {
        let rel = val.as_str().unwrap();
        assert!(
            dist.join(rel).exists(),
            "manifest entry {rel} must point to existing file"
        );
    }

    // index.html contains script + link tags pointing at hashed filenames.
    let index = std::fs::read_to_string(dist.join("index.html")).unwrap();
    let js_val = manifest["app.js"].as_str().unwrap();
    assert!(
        index.contains(js_val),
        "index.html must reference hashed JS"
    );
    assert!(
        index.contains(r#"<link rel="stylesheet""#),
        "index.html must reference CSS"
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
