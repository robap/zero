//! `zero build` subcommand entry point.

use sha2::{Digest, Sha256};

use crate::build::bundler::bundle;
use crate::build::css::process_css;
use crate::build::index_html::render;
use crate::build::manifest::write as write_manifest;
use crate::config::Config;

/// Run the `zero build` subcommand.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub async fn run() -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root);
    let out_dir = cwd.join(&config.build.out);
    let assets_dir = out_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    // 1. Bundle JS.
    let bundle_src = bundle(&config)?;
    let hash = &format!("{:x}", Sha256::digest(bundle_src.as_bytes()))[..8];
    let bundle_filename = format!("app.{hash}.js");
    let bundle_path = assets_dir.join(&bundle_filename);
    std::fs::write(&bundle_path, &bundle_src)?;

    // 2. Hash + copy CSS.
    let css_pairs = process_css(&root, &out_dir)?;

    // 3. Build manifest entries.
    let mut manifest_entries: Vec<(String, String)> = Vec::new();
    manifest_entries.push(("app.js".to_string(), format!("assets/{bundle_filename}")));
    for pair in &css_pairs {
        manifest_entries.push(pair.clone());
    }
    write_manifest(&out_dir, &manifest_entries)?;

    // 4. Render static index.html.
    render(&root, &out_dir, &manifest_entries)?;

    println!(
        "zero build — {} bytes JS, {} CSS file(s); output in {}/",
        bundle_src.len(),
        css_pairs.len(),
        out_dir.display()
    );

    Ok(())
}
