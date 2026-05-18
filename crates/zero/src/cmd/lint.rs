//! `zero lint` subcommand: design-system lint over user SCSS / CSS.
//!
//! Walks the project root via `zero-lint`, prints diagnostics in
//! `zero test`-shaped output (path + caret snippet by default;
//! single-line per diagnostic with `--quiet`), and exits non-zero if any
//! fired.

use std::io::Write;
use zero_config::Config;
use zero_lint::{Diagnostic, lint_project};

/// Run `zero lint [--quiet]`.
///
/// # Parameters
/// - `quiet`: when true, suppress the source-snippet line and caret.
///
/// # Returns
/// `Ok(())` if no diagnostics fire. Exits the process with code 1 if any
/// diagnostic is emitted.
pub async fn run(quiet: bool) -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    let root = config.project_root_path();
    let diags = lint_project(&root)?;

    let stderr = std::io::stderr();
    let mut out = stderr.lock();
    for d in &diags {
        write_diag(&mut out, &root, d, quiet)?;
    }
    if !diags.is_empty() {
        writeln!(
            out,
            "\nzero lint — {} diagnostic{}",
            diags.len(),
            if diags.len() == 1 { "" } else { "s" }
        )?;
        std::process::exit(1);
    }
    println!("zero lint — clean");
    Ok(())
}

fn write_diag(
    w: &mut impl Write,
    root: &std::path::Path,
    d: &Diagnostic,
    quiet: bool,
) -> anyhow::Result<()> {
    let rel = d
        .file
        .strip_prefix(root)
        .unwrap_or(&d.file)
        .display()
        .to_string();
    writeln!(
        w,
        "{rel}:{}:{}  {}  {}: {} — {}",
        d.line, d.column, d.rule, d.property, d.value, d.message
    )?;
    if !quiet
        && let Ok(source) = std::fs::read_to_string(&d.file)
        && let Some(line) = source.lines().nth(d.line.saturating_sub(1) as usize)
    {
        writeln!(w, "      {line}")?;
        let mut caret = String::from("      ");
        for _ in 1..d.column {
            caret.push(' ');
        }
        caret.push('^');
        writeln!(w, "{caret}")?;
    }
    Ok(())
}
