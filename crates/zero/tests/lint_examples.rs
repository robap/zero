//! Integration tests asserting that the in-repo example apps lint clean.

mod common;

use assert_cmd::Command;
use common::{prepare_example, prepare_showcase};

#[test]
#[ignore = "slow"]
fn tracker_lints_clean() {
    let tmp = prepare_example("tracker");
    let out = Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .arg("--quiet")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "tracker should lint clean. stdout={stdout} stderr={stderr}"
    );
}

#[test]
#[ignore = "slow"]
fn showcase_lints_clean() {
    let tmp = prepare_showcase();
    let out = Command::cargo_bin("zero")
        .unwrap()
        .arg("lint")
        .arg("--quiet")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "showcase should lint clean. stdout={stdout} stderr={stderr}"
    );
}
