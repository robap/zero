//! Test file discovery: walk the project root for `*.test.js` / `*.spec.js`.

use std::path::PathBuf;

/// Options for the discovery pass.
pub struct DiscoveryOpts<'a> {
    /// Absolute path to the project root the walk descends from.
    pub root: &'a std::path::Path,
    /// Build output directory; files inside it are skipped.
    pub out_dir: &'a std::path::Path,
    /// Additional directories to skip during the walk, beyond `out_dir`
    /// (e.g. `build/` in no-config mode). May be empty.
    pub extra_skip_dirs: &'a [PathBuf],
    /// Optional explicit file path or substring filter. An existing file is
    /// run directly (see `cwd`); otherwise it filters the walk by substring.
    pub target: Option<&'a str>,
    /// Base for cwd-first resolution of an explicit-file `target`. In an
    /// in-project run this is the real CWD (distinct from `root`); in
    /// no-config mode it equals `root`.
    pub cwd: &'a std::path::Path,
}

/// Discovery output: sorted list of absolute paths.
#[derive(Debug)]
pub struct DiscoveryResult {
    pub files: Vec<PathBuf>,
}

/// Discover test files under the root, per [`DiscoveryOpts`].
///
/// # Returns
/// `Ok(DiscoveryResult)` with sorted absolute paths on success.
pub fn discover(opts: DiscoveryOpts<'_>) -> anyhow::Result<DiscoveryResult> {
    let root = opts.root;

    // If target resolves to an existing regular file, bypass discovery.
    // Resolve cwd-first, then fall back to project-root-relative; the first
    // base that names a regular file wins. When `cwd == root` the first base
    // wins and the duplicate is harmless.
    if let Some(t) = opts.target {
        for base in [opts.cwd, root] {
            let candidate = base.join(t);
            if candidate.is_file() {
                let abs = candidate.canonicalize().unwrap_or(candidate);
                return Ok(DiscoveryResult { files: vec![abs] });
            }
        }
    }

    // Walk root, collecting test files. Skip the build out-dir plus any
    // caller-supplied extra dirs (e.g. `build/` in no-config mode).
    let mut skip_dirs: Vec<PathBuf> = vec![opts.out_dir.to_path_buf()];
    skip_dirs.extend(opts.extra_skip_dirs.iter().cloned());
    let mut files: Vec<PathBuf> = Vec::new();
    walk_dir(root, &skip_dirs, &mut files)?;
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

/// Recursively walk `dir`, appending matching files to `out`. Any path under
/// one of `skip_dirs` is skipped.
fn walk_dir(
    dir: &std::path::Path,
    skip_dirs: &[PathBuf],
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

        // Skip hidden entries — narrow exception: descend into `.zero/`
        // but only its `components/` subtree, so framework-shipped
        // component tests get picked up by `zero test`.
        if name_str.starts_with('.') {
            if name_str == ".zero" && path.is_dir() {
                walk_dot_zero(&path, skip_dirs, out)?;
            }
            continue;
        }
        // Skip node_modules.
        if name_str == "node_modules" {
            continue;
        }
        // Skip the build output directory and any extra skip dirs.
        if skip_dirs.iter().any(|d| path.starts_with(d)) {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, skip_dirs, out)?;
        } else if is_test_file(&name_str) {
            out.push(path);
        }
    }

    Ok(())
}

