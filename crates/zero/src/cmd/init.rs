//! `zero init` — scaffold a zero app into `./<root>/`.

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;

use zero_config::Config;
use zero_config::toml_writer::{TomlInput, render_toml};
use zero_scaffold::{ScaffoldContext, framework_manifest, write_initial_project};

use crate::prompts::{Answers, prompt_user};

/// Convert wizard answers into the plain-data input `render_toml` consumes.
fn toml_input_from_answers(a: &Answers) -> TomlInput {
    TomlInput {
        root: a.root.clone(),
        port: a.port,
        proxy: a.proxy.clone(),
        out: a.out.clone(),
    }
}

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
        // `--yes` must never touch the terminal: take the wizard's
        // defaults so scripted/CI scaffolding works in non-interactive
        // shells (dialoguer errors with "not a terminal" otherwise).
        let answers = if yes {
            Answers::defaults()
        } else {
            println!("zero init — let's set up a project");
            prompt_user()?
        };
        let input = toml_input_from_answers(&answers);
        write_toml_file(&toml_path, &render_toml(&input))?;
        config_from_answers(&answers)?
    };

    let root_dir = cwd.join(&config.project.root);
    if root_dir.exists() && fs::read_dir(&root_dir)?.next().is_some() {
        anyhow::bail!(
            "zero init: ./{}/ is not empty; refusing to overwrite",
            config.project.root
        );
    }

    println!("{}", render_init_plan(!yes));
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
/// # Parameters
/// - `with_prompt`: append the trailing `"Proceed? [Y/n]"` line. `false`
///   under `--yes`, where no prompt follows the plan.
///
/// # Returns
/// The rendered plan.
fn render_init_plan(with_prompt: bool) -> String {
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
        ".gitignore",
        "src/app.ts",
        "src/routes/home.ts",
        "src/routes/home.test.ts",
        "styles/app.scss",
    ] {
        let _ = writeln!(out, "    {path}");
    }
    if with_prompt {
        out.push_str("\nProceed? [Y/n]");
    }
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
    Config::from_toml_str(&render_toml(&toml_input_from_answers(a)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_answers() -> Answers {
        Answers {
            root: "web".to_string(),
            port: 1234,
            proxy: Some("http://127.0.0.1:8080".to_string()),
            out: "dist".to_string(),
        }
    }

    #[test]
    fn init_plan_lists_framework_and_user_groups() {
        let plan = render_init_plan(true);
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

    #[test]
    fn init_plan_omits_prompt_when_not_interactive() {
        let plan = render_init_plan(false);
        assert!(
            !plan.contains("Proceed?"),
            "plan must not dangle a prompt under --yes: {plan}"
        );
        assert!(
            plan.contains("user files"),
            "plan body must still render: {plan}"
        );
    }

    #[test]
    fn answers_defaults_match_wizard_defaults() {
        let a = crate::prompts::Answers::defaults();
        assert_eq!(a.root, "web");
        assert_eq!(a.port, 3000);
        assert_eq!(a.proxy, None);
        assert_eq!(a.out, "dist");
        // Defaults must round-trip through the same validation as the wizard.
        config_from_answers(&a).expect("default answers produce a valid config");
    }

    #[test]
    fn init_plan_lists_agents_md_under_framework_files() {
        let plan = render_init_plan(true);
        let framework_idx = plan
            .find("framework files (regenerable, under .zero/)")
            .expect("framework header present");
        let user_idx = plan.find("user files").expect("user header present");
        let agents_idx = plan.find("AGENTS.md").expect("AGENTS.md present in plan");
        assert!(
            agents_idx > framework_idx && agents_idx < user_idx,
            "AGENTS.md must be under framework files, not user files: {plan}"
        );
        assert_eq!(
            plan.matches("AGENTS.md").count(),
            1,
            "AGENTS.md should appear exactly once: {plan}"
        );
    }

    #[test]
    fn toml_input_from_answers_copies_fields() {
        let a = sample_answers();
        let input = toml_input_from_answers(&a);
        assert_eq!(input.root, "web");
        assert_eq!(input.port, 1234);
        assert_eq!(input.proxy.as_deref(), Some("http://127.0.0.1:8080"));
        assert_eq!(input.out, "dist");
    }

    #[test]
    fn config_from_answers_round_trips_through_toml() {
        let a = sample_answers();
        let cfg = config_from_answers(&a).expect("config_from_answers");
        assert_eq!(cfg.project.root, "web");
        assert_eq!(cfg.dev.port, 1234);
        assert_eq!(cfg.build.out, "dist");
    }

    #[test]
    fn write_toml_file_creates_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("zero.toml");
        write_toml_file(&path, "[project]\nroot = \"web\"\n").expect("first write");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("[project]"));
    }

    #[test]
    fn write_toml_file_refuses_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("zero.toml");
        std::fs::write(&path, "existing").unwrap();
        let err = write_toml_file(&path, "new").expect_err("should refuse overwrite");
        assert!(err.to_string().contains("failed to write"));
        // Original content is preserved.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing");
    }
}
