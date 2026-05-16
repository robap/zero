//! `zero init` — scaffold a zero app into `./<root>/`.

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::prompts::{Answers, prompt_user};
use crate::scaffold::{ScaffoldContext, framework_manifest, write_initial_project};
use crate::toml_writer::render_toml;

/// Run the `zero init` subcommand.
///
/// # Parameters
/// - `yes`: when `true`, skip the pre-flight confirmation prompt.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub async fn run(yes: bool) -> anyhow::Result<()> {
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

    println!("{}", render_init_plan());
    if !yes && !crate::prompts::confirm_default_yes("Proceed?")? {
        println!("zero init: aborted by user");
        return Ok(());
    }

    let title = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "My zero app".to_string());

    write_initial_project(&root_dir, &ScaffoldContext { title })?;

    println!(
        "Scaffold written to ./{}/ — run `zero dev` to start.",
        config.project.root
    );
    Ok(())
}

/// Build the multi-line plan string `zero init` prints before writing
/// any files. Pure (no I/O) so tests can assert against it.
///
/// # Returns
/// The rendered plan, ending with `"Proceed? [Y/n]"`.
fn render_init_plan() -> String {
    let mut out = String::new();
    out.push_str("zero init will create:\n\n");
    out.push_str("  framework files (regenerable, under .zero/)\n");
    for (path, _) in framework_manifest() {
        let _ = writeln!(out, "    {path}");
    }
    out.push_str("\n  user files\n");
    for path in [
        "index.html",
        "tsconfig.json",
        "AGENTS.md",
        ".gitignore",
        "src/app.ts",
        "src/routes/home.ts",
        "src/routes/home.test.ts",
        "styles/app.scss",
    ] {
        let _ = writeln!(out, "    {path}");
    }
    out.push_str("\nProceed? [Y/n]");
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_plan_lists_framework_and_user_groups() {
        let plan = render_init_plan();
        assert!(
            plan.contains("framework files (regenerable, under .zero/)"),
            "missing framework header: {plan}"
        );
        assert!(
            plan.contains(".zero/styles/_tokens.scss"),
            "missing _tokens.scss path: {plan}"
        );
        assert!(
            plan.contains("user files"),
            "missing user files header: {plan}"
        );
        assert!(
            plan.contains("styles/app.scss"),
            "missing styles/app.scss path: {plan}"
        );
        assert!(plan.contains(".gitignore"), "missing .gitignore: {plan}");
        assert!(
            plan.contains("Proceed? [Y/n]"),
            "missing Proceed? prompt: {plan}"
        );
    }
}
