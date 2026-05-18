//! L13 integration: build a tempdir with a `.zero/styles/_tokens.scss`
//! declaring a small set of vars, then lint a user file that references
//! both a known and an unknown one.

use std::collections::HashSet;
use std::path::PathBuf;
use zero_lint::{LintCtx, lint_source_with_ctx};

fn ctx_with(names: &[&str]) -> LintCtx {
    LintCtx {
        defined_vars: names.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
    }
}

#[test]
fn flags_unknown_var_reference() {
    let ctx = ctx_with(&["--space-md", "--color-bg"]);
    let path = PathBuf::from("test.scss");
    let src = ".x { padding: var(--space-md); border-radius: var(--radius-pill); }";
    let diags = lint_source_with_ctx(&path, src, &ctx);
    let l13: Vec<_> = diags.iter().filter(|d| d.rule == "L13").collect();
    assert_eq!(l13.len(), 1, "diags = {diags:#?}");
    assert!(
        l13[0].message.contains("--radius-pill"),
        "msg = {}",
        l13[0].message
    );
}

#[test]
fn defined_var_is_silent() {
    let ctx = ctx_with(&["--space-md"]);
    let path = PathBuf::from("test.scss");
    let diags = lint_source_with_ctx(&path, ".x { padding: var(--space-md); }", &ctx);
    let l13: Vec<_> = diags.iter().filter(|d| d.rule == "L13").collect();
    assert_eq!(l13.len(), 0);
}

#[test]
fn empty_context_skips_rule() {
    // Without a project context the rule has no ground truth and stays silent.
    let path = PathBuf::from("test.scss");
    let diags = lint_source_with_ctx(
        &path,
        ".x { color: var(--definitely-not-a-token); }",
        &LintCtx::default(),
    );
    let l13: Vec<_> = diags.iter().filter(|d| d.rule == "L13").collect();
    assert_eq!(l13.len(), 0);
}

#[test]
fn project_level_collect_picks_up_zero_styles() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join(".zero/styles")).unwrap();
    std::fs::create_dir_all(root.join("styles")).unwrap();
    std::fs::write(
        root.join(".zero/styles/_tokens.scss"),
        ":root { --space-md: 1rem; --color-bg: #fff; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("styles/app.scss"),
        ".x { padding: var(--space-md); border-radius: var(--radius-pill); }\n",
    )
    .unwrap();

    let diags = zero_lint::lint_project(root).unwrap();
    let l13: Vec<_> = diags.iter().filter(|d| d.rule == "L13").collect();
    assert_eq!(l13.len(), 1, "diags = {diags:#?}");
    assert!(l13[0].message.contains("--radius-pill"));
}
