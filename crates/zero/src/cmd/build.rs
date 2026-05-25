//! `zero build` subcommand entry point.

use std::path::Path;

use sha2::{Digest, Sha256};

use zero_bundler::bundler::bundle;
use zero_bundler::css::process_css;
use zero_bundler::index_html::render;
use zero_bundler::manifest::write as write_manifest;
use zero_config::Config;

/// Run the `zero build` subcommand.
///
/// # Parameters
/// - `sourcemap_override`: `Some(true)` / `Some(false)` from the CLI flag,
///   `None` falls back to `[build] sourcemap` in `zero.toml`.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub async fn run(sourcemap_override: Option<bool>) -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    build_inner(&config, sourcemap_override).await
}

/// Build core, callable with a pre-loaded `Config` (lets `zero preview`
/// share the same config snapshot it uses to serve the output).
///
/// # Parameters
/// - `config`: validated `zero.toml` config.
/// - `sourcemap_override`: `Some(true)` / `Some(false)` from the CLI flag,
///   `None` falls back to `[build] sourcemap` in the config.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub(crate) async fn build_inner(
    config: &Config,
    sourcemap_override: Option<bool>,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root);
    let out_dir = config.out_dir_path();
    if let Ok(meta) = std::fs::symlink_metadata(&out_dir) {
        if meta.file_type().is_symlink() {
            anyhow::bail!(
                "build.out `{}` is a symlink; refuse to delete through it. \
                 Remove the symlink and run `zero build` again.",
                out_dir.display()
            );
        }
        std::fs::remove_dir_all(&out_dir)?;
    }
    let assets_dir = out_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    let emit_sourcemap = sourcemap_override.unwrap_or(config.build.sourcemap);

    // 1. Bundle JS.
    let bundle_out = bundle(config, emit_sourcemap)?;
    let bundle_src = bundle_out.code;
    let hash = &format!("{:x}", Sha256::digest(bundle_src.as_bytes()))[..8];
    let bundle_filename = format!("app.{hash}.js");
    let bundle_path = assets_dir.join(&bundle_filename);
    let final_bundle = if let Some(ref map) = bundle_out.source_map {
        let map_filename = format!("{bundle_filename}.map");
        let map_path = assets_dir.join(&map_filename);
        std::fs::write(&map_path, map)?;
        let mut s = bundle_src.clone();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(&format!("//# sourceMappingURL={map_filename}\n"));
        s
    } else {
        bundle_src.clone()
    };
    std::fs::write(&bundle_path, &final_bundle)?;

    // 2. Hash + copy CSS.
    let css_pairs = process_css(&root, &out_dir, emit_sourcemap)?;

    // 3. Build manifest entries.
    let mut manifest_entries: Vec<(String, String)> = Vec::new();
    manifest_entries.push(("app.js".to_string(), format!("assets/{bundle_filename}")));
    for pair in &css_pairs {
        manifest_entries.push(pair.clone());
    }
    write_manifest(&out_dir, &manifest_entries)?;

    // 4. Render static index.html.
    render(&root, &out_dir, &manifest_entries)?;

    // 5. Copy static `public/` assets, if any, mirroring the dev server's
    //    `/public/*` route so URLs resolve identically across dev and prod.
    let public_dir = root.join("public");
    let public_copied = if public_dir.is_dir() {
        copy_tree(&public_dir, &out_dir.join("public"))?
    } else {
        0
    };

    let fonts_src = root.join(".zero").join("fonts");
    let fonts_copied = if fonts_src.is_dir() {
        copy_tree(&fonts_src, &out_dir.join(".zero").join("fonts"))?
    } else {
        0
    };

    println!(
        "zero build — {} bytes JS, {} CSS file(s), {} public asset(s), {} font asset(s); output in {}/",
        bundle_src.len(),
        css_pairs.len(),
        public_copied,
        fonts_copied,
        out_dir.display()
    );

    Ok(())
}

