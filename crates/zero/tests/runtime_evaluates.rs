//! Optional integration test: write `runtime_module()` to a temp `.mjs` file
//! and evaluate it with Node, asserting clean exit. Skipped if `node` is not
//! on PATH so `cargo test` doesn't require Node.

use std::process::Command;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn runtime_module_evaluates_under_node() {
    if !node_available() {
        eprintln!("skipping: `node` not on PATH");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("runtime.mjs");
    let body = zero_runtime::runtime_module();
    // The runtime reads `globalThis.document`; provide a minimal stub so
    // the module can be imported without throwing in a bare Node env.
    let prelude = "globalThis.document = globalThis.document || {};\n";
    std::fs::write(&path, format!("{prelude}{body}")).unwrap();

    let output = Command::new("node")
        .args([
            "--input-type=module",
            "--eval",
            &format!("await import('file://{}');", path.display()),
        ])
        .output()
        .expect("failed to spawn node");

    assert!(
        output.status.success(),
        "node evaluation failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
