//! Gitignore-aware walker that yields user-authored SCSS / CSS files.

use std::path::{Path, PathBuf};

/// Yield every user-authored `.scss` / `.css` file under `root`.
///
/// Honors `.gitignore` and unconditionally skips framework-owned and build
/// directories (`.zero/`, `dist/`, `node_modules/`, `coverage/`,
/// `mutation/`, `target/`).
pub fn user_scss_files(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .standard_filters(true)
        .add_custom_ignore_filename(".gitignore");

    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
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
        if matches!(ext.as_deref(), Some("scss") | Some("css")) {
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

    #[test]
    fn walker_excludes_dot_zero_and_dist() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".zero/styles")).unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::create_dir_all(root.join("node_modules/foo")).unwrap();
        fs::create_dir_all(root.join("styles")).unwrap();
        fs::write(root.join(".zero/styles/_tokens.scss"), "// ignored").unwrap();
        fs::write(root.join("dist/bundle.css"), "/* ignored */").unwrap();
        fs::write(root.join("node_modules/foo/x.scss"), "// ignored").unwrap();
        fs::write(root.join("styles/app.scss"), ".x{}").unwrap();

        let files = user_scss_files(root);
        let rels: Vec<String> = files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();

        assert!(
            rels.iter().any(|s| s == "styles/app.scss"),
            "expected styles/app.scss in {rels:?}"
        );
        for forbidden in [".zero", "dist/", "node_modules/"] {
            assert!(
                !rels.iter().any(|s| s.contains(forbidden)),
                "walker yielded forbidden path containing {forbidden}: {rels:?}"
            );
        }
    }

    #[test]
    fn walker_honors_gitignore() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("vendor")).unwrap();
        fs::create_dir_all(root.join("styles")).unwrap();
        fs::write(root.join(".gitignore"), "vendor/\n").unwrap();
        fs::write(root.join("vendor/x.scss"), "// ignored").unwrap();
        fs::write(root.join("styles/app.scss"), ".x{}").unwrap();

        let files = user_scss_files(root);
        let rels: Vec<String> = files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert!(
            rels.iter().any(|s| s == "styles/app.scss"),
            "expected styles/app.scss in {rels:?}"
        );
        assert!(
            !rels.iter().any(|s| s.starts_with("vendor/")),
            "walker did not honor .gitignore: {rels:?}"
        );
    }
}
