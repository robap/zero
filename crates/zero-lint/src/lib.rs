//! Design-system lint for `zero` user SCSS.
//!
//! Library-only crate. The `zero` binary wires `zero lint` to
//! [`lint_project`]; rule modules under `rules/` are pure functions over
//! [`Decl`] / [`RuleBody`] streams produced by [`scan`].

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod rules;
pub mod scan;
pub mod tokens;
pub mod vars;
pub mod walk;

/// Project-wide context shared with every rule. Built once per
/// [`lint_project`] call and passed to each declaration / body check.
#[derive(Debug, Default)]
pub struct LintCtx {
    /// Every `--name` declared anywhere in the compiled SCSS (framework
    /// partials + user files). L13 uses this to validate `var(--name)`
    /// references.
    pub defined_vars: HashSet<String>,
}

/// One diagnostic emitted by a lint rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Rule identifier, e.g. `"L01"` ... `"L11"`.
    pub rule: &'static str,
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub property: String,
    pub value: String,
    /// Replacement text, e.g. `"use var(--weight-semi)"`.
    pub message: String,
}

/// Walk `root`, run every rule, return aggregated diagnostics ordered by
/// `(file, line, column)`.
pub fn lint_project(root: &Path) -> anyhow::Result<Vec<Diagnostic>> {
    let ctx = LintCtx {
        defined_vars: vars::collect_defined_vars(root),
    };
    let mut out: Vec<Diagnostic> = Vec::new();
    for file in walk::user_scss_files(root) {
        let source = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        out.extend(lint_source_with_ctx(&file, &source, &ctx));
    }
    out.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.rule.cmp(b.rule))
    });
    Ok(out)
}

/// Lint a single file's source against an empty context. Exposed for test
/// fixtures that drive the scanner + rules without touching the file
/// system; uses an empty `LintCtx` so var-undefined diagnostics are
/// suppressed (the fixture isn't a full project).
pub fn lint_source(file: &Path, source: &str) -> Vec<Diagnostic> {
    lint_source_with_ctx(file, source, &LintCtx::default())
}

/// Lint a single file's source with the supplied context.
pub fn lint_source_with_ctx(file: &Path, source: &str, ctx: &LintCtx) -> Vec<Diagnostic> {
    let (decls, bodies) = scan::scan(source);
    let mut out: Vec<Diagnostic> = Vec::new();
    for decl in &decls {
        out.extend(rules::check_decl(file, decl, ctx));
    }
    for body in &bodies {
        out.extend(rules::check_body(file, body, ctx));
    }
    out
}
