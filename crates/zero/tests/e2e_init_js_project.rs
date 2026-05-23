//! End-to-end regression: a pure-JS project still builds and tests cleanly.
//!
//! The scaffold is TS by default; this test converts it to JS by renaming
//! files and rewriting imports, then exercises `zero test` and `zero build`.

use std::fs;

#[test]
fn js_project_after_renaming_scaffold_files() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
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

    // Move .ts files to .js, dropping type annotations and import-extension references.
    let app_ts = fs::read_to_string(web.join("src/app.ts")).unwrap();
    let app_js = app_ts.replace("./routes/home.ts", "./routes/home.js");
    fs::write(web.join("src/app.js"), app_js).unwrap();
    fs::remove_file(web.join("src/app.ts")).unwrap();

    // Plain JS rewrite of home.ts (no type imports, no annotations).
    let home_js = r#"import { html, inject } from "zero";

function Counter() {
  return html`<p>Count: ${() => inject("count").val}</p>`;
}

export default function Home() {
  return html`
    <h1>Hello from zero</h1>
    <button @click=${() => inject("count").update(n => n + 1)}>Increment</button>
    ${Counter()}
  `;
}
"#;
    fs::write(web.join("src/routes/home.js"), home_js).unwrap();
    fs::remove_file(web.join("src/routes/home.ts")).unwrap();

    let test_ts = fs::read_to_string(web.join("src/routes/home.test.ts")).unwrap();
    let test_js = test_ts.replace("./home.ts", "./home.js");
    fs::write(web.join("src/routes/home.test.js"), test_js).unwrap();
    fs::remove_file(web.join("src/routes/home.test.ts")).unwrap();

    // `zero test` still passes.
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("0 failed"));

    // `zero build` still succeeds and emits an app.<hash>.js.
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();
    let assets = tmp.path().join("dist/assets");
    let bundle = std::fs::read_dir(&assets)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("app.") && n.ends_with(".js"))
                .unwrap_or(false)
        })
        .expect("expected app.<hash>.js");
    let bundled = fs::read_to_string(&bundle).unwrap();
    assert!(
        bundled.contains(r#"__zero_require("./src/app.js")"#)
            || bundled.contains("__zero_require('./src/app.js')"),
        "bundle missing JS entry require"
    );
}
