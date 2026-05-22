//! Helpers shared across integration tests. Cargo treats `tests/common/`
//! as a non-test directory; each test file pulls this in via `mod common;`.

use std::path::Path;

/// Recursively copy `src` into `dst`, skipping any entry whose top-level
/// name appears in `skip_top_level`. Empty directories at the source are
/// not created at the destination.
///
/// # Parameters
/// - `src`: source directory to copy from.
/// - `dst`: destination directory; created if missing.
/// - `skip_top_level`: directory or file names (immediate children of
///   `src`) to omit from the copy.
pub fn copy_dir_filtered(src: &Path, dst: &Path, skip_top_level: &[&str]) {
    copy_inner(src, dst, src, skip_top_level);
}

fn copy_inner(src_root: &Path, dst_root: &Path, src_cur: &Path, skip_top_level: &[&str]) {
    std::fs::create_dir_all(dst_root).unwrap();
    for entry in std::fs::read_dir(src_cur).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Only the immediate children of src_root are subject to the
        // skip list; nested entries with the same name are kept.
        if src_cur == src_root && skip_top_level.contains(&name_str.as_ref()) {
            continue;
        }
        let rel = path.strip_prefix(src_root).unwrap();
        let target = dst_root.join(rel);
        if path.is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_inner(src_root, dst_root, &path, skip_top_level);
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// Copy the in-repo `showcase/` user files into a fresh tempdir and run
/// `zero update --yes` to materialize the framework manifest into
/// `<tempdir>/.zero/`. Returns the owned tempdir so the caller can build,
/// test, or dev-serve against it.
///
/// `showcase/.zero/` is gitignored, so it never participates in the copy;
/// the framework manifest is the single source of truth.
#[allow(dead_code)]
pub fn prepare_showcase() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let src = workspace_root().join("showcase");
    copy_dir_filtered(&src, tmp.path(), &[".zero", "dist", "node_modules"]);

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("update")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();

    tmp
}

/// Copy the in-repo `examples/<name>/` user files into a fresh tempdir and
/// run `zero update --yes` to materialize the framework manifest into
/// `<tempdir>/.zero/`. Mirrors `prepare_showcase`; the difference is the
/// source subdirectory.
#[allow(dead_code)]
pub fn prepare_example(name: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let src = workspace_root().join("examples").join(name);
    copy_dir_filtered(&src, tmp.path(), &[".zero", "dist", "node_modules"]);

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("update")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();

    tmp
}

/// Resolve the workspace root from `crates/zero/`'s manifest dir.
///
/// `env!("CARGO_MANIFEST_DIR")` resolves to `zero/crates/zero` post-split,
/// so walking up two levels lands at the workspace root where `showcase/`,
/// `examples/`, and `runtime/` live.
#[allow(dead_code)]
pub fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/zero/ should be two levels below the workspace root")
        .to_path_buf()
}
