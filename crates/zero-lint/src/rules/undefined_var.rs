//! L13 — `var(--name)` references whose `--name` isn't declared anywhere
//! in the project.
//!
//! Catches renames (`var(--radius-pill)` after a token was renamed to
//! `--radius-3xl`), utility-class confusions (`var(--pad-sm)` — `pad-sm`
//! is a class, not a custom property), and plain typos. The defined-name
//! set is built once per `lint_project` call by [`crate::vars`].
//!
//! Skipped when the context is empty — that means the caller is running
//! the rule against a single source file without a project on disk
//! (most likely a unit-test fixture), and we have no ground truth.

use crate::scan::Decl;
use crate::vars::extract_var_refs;
use crate::{Diagnostic, LintCtx};
use std::path::Path;

pub fn check(file: &Path, decl: &Decl, ctx: &LintCtx) -> Vec<Diagnostic> {
    if ctx.defined_vars.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Diagnostic> = Vec::new();
    for name in extract_var_refs(&decl.value) {
        if ctx.defined_vars.contains(&name) {
            continue;
        }
        out.push(Diagnostic {
            rule: "L13",
            file: file.to_path_buf(),
            line: decl.line,
            column: decl.column,
            property: decl.property.clone(),
            value: decl.value.clone(),
            message: format!("var({name}) — no such custom property declared in this project"),
        });
    }
    out
}
