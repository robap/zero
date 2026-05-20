//! JS/TS lint pipeline: file walker, SWC parse via `zero-transpile`, and
//! per-rule visitors. Built on top of `FileCtx`; rules are pure functions
//! over the context.

use std::path::Path;

use crate::Diagnostic;

pub mod context;
pub mod diag;
pub mod parse_error;
pub mod rules;
pub mod walk;

/// Lint one JS/TS file.
///
/// On parse error: returns a single `P01` diagnostic and skips the
/// rules. On success: runs each registered rule against the file's
/// [`context::FileCtx`] and returns all fired diagnostics.
pub fn lint_js_file(file: &Path, source: &str, root: &Path) -> Vec<Diagnostic> {
    let ctx = match parse_error::check(file, source, root) {
        Ok(c) => c,
        Err(d) => return vec![d],
    };
    let mut out: Vec<Diagnostic> = Vec::new();
    out.extend(rules::r01_template_val_read::check(&ctx));
    out.extend(rules::r02_val_assignment::check(&ctx));
    out.extend(rules::r03_module_reactive::check(&ctx));
    out.extend(rules::t01_event_listener::check(&ctx));
    out.extend(rules::t02_event_modifier::check(&ctx));
    out.extend(rules::t03_each_no_key::check(&ctx));
    out.extend(rules::t04_direct_dom::check(&ctx));
    out.extend(rules::c01_no_class_component::check(&ctx));
    out.extend(rules::c02_custom_elements::check(&ctx));
    out.extend(rules::i01_unknown_specifier::check(&ctx));
    out.extend(rules::i02_dot_zero_import::check(&ctx));
    out.extend(rules::s01_function_size::check(&ctx));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn clean_file_emits_no_diagnostics() {
        let file = PathBuf::from("/tmp/src/app.ts");
        let root = PathBuf::from("/tmp");
        let diags = lint_js_file(&file, "export const x = 1;", &root);
        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
    }

    #[test]
    fn parse_error_short_circuits() {
        let file = PathBuf::from("/tmp/src/bad.ts");
        let root = PathBuf::from("/tmp");
        let diags = lint_js_file(&file, "const x: = ;", &root);
        assert_eq!(diags.len(), 1, "expected exactly one diagnostic");
        assert_eq!(diags[0].rule, "P01");
    }
}
