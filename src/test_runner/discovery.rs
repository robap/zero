//! Test file discovery: walk the project root for `*.test.js` / `*.spec.js`.

use std::path::PathBuf;

/// Options for the discovery pass.
pub struct DiscoveryOpts<'a> {
    pub root: &'a std::path::Path,
    pub out_dir: &'a std::path::Path,
    pub target: Option<&'a str>,
}

/// Discovery output: sorted list of absolute paths.
#[derive(Debug)]
pub struct DiscoveryResult {
    pub files: Vec<PathBuf>,
}

/// Discover test files under `opts.root`.
///
/// # Parameters
/// - `opts.root`: absolute path to the project root.
/// - `opts.out_dir`: build output directory (files inside are skipped).
/// - `opts.target`: optional substring filter or explicit file path.
///
/// # Returns
/// `Ok(DiscoveryResult)` with sorted absolute paths on success.
pub fn discover(opts: DiscoveryOpts<'_>) -> anyhow::Result<DiscoveryResult> {
    let root = opts.root;
    let out_dir = opts.out_dir;

    // If target resolves to an existing regular file, bypass discovery.
    if let Some(t) = opts.target {
        let candidate = root.join(t);
        if candidate.is_file() {
            let abs = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone());
            return Ok(DiscoveryResult { files: vec![abs] });
        }
    }

    // Walk root, collecting test files.
    let mut files: Vec<PathBuf> = Vec::new();
    walk_dir(root, out_dir, &mut files)?;
    files.sort();

    // Detect TS/JS collisions before filtering. Bail with both paths in the
    // message — same rule the bundler enforces for the entry point.
    for p in &files {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let (stem_kind, ts_kind) = if let Some(stem) = name.strip_suffix(".test.ts") {
            (stem.to_string(), ".test")
        } else if let Some(stem) = name.strip_suffix(".spec.ts") {
            (stem.to_string(), ".spec")
        } else {
            continue;
        };
        let sibling = p.with_file_name(format!("{stem_kind}{ts_kind}.js"));
        if sibling.is_file() {
            anyhow::bail!(
                "zero test: {} and {} both exist; remove one",
                p.display(),
                sibling.display()
            );
        }
    }

    // Apply substring filter if target was provided but didn't resolve to a file.
    if let Some(t) = opts.target {
        files.retain(|p| {
            let rel = p.strip_prefix(root).unwrap_or(p);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            rel_str.contains(t)
        });
    }

    Ok(DiscoveryResult { files })
}

/// Recursively walk `dir`, appending matching files to `out`.
fn walk_dir(
    dir: &std::path::Path,
    out_dir: &std::path::Path,
    out: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden entries.
        if name_str.starts_with('.') {
            continue;
        }
        // Skip node_modules.
        if name_str == "node_modules" {
            continue;
        }
        // Skip the build output directory.
        if path.starts_with(out_dir) {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, out_dir, out)?;
        } else if is_test_file(&name_str) {
            out.push(path);
        }
    }

    Ok(())
}

/// Returns true if the filename ends with `.test.{js,ts}` or `.spec.{js,ts}`.
fn is_test_file(name: &str) -> bool {
    name.ends_with(".test.js")
        || name.ends_with(".spec.js")
        || name.ends_with(".test.ts")
        || name.ends_with(".spec.ts")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_root() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn opts<'a>(
        root: &'a std::path::Path,
        out_dir: &'a std::path::Path,
        target: Option<&'a str>,
    ) -> DiscoveryOpts<'a> {
        DiscoveryOpts {
            root,
            out_dir,
            target,
        }
    }

    #[test]
    fn collects_test_and_spec_files_but_not_plain_js() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::write(root.join("a.test.js"), "").unwrap();
        fs::write(root.join("b.test.js"), "").unwrap();
        fs::write(root.join("c.spec.js"), "").unwrap();
        fs::write(root.join("d.js"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        assert_eq!(result.files.len(), 3);
    }

    #[test]
    fn result_is_sorted() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::write(root.join("z.test.js"), "").unwrap();
        fs::write(root.join("a.test.js"), "").unwrap();
        fs::write(root.join("m.spec.js"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        let names: Vec<_> = result
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn substring_filter_matches_relative_path() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join("routes")).unwrap();
        fs::write(root.join("routes").join("home.test.js"), "").unwrap();
        fs::write(root.join("app.test.js"), "").unwrap();

        let result = discover(opts(root, &out, Some("routes"))).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].to_string_lossy().contains("routes"));
    }

    #[test]
    fn explicit_file_target_bypasses_discovery() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::write(root.join("something.js"), "").unwrap();
        fs::write(root.join("other.test.js"), "").unwrap();

        let result = discover(opts(root, &out, Some("something.js"))).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].to_string_lossy().ends_with("something.js"));
    }

    #[test]
    fn skips_hidden_directories() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden").join("foo.test.js"), "").unwrap();
        fs::write(root.join("visible.test.js"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(
            result.files[0]
                .to_string_lossy()
                .ends_with("visible.test.js")
        );
    }

    #[test]
    fn skips_node_modules() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules").join("bar.test.js"), "").unwrap();
        fs::write(root.join("real.test.js"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].to_string_lossy().ends_with("real.test.js"));
    }

    #[test]
    fn skips_out_dir() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("bundle.test.js"), "").unwrap();
        fs::write(root.join("src.test.js"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].to_string_lossy().ends_with("src.test.js"));
    }

    #[test]
    fn collects_test_ts_and_spec_ts() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");
        fs::write(root.join("a.test.ts"), "").unwrap();
        fs::write(root.join("b.spec.ts"), "").unwrap();
        fs::write(root.join("c.test.js"), "").unwrap();
        let result = discover(opts(root, &out, None)).unwrap();
        assert_eq!(result.files.len(), 3);
    }

    #[test]
    fn collision_ts_and_js_for_same_logical_name_errors() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");
        fs::write(root.join("home.test.ts"), "").unwrap();
        fs::write(root.join("home.test.js"), "").unwrap();
        let err = discover(opts(root, &out, None)).expect_err("should error");
        let msg = format!("{err}");
        assert!(msg.contains("home.test.ts"), "msg missing ts path: {msg}");
        assert!(msg.contains("home.test.js"), "msg missing js path: {msg}");
    }

    #[test]
    fn empty_tree_returns_empty() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        let result = discover(opts(root, &out, None)).unwrap();
        assert!(result.files.is_empty());
    }
}
