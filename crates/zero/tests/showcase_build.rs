//! Integration test: the in-repo `showcase/` project builds end-to-end.
//!
//! The showcase's `.zero/` directory is gitignored — `zero update --yes`
//! populates it from the framework manifest before the build runs. This
//! exercises the resolver/bundler against every component shipped in
//! `.zero/components/`.

use std::path::Path;

mod common;

#[test]
fn showcase_build_emits_components_in_bundle_and_css() {
    let tmp = common::prepare_showcase();

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let dist = tmp.path().join("dist");
    assert!(dist.join("index.html").exists(), "missing dist/index.html");

    let assets = dist.join("assets");
    let entries: Vec<_> = std::fs::read_dir(&assets)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    let js = entries
        .iter()
        .find(|p| has_prefix_ext(p, "app.", "js"))
        .expect("expected app.<hash>.js");
    let css = entries
        .iter()
        .find(|p| has_prefix_ext(p, "app.", "css"))
        .expect("expected app.<hash>.css");

    let css_body = std::fs::read_to_string(css).unwrap();
    assert!(
        css_body.contains("@layer components"),
        "compiled CSS missing @layer components"
    );

    let js_body = std::fs::read_to_string(js).unwrap();
    assert!(
        js_body.contains("__zero_define('./.zero/components/index.ts'"),
        "bundle missing components index define"
    );
}

fn has_prefix_ext(path: &Path, prefix: &str, ext: &str) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with(prefix) && name.ends_with(&format!(".{ext}"))
}
