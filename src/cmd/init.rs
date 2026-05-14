//! `zero init` — scaffold a zero app into `./<root>/`.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::prompts::{Answers, prompt_user};
use crate::scaffold::{ScaffoldContext, write_to};
use crate::toml_writer::render_toml;

/// Run the `zero init` subcommand.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub async fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let toml_path = cwd.join("zero.toml");

    let config = if toml_path.exists() {
        Config::load_from_cwd()?
    } else {
        println!("zero init — let's set up a project");
        let answers = prompt_user()?;
        write_toml_file(&toml_path, &render_toml(&answers))?;
        config_from_answers(&answers)?
    };

    let root_dir = cwd.join(&config.project.root);
    if root_dir.exists() && fs::read_dir(&root_dir)?.next().is_some() {
        anyhow::bail!(
            "zero init: ./{}/ is not empty; refusing to overwrite",
            config.project.root
        );
    }

    let title = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "My zero app".to_string());

    write_to(&root_dir, &ScaffoldContext { title })?;

    println!(
        "Scaffold written to ./{}/ — run `zero dev` to start.",
        config.project.root
    );
    Ok(())
}

/// Write `text` to `path`, refusing if the file already exists.
///
/// # Parameters
/// - `path`: target file.
/// - `text`: contents to write.
///
/// # Returns
/// `Ok(())` on success.
fn write_toml_file(path: &Path, text: &str) -> anyhow::Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

/// Build a `Config` directly from the wizard's `Answers` (no round-trip
/// through TOML parsing required to keep the init flow moving).
///
/// # Parameters
/// - `a`: the wizard's collected answers.
///
/// # Returns
/// A validated `Config`.
fn config_from_answers(a: &Answers) -> anyhow::Result<Config> {
    // Re-use the parser so the same validation rules apply.
    Config::from_toml_str(&render_toml(a))
}
