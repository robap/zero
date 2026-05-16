//! `zero build` subcommand entry point.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::build::bundler::bundle;
use crate::build::css::process_css;
use crate::build::index_html::render;
use crate::build::manifest::write as write_manifest;
use crate::config::Config;

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
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root);
    let out_dir = cwd.join(&config.build.out);
    let assets_dir = out_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    let emit_sourcemap = sourcemap_override.unwrap_or(config.build.sourcemap);

    // 1. Bundle JS.
    let bundle_out = bundle(&config, emit_sourcemap)?;
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
        copy_public(&public_dir, &out_dir.join("public"))?
    } else {
        0
    };

    println!(
        "zero build — {} bytes JS, {} CSS file(s), {} public asset(s); output in {}/",
        bundle_src.len(),
        css_pairs.len(),
        public_copied,
        out_dir.display()
    );

    Ok(())
}

/// Recursively copy every file under `src` into `dst`, preserving the
/// relative tree. Returns the total file count copied.
fn copy_public(src: &Path, dst: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(dst)?;
    let mut count = 0usize;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            count += copy_public(&path, &target)?;
        } else if path.is_file() {
            std::fs::copy(&path, &target)?;
            count += 1;
        }
    }
    Ok(count)
}
