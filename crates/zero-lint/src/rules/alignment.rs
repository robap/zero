//! L12 — alignment-property literals.
//!
//! Flags `align-items` / `justify-content` / `align-self` / `justify-self`
//! / `text-align` when the value maps to one of the alignment utility
//! classes shipped in `_alignment.scss`. The framework's stance is that
//! these properties belong in the class list (`class="cluster align-center"`)
//! rather than written inline; the rule says so out loud.
//!
//! Selectors that already reference an alignment utility class (`.align-*`,
//! `.justify-*`, `.text-*`, `.flex-*`) are skipped — those are override
//! sites where re-declaring the property is the point.

use super::is_whitelisted_value;
use crate::Diagnostic;
use crate::scan::Decl;
use std::path::Path;

pub fn check(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    if is_whitelisted_value(&decl.value) {
        return None;
    }
    if selector_path_references_utility(&decl.selector_path) {
        return None;
    }
    let utility = utility_for(&decl.property, decl.value.trim())?;
    Some(Diagnostic {
        rule: "L12",
        file: file.to_path_buf(),
        line: decl.line,
        column: decl.column,
        property: decl.property.clone(),
        value: decl.value.clone(),
        message: format!("use the .{utility} utility class instead of writing this inline"),
    })
}

fn selector_path_references_utility(path: &[String]) -> bool {
    let joined = path.join(" ").to_ascii_lowercase();
    [
        ".align-",
        ".justify-",
        ".text-start",
        ".text-center",
        ".text-end",
        ".flex-row",
        ".flex-col",
    ]
    .iter()
    .any(|needle| joined.contains(needle))
}

/// Map (`property`, normalized value) → utility class. Returns `None`
/// when the value doesn't correspond to a shipped utility.
fn utility_for(property: &str, value: &str) -> Option<&'static str> {
    let v = value.to_ascii_lowercase();
    match property {
        "align-items" => align_to_utility(&v).map(|s| match s {
            "start" => "align-start",
            "center" => "align-center",
            "end" => "align-end",
            "stretch" => "align-stretch",
            "baseline" => "align-baseline",
            _ => unreachable!(),
        }),
        "align-self" => align_to_utility(&v).map(|s| match s {
            "start" => "align-self-start",
            "center" => "align-self-center",
            "end" => "align-self-end",
            "stretch" => "align-self-stretch",
            "baseline" => "align-self-baseline",
            _ => unreachable!(),
        }),
        "justify-content" => match v.as_str() {
            "start" | "flex-start" => Some("justify-start"),
            "center" => Some("justify-center"),
            "end" | "flex-end" => Some("justify-end"),
            "space-between" => Some("justify-between"),
            "space-around" => Some("justify-around"),
            "space-evenly" => Some("justify-evenly"),
            _ => None,
        },
        "justify-self" => match v.as_str() {
            "start" | "flex-start" => Some("justify-self-start"),
            "center" => Some("justify-self-center"),
            "end" | "flex-end" => Some("justify-self-end"),
            "stretch" => Some("justify-self-stretch"),
            _ => None,
        },
        "text-align" => match v.as_str() {
            "start" | "left" => Some("text-start"),
            "center" => Some("text-center"),
            "end" | "right" => Some("text-end"),
            _ => None,
        },
        "flex-direction" => match v.as_str() {
            "row" => Some("flex-row"),
            "row-reverse" => Some("flex-row-reverse"),
            "column" => Some("flex-col"),
            "column-reverse" => Some("flex-col-reverse"),
            _ => None,
        },
        _ => None,
    }
}

/// Normalize `align-items` / `align-self` values to the bare keyword the
/// utility names use.
fn align_to_utility(value: &str) -> Option<&'static str> {
    match value {
        "start" | "flex-start" => Some("start"),
        "center" => Some("center"),
        "end" | "flex-end" => Some("end"),
        "stretch" => Some("stretch"),
        "baseline" => Some("baseline"),
        _ => None,
    }
}
