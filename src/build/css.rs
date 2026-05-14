//! CSS processing for `zero build`: hash-name and copy CSS files.

use std::path::Path;

use sha2::{Digest, Sha256};

/// Hash and copy each `*.css` file from `root/styles/` to `out/assets/`.
///
/// # Parameters
/// - `root`: project root directory (the `[project] root` path).
/// - `out`: build output directory (the `[build] out` path).
///
/// # Returns
/// A list of `(source-relative, output-relative)` pairs, e.g.
/// `("styles/app.css", "assets/app.5e8d9f01.css")`.
pub fn process_css(root: &Path, out: &Path) -> anyhow::Result<Vec<(String, String)>> {
    let styles_dir = root.join("styles");
    let assets_dir = out.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    let mut pairs = Vec::new();
    let entries = match std::fs::read_dir(&styles_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(pairs),
        Err(e) => return Err(e.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("css") {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let hash = &format!("{:x}", Sha256::digest(&bytes))[..8];
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("style");
        let out_filename = format!("{stem}.{hash}.css");
        std::fs::write(assets_dir.join(&out_filename), &bytes)?;
        let source_rel = format!(
            "styles/{}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("")
        );
        let out_rel = format!("assets/{out_filename}");
        pairs.push((source_rel, out_rel));
    }

    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn process_css_hashes_and_copies() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("styles")).unwrap();
        std::fs::write(root.path().join("styles/app.css"), "body { color: red; }").unwrap();
        let out = tempdir().unwrap();
        let pairs = process_css(root.path(), out.path()).unwrap();
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
        let pairs = process_css(root.path(), out.path()).unwrap();
        assert!(pairs.is_empty());
    }
}
