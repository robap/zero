//! Gitignore-aware walker that yields user-authored JS / TS files under
//! `<root>/src/`.

use std::path::{Path, PathBuf};

/// Yield every user-authored `.ts` / `.tsx` / `.js` / `.jsx` file under
/// `<root>/src/`.
///
/// Honors `.gitignore` and unconditionally skips framework-owned and build
/// directories (`.zero/`, `dist/`, `node_modules/`, `coverage/`,
/// `mutation/`, `target/`). Test files (`*.test.{ts,js,tsx,jsx}` and
/// `*.spec.{ts,js,tsx,jsx}`) are returned; per-rule scope decides whether
/// to apply.
///
/// Returns an empty vec if `<root>/src/` does not exist (the
/// framework-internal directories like `runtime/` aren't user projects).
pub fn user_js_files(root: &Path) -> Vec<PathBuf> {
    let src_dir = root.join("src");
    if !src_dir.is_dir() {
        return Vec::new();
    }
    let mut out: Vec<PathBuf> = Vec::new();
    let mut builder = ignore::WalkBuilder::new(&src_dir);
    builder
        .standard_filters(true)
        .add_custom_ignore_filename(".gitignore");

    let mut overrides = ignore::overrides::OverrideBuilder::new(&src_dir);
    for pat in [
        "!.zero/**",
        "!**/.zero/**",
        "!dist/**",
        "!**/dist/**",
        "!node_modules/**",
        "!**/node_modules/**",
        "!coverage/**",
        "!**/coverage/**",
        "!mutation/**",
        "!**/mutation/**",
        "!target/**",
        "!**/target/**",
    ] {
        let _ = overrides.add(pat);
    }
    if let Ok(ov) = overrides.build() {
        builder.overrides(ov);
    }

    for entry in builder.build().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        if matches!(
            ext.as_deref(),
            Some("ts") | Some("tsx") | Some("js") | Some("jsx")
        ) {
            out.push(path.to_path_buf());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn rels(root: &Path, files: &[PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn walker_yields_ts_js_tsx_jsx_under_src() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src/components")).unwrap();
        fs::write(root.join("src/app.ts"), "").unwrap();
        fs::write(root.join("src/components/Btn.tsx"), "").unwrap();
        fs::write(root.join("src/util.js"), "").unwrap();
        fs::write(root.join("src/legacy.jsx"), "").unwrap();
        fs::write(root.join("src/styles.css"), "").unwrap();
        let files = user_js_files(root);
        let r = rels(root, &files);
        assert!(r.iter().any(|s| s == "src/app.ts"), "missing app.ts: {r:?}");
        assert!(
            r.iter().any(|s| s == "src/components/Btn.tsx"),
            "missing Btn.tsx: {r:?}"
        );
        assert!(r.iter().any(|s| s == "src/util.js"), "missing util.js");
        assert!(
            r.iter().any(|s| s == "src/legacy.jsx"),
            "missing legacy.jsx"
        );
        assert!(
            !r.iter().any(|s| s.ends_with(".css")),
            "should not include .css"
        );
    }

    #[test]
    fn walker_excludes_dot_zero_dist_node_modules_target() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src/.zero")).unwrap();
        fs::create_dir_all(root.join("src/dist")).unwrap();
        fs::create_dir_all(root.join("src/node_modules/foo")).unwrap();
        fs::create_dir_all(root.join("src/target")).unwrap();
        fs::write(root.join("src/.zero/x.ts"), "").unwrap();
        fs::write(root.join("src/dist/b.js"), "").unwrap();
        fs::write(root.join("src/node_modules/foo/a.ts"), "").unwrap();
        fs::write(root.join("src/target/x.js"), "").unwrap();
        fs::write(root.join("src/app.ts"), "").unwrap();
        let files = user_js_files(root);
        let r = rels(root, &files);
        assert!(r.iter().any(|s| s == "src/app.ts"));
        for forbidden in [".zero/", "dist/", "node_modules/", "target/"] {
            assert!(
                !r.iter().any(|s| s.contains(forbidden)),
                "walker yielded forbidden path containing {forbidden}: {r:?}"
            );
        }
    }

    #[test]
    fn walker_returns_empty_when_no_src_dir() {
        let dir = tempdir().unwrap();
        let files = user_js_files(dir.path());
        assert!(files.is_empty(), "expected empty, got {files:?}");
    }

    #[test]
    fn walker_yields_test_and_spec_files() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/foo.test.ts"), "").unwrap();
        fs::write(root.join("src/bar.spec.js"), "").unwrap();
        let files = user_js_files(root);
        let r = rels(root, &files);
        assert!(r.iter().any(|s| s == "src/foo.test.ts"), "missing test");
        assert!(r.iter().any(|s| s == "src/bar.spec.js"), "missing spec");
    }
}
