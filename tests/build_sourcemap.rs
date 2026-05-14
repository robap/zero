//! Integration test: `zero build --sourcemap` emits an external `.map` file
//! alongside the bundle and appends `//# sourceMappingURL=` to the bundle.

#[test]
fn build_with_sourcemap_emits_map_file_and_url_comment() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();

    // Hand-author a minimal TS project (avoiding `zero init` so this test is
    // independent of scaffold contents).
    let web = tmp.path().join("web");
    std::fs::create_dir_all(web.join("src/routes")).unwrap();
    std::fs::write(
        web.join("index.html"),
        "<!doctype html><html><head><title>x</title></head><body><div id=app></div></body></html>",
    )
    .unwrap();
    std::fs::write(
        web.join("src/app.ts"),
        "import { signal } from \"zero\";\nconst n: number = 1;\nsignal(n);\n",
    )
    .unwrap();

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .arg("--sourcemap")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets = tmp.path().join("dist/assets");
    let entries: Vec<std::path::PathBuf> = std::fs::read_dir(&assets)
        .expect("assets dir should exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    let bundle_path = entries
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("app.") && n.ends_with(".js"))
                .unwrap_or(false)
        })
        .expect("expected app.<hash>.js");
    let map_path = entries
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("app.") && n.ends_with(".js.map"))
                .unwrap_or(false)
        })
        .expect("expected app.<hash>.js.map");

    let bundle = std::fs::read_to_string(bundle_path).unwrap();
    assert!(
        bundle.contains("//# sourceMappingURL="),
        "bundle missing sourceMappingURL comment"
    );

    let map = std::fs::read_to_string(map_path).unwrap();
    assert!(
        map.contains(r#""version":3"#) || map.contains(r#""version": 3"#),
        "map missing version: {map}"
    );
    assert!(
        map.contains("./src/app.ts"),
        "map sources missing app.ts: {map}"
    );
}
