//! Full integration test for `zero build`: JS bundle, CSS copy, manifest, index.html.

#[test]
fn full_build_produces_all_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
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

    // JS bundle.
    let assets: Vec<_> = std::fs::read_dir(dist.join("assets"))
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
    assert_eq!(assets.len(), 1, "expected exactly one app.<hash>.js");

    // CSS bundle.
    let css_assets: Vec<_> = std::fs::read_dir(dist.join("assets"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("css"))
        .collect();
    assert_eq!(css_assets.len(), 1, "expected exactly one hashed CSS file");

    // manifest.json.
    let manifest_text = std::fs::read_to_string(dist.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    let app_entry = manifest["app.js"]
        .as_str()
        .expect("manifest must have app.js");
    assert!(app_entry.starts_with("assets/app.") && app_entry.ends_with(".js"));
    assert!(
        dist.join(app_entry).exists(),
        "manifest app entry must point to existing file"
    );
    let css_key = manifest
        .as_object()
        .unwrap()
        .keys()
        .find(|k| k.ends_with(".css"))
        .expect("manifest must have a CSS key");
    let css_entry = manifest[css_key.as_str()].as_str().unwrap();
    assert!(
        dist.join(css_entry).exists(),
        "manifest css entry must point to existing file"
    );

    // index.html.
    let index = std::fs::read_to_string(dist.join("index.html")).unwrap();
    let js_filename = app_entry;
    assert!(
        index.contains(&format!("src=\"/{js_filename}\"")),
        "index.html must reference the hashed JS"
    );
    assert!(
        index.contains(r#"<link rel="stylesheet""#),
        "index.html must include a CSS link"
    );
}
