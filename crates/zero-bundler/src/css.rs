//! CSS processing for `zero build`: hash-name and copy CSS files; compile SCSS.

use std::collections::HashSet;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Hash, copy, or compile each `*.css` / `*.scss` file from `root/styles/` to `out/assets/`.
///
/// Partials (files whose name begins with `_`) are skipped. A stem collision between
/// a `.css` and a `.scss` file with the same name is a hard error.
///
/// # Parameters
/// - `root`: project root directory (the `[project] root` path).
/// - `out`: build output directory (the `[build] out` path).
/// - `emit_sourcemap`: write an external `.map` file and append `sourceMappingURL` comment.
///
/// # Returns
/// A sorted list of `(source-relative, output-relative)` pairs, e.g.
/// `("styles/app.scss", "assets/app.5e8d9f01.css")`.
pub fn process_css(
    root: &Path,
    out: &Path,
    emit_sourcemap: bool,
) -> anyhow::Result<Vec<(String, String)>> {
    let styles_dir = root.join("styles");
    let assets_dir = out.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    let entries = match std::fs::read_dir(&styles_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut all_entries: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry?;
        all_entries.push(entry.path());
    }

    // Collision check: find stems present in both .css and .scss (excluding partials).
    let css_stems: HashSet<String> = all_entries
        .iter()
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == "css" && !name.starts_with('_')
        })
        .filter_map(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    let scss_stems: HashSet<String> = all_entries
        .iter()
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == "scss" && !name.starts_with('_')
        })
        .filter_map(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    let collisions: Vec<&String> = css_stems.intersection(&scss_stems).collect();
    if !collisions.is_empty() {
        let stem = collisions[0];
        anyhow::bail!("styles/{stem}: both .scss and .css present; rename one");
    }

    let mut pairs = Vec::new();

    for path in &all_entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("style");

        if name.starts_with('_') {
            continue;
        }

        match ext {
            "css" => {
                let bytes = std::fs::read(path)?;
                let hash = &format!("{:x}", Sha256::digest(&bytes))[..8];
                let out_filename = format!("{stem}.{hash}.css");
                std::fs::write(assets_dir.join(&out_filename), &bytes)?;
                pairs.push((format!("styles/{name}"), format!("assets/{out_filename}")));
            }
            "scss" => {
                let source = std::fs::read_to_string(path)?;
                let logical = format!("styles/{name}");
                let sass_opts = zero_sass::SassOptions {
                    filename: &logical,
                    inline_source_map: false,
                    emit_source_map: emit_sourcemap,
                    load_paths: &[],
                };
                let compiled = zero_sass::compile_scss(&source, path, &sass_opts)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                let hash = &format!("{:x}", Sha256::digest(compiled.code.as_bytes()))[..8];
                let out_filename = format!("{stem}.{hash}.css");

                let css_body = if emit_sourcemap {
                    let map_filename = format!("{out_filename}.map");
                    if let Some(ref map_json) = compiled.source_map {
                        std::fs::write(assets_dir.join(&map_filename), map_json)?;
                    }
                    let mut body = compiled.code.clone();
                    if !body.ends_with('\n') {
                        body.push('\n');
                    }
                    body.push_str(&format!("/*# sourceMappingURL={out_filename}.map */\n"));
                    body
                } else {
                    compiled.code.clone()
                };

                std::fs::write(assets_dir.join(&out_filename), &css_body)?;
                pairs.push((
                    format!("styles/{stem}.scss"),
                    format!("assets/{out_filename}"),
                ));
            }
            _ => continue,
        }
    }

    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn process_css_handles_css_only() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(root.path().join("styles/app.css"), "body { color: red; }").unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), false).unwrap();
        assert_eq!(pairs.len(), 1);
        let (src, dst) = &pairs[0];
        assert_eq!(src, "styles/app.css");
        assert!(dst.starts_with("assets/app.") && dst.ends_with(".css"));
        assert!(out.path().join(dst).exists());
    }

    #[test]
    fn process_css_returns_empty_when_no_styles_dir() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), false).unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn process_css_compiles_scss() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(
            root.path().join("styles/app.scss"),
            "$c: red; body { color: $c; }",
        )
        .unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), false).unwrap();
        assert_eq!(pairs.len(), 1);
        let (src, dst) = &pairs[0];
        assert_eq!(src, "styles/app.scss");
        assert!(
            dst.starts_with("assets/app.") && dst.ends_with(".css"),
            "unexpected dst: {dst}"
        );
        let css = std::fs::read_to_string(out.path().join(dst)).unwrap();
        assert!(css.contains("red"), "compiled CSS missing 'red': {css}");
        assert!(!css.contains("$c"), "SCSS variable leaked: {css}");
    }

    #[test]
    fn process_css_skips_underscore_partials() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(
            root.path().join("styles/app.scss"),
            "@use 'buttons'; .btn { padding: buttons.$btn-padding; }",
        )
        .unwrap();
        std::fs::write(
            root.path().join("styles/_buttons.scss"),
            "$btn-padding: 8px;",
        )
        .unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), false).unwrap();
        assert_eq!(pairs.len(), 1, "should have exactly 1 pair, got: {pairs:?}");
        assert_eq!(pairs[0].0, "styles/app.scss");
        // No _buttons.hash.css should exist
        let assets = std::fs::read_dir(out.path().join("assets")).unwrap();
        for entry in assets {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(!name.contains("_buttons"), "partial was emitted: {name}");
        }
    }

    #[test]
    fn process_css_emits_sourcemap_when_enabled() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(
            root.path().join("styles/app.scss"),
            "$c: red; body { color: $c; }",
        )
        .unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), true).unwrap();
        assert_eq!(pairs.len(), 1);
        let (_, dst) = &pairs[0];
        let css_body = std::fs::read_to_string(out.path().join(dst)).unwrap();
        let map_path = out.path().join(format!("{dst}.map"));
        assert!(
            map_path.exists(),
            "sourcemap file missing: {}",
            map_path.display()
        );
        let map_content = std::fs::read_to_string(&map_path).unwrap();
        assert!(
            map_content.contains("\"version\":3"),
            "sourcemap missing version"
        );
        assert!(
            css_body.contains("sourceMappingURL="),
            "CSS missing sourceMappingURL comment: {css_body}"
        );
    }

    #[test]
    fn process_css_no_sourcemap_by_default() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(
            root.path().join("styles/app.scss"),
            "$c: red; body { color: $c; }",
        )
        .unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), false).unwrap();
        let (_, dst) = &pairs[0];
        let map_path = out.path().join(format!("{dst}.map"));
        assert!(!map_path.exists(), "sourcemap emitted when disabled");
        let css_body = std::fs::read_to_string(out.path().join(dst)).unwrap();
        assert!(
            !css_body.contains("sourceMappingURL"),
            "sourceMappingURL present when disabled"
        );
    }

    #[test]
    fn process_css_errors_on_stem_collision() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(root.path().join("styles/app.css"), "body {}").unwrap();
        std::fs::write(root.path().join("styles/app.scss"), "body {}").unwrap();
        let out = tempdir().unwrap();
        let err =
            process_css(root.path(), out.path(), false).expect_err("expected collision error");
        let msg = err.to_string();
        assert!(
            msg.contains("app") || msg.contains(".scss") || msg.contains(".css"),
            "error message missing stem info: {msg}"
        );
    }

    #[test]
    fn process_css_propagates_scss_errors() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(root.path().join("styles/app.scss"), "body { color: ; }").unwrap();
        let out = tempdir().unwrap();
        let err = process_css(root.path(), out.path(), false).expect_err("expected error");
        let msg = err.to_string();
        assert!(
            msg.contains("styles/app.scss"),
            "error message missing file path: {msg}"
        );
    }

    #[test]
    fn process_css_sorts_pairs_deterministically() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(
            root.path().join("styles/b.scss"),
            "$c: blue; body { color: $c; }",
        )
        .unwrap();
        std::fs::write(root.path().join("styles/a.css"), "a { color: green; }").unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path(), false).unwrap();
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].0 < pairs[1].0, "pairs not sorted: {:?}", pairs);
    }
}
