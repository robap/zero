//! Cheap guard against accidental regressions of the canonical
//! `examples/<name>/web/` subdir layout. Each example's `zero.toml`
//! must declare `root = "web"`, and the user files (`index.html`,
//! `src/app.ts`) must live under `web/`.

use std::path::Path;

fn assert_canonical_layout(name: &str) {
    // Walk up from crates/zero/ to the workspace root.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/zero/ should be two levels below the workspace root");
    let example = repo.join("examples").join(name);
    let toml = std::fs::read_to_string(example.join("zero.toml"))
        .unwrap_or_else(|e| panic!("read zero.toml for {name}: {e}"));
    assert!(
        toml.contains("root = \"web\""),
        "examples/{name}/zero.toml must declare root = \"web\":\n{toml}"
    );
    assert!(
        example.join("web/index.html").is_file(),
        "examples/{name}/web/index.html missing"
    );
    assert!(
        example.join("web/src/app.ts").is_file(),
        "examples/{name}/web/src/app.ts missing"
    );
}

#[test]
fn counter_has_web_subdir() {
    assert_canonical_layout("counter");
}

#[test]
fn todos_has_web_subdir() {
    assert_canonical_layout("todos");
}

#[test]
fn tracker_has_web_subdir() {
    assert_canonical_layout("tracker");
}
