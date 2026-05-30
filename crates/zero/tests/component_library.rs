//! Integration test: `zero test` in the showcase passes every component
//! test. Acts as CI for the component library since each component ships
//! its own `*.test.ts` under `.zero/components/`.

use predicates::Predicate;
use predicates::str::contains;

mod common;

#[test]
#[ignore = "slow"]
fn showcase_test_runs_all_component_tests() {
    let tmp = common::prepare_showcase();

    let output = assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("test")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "zero test must exit cleanly:\n{stdout}"
    );
    assert!(
        contains("0 failed").eval(&stdout),
        "expected `0 failed` in test report:\n{stdout}"
    );

    // Each shipped component must show up in the report. The list mirrors
    // the manifest's `COMPONENT_NAMES` and is intentionally hard-coded so a
    // missing test (e.g. a component shipped without its `.test.ts`) is a
    // clear failure here.
    for name in [
        "Avatar",
        "Badge",
        "Button",
        "Card",
        "Checkbox",
        "Combobox",
        "Dialog",
        "Drawer",
        "Input",
        "Pagination",
        "Radio",
        "Select",
        "Spinner",
        "Table",
        "Tabs",
        "TextArea",
        "Toast",
        "Toggle",
    ] {
        assert!(
            stdout.contains(name),
            "test report missing `{name}` (was a component test dropped?):\n{stdout}"
        );
    }
}
