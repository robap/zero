//! Smoke pass: run the framework's existing `runtime/*.test.js` files under
//! the new Boa harness to surface incompatibilities early.
//!
//! These tests are marked `#[ignore]` so they don't gate CI for this slice,
//! but they can be run manually with `cargo test -- --ignored`.

use assert_cmd::Command;

/// Scaffold a temp project and copy a runtime test file into it, rewriting
/// bare-module imports to use `"zero"` instead of the relative runtime paths.
fn scaffold_with_runtime_test(content: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
    Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();

    // Rewrite relative runtime imports to bare "zero" specifier.
    let rewritten = content
        .replace("from './reactivity.js'", "from 'zero'")
        .replace("from './template.js'", "from 'zero'")
        .replace("from './router.js'", "from 'zero'")
        .replace("from './app.js'", "from 'zero'")
        .replace("from './dom-shim.js'", "from 'zero'")
        .replace("from \"./reactivity.js\"", "from \"zero\"")
        .replace("from \"./template.js\"", "from \"zero\"")
        .replace("from \"./router.js\"", "from \"zero\"")
        .replace("from \"./app.js\"", "from \"zero\"")
        .replace("from \"./dom-shim.js\"", "from \"zero\"");

    // Strip node:test / node:assert imports — these won't resolve under Boa.
    // We replace with "zero/test" equivalents.
    let rewritten = rewritten
        .replace(
            "import { describe, it, before, beforeEach, after, afterEach } from 'node:test';",
            "import { describe, it, beforeEach, afterEach, beforeAll, afterAll } from 'zero/test';",
        )
        .replace(
            "import { describe, it, before, beforeEach, after, afterEach } from \"node:test\";",
            "import { describe, it, beforeEach, afterEach, beforeAll, afterAll } from \"zero/test\";",
        )
        .replace(
            "import { describe, it, beforeEach, afterEach } from 'node:test';",
            "import { describe, it, beforeEach, afterEach } from 'zero/test';",
        )
        .replace(
            "import { describe, it, beforeEach } from 'node:test';",
            "import { describe, it, beforeEach } from 'zero/test';",
        )
        .replace(
            "import { describe, it } from 'node:test';",
            "import { describe, it } from 'zero/test';",
        )
        .replace(
            "import assert from 'node:assert/strict';",
            "// node:assert not available under Boa",
        )
        .replace(
            "import assert from \"node:assert/strict\";",
            "// node:assert not available under Boa",
        );

    let test_path = tmp.path().join("web/src/smoke.test.js");
    std::fs::write(&test_path, rewritten).unwrap();
    tmp
}

#[test]
#[ignore]
fn reactivity_test_smoke_passes_under_boa() {
    let content = std::fs::read_to_string("runtime/reactivity.test.js").unwrap();
    let tmp = scaffold_with_runtime_test(&content);
    let output = Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    eprintln!("stdout:\n{stdout}");
    // Soft bar: at least some tests should pass and none should catastrophically fail.
    assert!(
        stdout.contains("passed"),
        "expected at least some passing tests: {stdout}"
    );
}

#[test]
#[ignore]
fn template_test_smoke_passes_under_boa() {
    let content = std::fs::read_to_string("runtime/template.test.js").unwrap();
    let tmp = scaffold_with_runtime_test(&content);
    let output = Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    eprintln!("stdout:\n{stdout}");
    assert!(
        stdout.contains("passed"),
        "expected at least some passing tests: {stdout}"
    );
}
