//! L11 — layout-primitive detection. The one rule that operates on a
//! whole rule body rather than a single declaration. Conservative by
//! design: every required declaration must be present before a pattern
//! fires, and selectors that already reference the primitive class are
//! skipped (override / extension sites).

use crate::Diagnostic;
use crate::scan::{Decl, RuleBody};
use std::path::Path;

pub fn check(file: &Path, body: &RuleBody) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let selector = body.selector.to_ascii_lowercase();
    if selector_contains_primitive(&selector) {
        return out;
    }
    if let Some(d) = match_pattern(file, body, &selector) {
        out.push(d);
    }
    out
}

fn selector_contains_primitive(sel: &str) -> bool {
    [".cluster", ".stack", ".split", ".flank", ".grid", ".frame"]
        .iter()
        .any(|p| sel.contains(p))
}

fn match_pattern(file: &Path, body: &RuleBody, _selector: &str) -> Option<Diagnostic> {
    // First, try each pattern. Most-specific patterns first so a body
    // that matches both `stack` and the broader `cluster` shape gets the
    // more-specific suggestion.
    if let Some(name) = match_frame(body) {
        return Some(diag(file, body, name));
    }
    if let Some(name) = match_grid(body) {
        return Some(diag(file, body, name));
    }
    if let Some(name) = match_stack(body) {
        return Some(diag(file, body, name));
    }
    if let Some(name) = match_cluster(body) {
        return Some(diag(file, body, name));
    }
    if let Some(name) = match_split(body) {
        return Some(diag(file, body, name));
    }
    if let Some(name) = match_flank(body) {
        return Some(diag(file, body, name));
    }
    None
}

fn diag(file: &Path, body: &RuleBody, primitive: &'static str) -> Diagnostic {
    let line = body.decls.first().map(|d| d.line).unwrap_or(1);
    let column = body.decls.first().map(|d| d.column).unwrap_or(1);
    Diagnostic {
        rule: "L11",
        file: file.to_path_buf(),
        line,
        column,
        property: "(rule body)".to_string(),
        value: body.selector.clone(),
        message: format!(
            "this body matches the .{primitive} primitive — see \"When to reach for which primitive\" in AGENTS.md"
        ),
    }
}

fn find_decl<'a>(body: &'a RuleBody, property: &str) -> Option<&'a Decl> {
    body.decls.iter().find(|d| d.property == property)
}

fn decl_value_eq(body: &RuleBody, property: &str, expected: &str) -> bool {
    find_decl(body, property)
        .map(|d| d.value.trim().eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

/// True when `display` is one of the accepted flex-shaped values
/// (`flex` or `inline-flex`). The two layout primitives don't ship an
/// inline variant, but the body shapes are semantically identical and
/// agents reach for `inline-flex` for small chips / buttons.
fn has_flex_display(body: &RuleBody) -> bool {
    find_decl(body, "display")
        .map(|d| {
            let v = d.value.trim().to_ascii_lowercase();
            v == "flex" || v == "inline-flex"
        })
        .unwrap_or(false)
}

fn has_grid_display(body: &RuleBody) -> bool {
    find_decl(body, "display")
        .map(|d| {
            let v = d.value.trim().to_ascii_lowercase();
            v == "grid" || v == "inline-grid"
        })
        .unwrap_or(false)
}

fn match_cluster(body: &RuleBody) -> Option<&'static str> {
    if !has_flex_display(body) {
        return None;
    }
    if !decl_value_eq(body, "flex-wrap", "wrap") {
        return None;
    }
    Some("cluster")
}

fn match_stack(body: &RuleBody) -> Option<&'static str> {
    if !has_flex_display(body) {
        return None;
    }
    if !decl_value_eq(body, "flex-direction", "column") {
        return None;
    }
    Some("stack")
}

fn match_split(body: &RuleBody) -> Option<&'static str> {
    if !has_flex_display(body) {
        return None;
    }
    if !decl_value_eq(body, "justify-content", "space-between") {
        return None;
    }
    Some("split")
}

fn match_flank(body: &RuleBody) -> Option<&'static str> {
    if !has_flex_display(body) {
        return None;
    }
    // A child entry must include `flex: 0 0 auto` for the conservative
    // flank pattern. (Step 11 audit decides whether to keep this — kept
    // here so the fixture in this step locks the behaviour.)
    let has_flank_child = body.child_decls.values().any(|decls| {
        decls
            .iter()
            .any(|d| d.property == "flex" && d.value.split_whitespace().eq(["0", "0", "auto"]))
    });
    if !has_flank_child {
        return None;
    }
    Some("flank")
}

fn match_grid(body: &RuleBody) -> Option<&'static str> {
    if !has_grid_display(body) {
        return None;
    }
    let tmpl = find_decl(body, "grid-template-columns")?;
    let v = tmpl.value.to_ascii_lowercase();
    if v.contains("repeat(") && v.contains("auto-fit") && v.contains("minmax") {
        Some("grid")
    } else {
        None
    }
}

fn match_frame(body: &RuleBody) -> Option<&'static str> {
    let _ = find_decl(body, "aspect-ratio")?;
    if !decl_value_eq(body, "overflow", "hidden") {
        return None;
    }
    Some("frame")
}
