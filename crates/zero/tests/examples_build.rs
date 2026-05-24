//! Integration test: every shipped `examples/<name>/` project builds end
//! to end via `zero build`. The example's `.zero/` is gitignored — the
//! helper materializes it from the framework manifest before the build
//! runs.

use std::path::Path;

mod common;

#[test]
#[ignore = "slow"]
fn counter_builds() {
    build_example("counter");
}

#[test]
#[ignore = "slow"]
fn todos_builds() {
    build_example("todos");
}

#[test]
#[ignore = "slow"]
fn tracker_builds() {
    build_example("tracker");
}

fn build_example(name: &str) {
    let tmp = common::prepare_example(name);

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let dist = tmp.path().join("dist");
    assert!(
        dist.join("index.html").exists(),
        "missing dist/index.html for example `{name}`"
    );

    let assets = dist.join("assets");
    let entries: Vec<_> = std::fs::read_dir(&assets)
        .unwrap_or_else(|_| panic!("example `{name}` produced no dist/assets/"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    let js = entries
        .iter()
        .find(|p| has_prefix_ext(p, "app.", "js"))
        .unwrap_or_else(|| panic!("example `{name}` missing app.<hash>.js"));
    let js_size = std::fs::metadata(js)
        .unwrap_or_else(|_| panic!("example `{name}`: cannot stat {}", js.display()))
        .len();
    assert!(js_size > 0, "example `{name}` produced an empty app bundle");
}

fn has_prefix_ext(path: &Path, prefix: &str, ext: &str) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with(prefix) && name.ends_with(&format!(".{ext}"))
}
