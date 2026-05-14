//! Integration tests for SCSS build compilation.

fn write_scss_project(tmp: &std::path::Path) {
    std::fs::write(
        tmp.join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(tmp)
        .assert()
        .success();
}

fn find_asset(dir: &std::path::Path, prefix: &str, ext: &str) -> Option<String> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.starts_with(prefix) && name.ends_with(ext) {
                Some(name)
            } else {
                None
            }
        })
}

#[test]
fn build_compiles_scss_to_hashed_css() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets_dir = tmp.path().join("dist/assets");
    let css_file = find_asset(&assets_dir, "app.", ".css").expect("no hashed CSS found");
    assert!(
        !css_file.contains("scss"),
        "output filename should not contain scss: {css_file}"
    );

    let css_content = std::fs::read_to_string(assets_dir.join(&css_file)).unwrap();
    assert!(
        !css_content.contains("$space-md"),
        "SCSS variable leaked: {css_content}"
    );
}

#[test]
fn build_index_html_rewrites_scss_link() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let index = std::fs::read_to_string(tmp.path().join("dist/index.html")).unwrap();
    assert!(
        !index.contains("app.scss"),
        "source .scss href still present in output index.html: {index}"
    );
    assert!(
        index.contains(r#"<link rel="stylesheet" href="/assets/app."#),
        "hashed CSS link missing: {index}"
    );
}

#[test]
fn build_manifest_uses_scss_source_key() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let manifest_text = std::fs::read_to_string(tmp.path().join("dist/manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    let scss_key = manifest
        .as_object()
        .unwrap()
        .keys()
        .find(|k| k.ends_with(".scss"))
        .expect("manifest must have a .scss source key");
    let css_val = manifest[scss_key.as_str()].as_str().unwrap();
    assert!(
        css_val.ends_with(".css"),
        "manifest value should be hashed css: {css_val}"
    );
}

#[test]
fn build_emits_sourcemap_when_flag_set() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .args(["build", "--sourcemap"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets_dir = tmp.path().join("dist/assets");
    let css_file = find_asset(&assets_dir, "app.", ".css").expect("no css file");
    let map_file = format!("{css_file}.map");
    assert!(
        assets_dir.join(&map_file).exists(),
        ".map file not found: {map_file}"
    );
    let map_content = std::fs::read_to_string(assets_dir.join(&map_file)).unwrap();
    assert!(
        map_content.contains("\"version\":3"),
        "invalid sourcemap: {map_content}"
    );

    let css_content = std::fs::read_to_string(assets_dir.join(&css_file)).unwrap();
    assert!(
        css_content.contains("sourceMappingURL="),
        "sourceMappingURL comment missing: {css_content}"
    );
}

#[test]
fn build_stem_collision_fails_with_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    // Add a conflicting app.css alongside the scaffold's app.scss.
    std::fs::write(tmp.path().join("web/styles/app.css"), "body {}").unwrap();

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("app"));
}

#[test]
fn build_plain_css_project_still_works() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();

    // Bootstrap using zero init, then remove the scss files and add plain css.
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    let web = tmp.path().join("web");
    std::fs::remove_file(web.join("styles/app.scss")).ok();
    std::fs::remove_file(web.join("styles/_vars.scss")).ok();
    std::fs::write(web.join("styles/legacy.css"), "body { color: blue; }").unwrap();
    // Update index.html to reference the css file.
    let idx = std::fs::read_to_string(web.join("index.html")).unwrap();
    let idx = idx.replace(r#"href="/styles/app.scss""#, r#"href="/styles/legacy.css""#);
    std::fs::write(web.join("index.html"), idx).unwrap();

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets_dir = tmp.path().join("dist/assets");
    let css_file = find_asset(&assets_dir, "legacy.", ".css").expect("legacy css not found");
    assert!(assets_dir.join(&css_file).exists());
}