/// Walk the `components/` subtree of a `.zero/` directory. All other
/// subtrees of `.zero/` are framework-owned scaffolding and are not
/// searched for tests.
fn walk_dot_zero(
    dot_zero: &std::path::Path,
    skip_dirs: &[PathBuf],
    out: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let components = dot_zero.join("components");
    if components.is_dir() {
        walk_dir(&components, skip_dirs, out)?;
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
            extra_skip_dirs: &[],
            target,
            cwd: root,
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
    fn walks_into_dot_zero_components() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join(".zero/components")).unwrap();
        fs::write(root.join(".zero/components/Foo.test.ts"), "").unwrap();
        fs::write(root.join("visible.test.ts"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        let names: Vec<String> = result
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().any(|n| n.ends_with("Foo.test.ts")),
            "discovery did not pick up .zero/components/Foo.test.ts: {names:?}"
        );
    }

    #[test]
    fn does_not_walk_into_other_dot_zero_subdirs() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join(".zero/styles")).unwrap();
        fs::write(root.join(".zero/styles/extra.test.ts"), "").unwrap();
        fs::write(root.join("visible.test.ts"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        let names: Vec<String> = result
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(
            !names.iter().any(|n| n.contains(".zero/styles")),
            "discovery wrongly picked up .zero/styles/extra.test.ts: {names:?}"
        );
    }

    #[test]
    fn still_skips_other_hidden_dirs() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/foo.test.ts"), "").unwrap();
        fs::write(root.join("visible.test.ts"), "").unwrap();

        let result = discover(opts(root, &out, None)).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(
            result.files[0]
                .to_string_lossy()
                .ends_with("visible.test.ts")
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
    fn explicit_file_resolves_cwd_first() {
        let cwd_dir = make_root();
        let root_dir = make_root();
        let cwd = cwd_dir.path();
        let root = root_dir.path();
        let out = root.join("dist");

        fs::write(cwd.join("a.test.ts"), "").unwrap();
        fs::write(root.join("a.test.ts"), "").unwrap();

        let result = discover(DiscoveryOpts {
            root,
            out_dir: &out,
            extra_skip_dirs: &[],
            target: Some("a.test.ts"),
            cwd,
        })
        .unwrap();
        assert_eq!(result.files.len(), 1);
        // The cwd copy wins over the root copy.
        let resolved = result.files[0].canonicalize().unwrap();
        let expected = cwd.join("a.test.ts").canonicalize().unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn explicit_file_falls_back_to_root() {
        let cwd_dir = make_root();
        let root_dir = make_root();
        let cwd = cwd_dir.path();
        let root = root_dir.path();
        let out = root.join("dist");

        // File exists only under root; cwd copy absent.
        fs::write(root.join("only.test.ts"), "").unwrap();

        let result = discover(DiscoveryOpts {
            root,
            out_dir: &out,
            extra_skip_dirs: &[],
            target: Some("only.test.ts"),
            cwd,
        })
        .unwrap();
        assert_eq!(result.files.len(), 1);
        let resolved = result.files[0].canonicalize().unwrap();
        let expected = root.join("only.test.ts").canonicalize().unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn non_file_target_still_applies_substring_filter() {
        let cwd_dir = make_root();
        let root_dir = make_root();
        let cwd = cwd_dir.path();
        let root = root_dir.path();
        let out = root.join("dist");

        fs::create_dir_all(root.join("routes")).unwrap();
        fs::write(root.join("routes").join("home.test.js"), "").unwrap();
        fs::write(root.join("app.test.js"), "").unwrap();

        // "routes" names no file under cwd or root → substring filter applies.
        let result = discover(DiscoveryOpts {
            root,
            out_dir: &out,
            extra_skip_dirs: &[],
            target: Some("routes"),
            cwd,
        })
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].to_string_lossy().contains("routes"));
    }

    #[test]
    fn extra_skip_dirs_are_skipped() {
        let dir = make_root();
        let root = dir.path();
        let out = root.join("dist");
        let build = root.join("build");

        fs::create_dir_all(&build).unwrap();
        fs::write(build.join("foo.test.ts"), "").unwrap();
        fs::write(root.join("foo.test.ts"), "").unwrap();

        let extra = vec![build.clone()];
        let result = discover(DiscoveryOpts {
            root,
            out_dir: &out,
            extra_skip_dirs: &extra,
            target: None,
            cwd: root,
        })
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(
            !result.files[0].starts_with(&build),
            "build/ should be skipped"
        );
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