/// Recursively copy every file under `src` into `dst`, preserving the
/// relative tree. Returns the total file count copied.
fn copy_tree(src: &Path, dst: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(dst)?;
    let mut count = 0usize;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            count += copy_tree(&path, &target)?;
        } else if path.is_file() {
            std::fs::copy(&path, &target)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::CWD_LOCK;

    struct CwdGuard {
        prev: std::path::PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    impl CwdGuard {
        fn enter(target: &Path) -> Self {
            let lock = CWD_LOCK.lock().unwrap();
            let prev = std::env::current_dir().unwrap();
            std::env::set_current_dir(target).unwrap();
            CwdGuard { prev, _lock: lock }
        }
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.prev);
        }
    }

    /// Minimal scaffold of a project the bundler can build.
    fn write_minimal_project(root: &Path) {
        std::fs::write(
            root.join("zero.toml"),
            "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
        )
        .unwrap();
        let web = root.join("web");
        std::fs::create_dir_all(web.join("src")).unwrap();
        std::fs::write(
            web.join("index.html"),
            "<!doctype html><html><head><title>x</title></head><body></body></html>",
        )
        .unwrap();
        std::fs::write(
            web.join("src").join("app.ts"),
            "export const x = 1;\nconsole.log(x);\n",
        )
        .unwrap();
    }

    #[tokio::test]
    async fn missing_zero_toml_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        let err = super::run(None).await.expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains("zero.toml"), "msg: {msg}");
    }

    #[tokio::test]
    async fn override_sourcemap_true_writes_external_map_file() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        super::run(Some(true)).await.unwrap();
        let assets = tmp.path().join("dist").join("assets");
        let mut had_map = false;
        for e in std::fs::read_dir(&assets).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|s| s.to_str()) == Some("map") {
                had_map = true;
            }
        }
        assert!(had_map, "expected a .map file in {}", assets.display());
    }

    #[tokio::test]
    async fn override_sourcemap_false_omits_external_map_file() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        super::run(Some(false)).await.unwrap();
        let assets = tmp.path().join("dist").join("assets");
        let mut had_map = false;
        for e in std::fs::read_dir(&assets).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|s| s.to_str()) == Some("map") {
                had_map = true;
            }
        }
        assert!(!had_map, "did not expect a .map file");
    }

    #[tokio::test]
    async fn build_writes_manifest_and_index_html() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        super::run(None).await.unwrap();
        let dist = tmp.path().join("dist");
        assert!(dist.join("manifest.json").is_file(), "manifest missing");
        assert!(dist.join("index.html").is_file(), "index.html missing");
    }

    #[tokio::test]
    async fn override_none_falls_back_to_config_default() {
        // Default build.sourcemap is `false`, so no .map should be written.
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        super::run(None).await.unwrap();
        let assets = tmp.path().join("dist").join("assets");
        let mut had_map = false;
        for e in std::fs::read_dir(&assets).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|s| s.to_str()) == Some("map") {
                had_map = true;
            }
        }
        assert!(!had_map, "did not expect a .map file with default config");
    }

    #[test]
    fn copy_tree_recurses_and_counts_files() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src_pub");
        let dst = tmp.path().join("dst_pub");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        std::fs::write(src.join("nested").join("b.txt"), "b").unwrap();
        let n = copy_tree(&src, &dst).unwrap();
        assert_eq!(n, 2);
        assert!(dst.join("a.txt").is_file());
        assert!(dst.join("nested").join("b.txt").is_file());
    }

    #[tokio::test]
    async fn build_copies_dot_zero_fonts_into_dist() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        let fonts_dir = tmp.path().join("web").join(".zero").join("fonts");
        std::fs::create_dir_all(&fonts_dir).unwrap();
        std::fs::write(fonts_dir.join("Geist-VariableFont_wght.woff2"), b"stub").unwrap();
        super::run(None).await.unwrap();
        let out = tmp.path().join("dist").join(".zero").join("fonts");
        assert!(
            out.join("Geist-VariableFont_wght.woff2").is_file(),
            "font not copied to dist/.zero/fonts/"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn errors_when_out_dir_is_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        let real = tmp.path().join("other_dist");
        std::fs::create_dir_all(&real).unwrap();
        let link = tmp.path().join("dist");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = super::run(None).await.expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains("symlink"), "msg: {msg}");
    }

    #[tokio::test]
    async fn clears_stale_sourcemap_on_disabled_rebuild() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        super::run(Some(true)).await.unwrap();
        super::run(Some(false)).await.unwrap();
        let assets = tmp.path().join("dist").join("assets");
        for e in std::fs::read_dir(&assets).unwrap() {
            let p = e.unwrap().path();
            assert_ne!(
                p.extension().and_then(|s| s.to_str()),
                Some("map"),
                "stale .map survived rebuild: {}",
                p.display()
            );
        }
    }

    #[tokio::test]
    async fn clears_out_dir_before_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        let assets = tmp.path().join("dist").join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("junk.txt"), b"stale").unwrap();
        super::run(None).await.unwrap();
        assert!(
            !assets.join("junk.txt").exists(),
            "stale file should be removed"
        );
        let dist = tmp.path().join("dist");
        assert!(dist.join("index.html").is_file());
        assert!(dist.join("manifest.json").is_file());
    }
}
