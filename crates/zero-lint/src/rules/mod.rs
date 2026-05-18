//! Lint rules. Each rule is a pure function over [`crate::scan::Decl`] or
//! [`crate::scan::RuleBody`] that returns zero or one [`Diagnostic`].

use crate::scan::{Decl, RuleBody};
use crate::{Diagnostic, LintCtx};
use std::path::Path;

pub mod alignment;
pub mod box_model;
pub mod color;
pub mod layout;
pub mod typography;
pub mod undefined_var;

/// Check `decl` against every declaration-shaped rule and return all
/// diagnostics that fire.
pub fn check_decl(file: &Path, decl: &Decl, ctx: &LintCtx) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    if let Some(d) = typography::check(file, decl) {
        out.push(d);
    }
    if let Some(d) = color::check(file, decl) {
        out.push(d);
    }
    if let Some(d) = box_model::check(file, decl) {
        out.push(d);
    }
    if let Some(d) = alignment::check(file, decl) {
        out.push(d);
    }
    out.extend(undefined_var::check(file, decl, ctx));
    out
}

/// Check `body` against every body-shaped rule and return all diagnostics.
pub fn check_body(file: &Path, body: &RuleBody, _ctx: &LintCtx) -> Vec<Diagnostic> {
    layout::check(file, body)
}

/// Value-level whitelist that suppresses every numeric rule. Matches the
/// CSS-keyword sentinels and any value composed entirely of `var(--…)`
/// references combined with `0` via a calc-shaped expression.
pub fn is_whitelisted_value(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "0" | "0%" | "auto" | "none" | "inherit" | "initial" | "unset" | "currentcolor"
        | "transparent" => return true,
        _ => {}
    }
    if is_var_reference(trimmed) {
        return true;
    }
    if let Some(inner) = strip_calc(trimmed) {
        return calc_inner_is_safe(inner);
    }
    false
}

fn is_var_reference(s: &str) -> bool {
    let s = s.trim();
    if !(s.starts_with("var(") && s.ends_with(')')) {
        return false;
    }
    let inner = &s[4..s.len() - 1];
    inner.trim().starts_with("--")
}

fn strip_calc(s: &str) -> Option<&str> {
    let s = s.trim();
    if (s.starts_with("calc(") || s.starts_with("CALC(")) && s.ends_with(')') {
        Some(&s[5..s.len() - 1])
    } else {
        None
    }
}

fn calc_inner_is_safe(inner: &str) -> bool {
    let mut buf = String::new();
    let mut depth = 0i32;
    let mut tokens: Vec<String> = Vec::new();
    for ch in inner.chars() {
        match ch {
            '(' => {
                depth += 1;
                buf.push(ch);
            }
            ')' => {
                depth -= 1;
                buf.push(ch);
            }
            ' ' | '+' | '-' | '*' | '/' if depth == 0 => {
                if !buf.trim().is_empty() {
                    tokens.push(buf.trim().to_string());
                }
                buf.clear();
            }
            _ => buf.push(ch),
        }
    }
    if !buf.trim().is_empty() {
        tokens.push(buf.trim().to_string());
    }
    if tokens.is_empty() {
        return false;
    }
    tokens
        .iter()
        .all(|t| t == "0" || t == "0%" || is_var_reference(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_accepts_sentinels_and_var() {
        for v in [
            "0",
            "0%",
            "auto",
            "none",
            "inherit",
            "initial",
            "unset",
            "currentColor",
            "transparent",
            "var(--space-md)",
        ] {
            assert!(is_whitelisted_value(v), "expected whitelisted: {v}");
        }
    }

    #[test]
    fn whitelist_accepts_calc_of_var_and_zero() {
        assert!(is_whitelisted_value(
            "calc(var(--space-md) + var(--space-sm))"
        ));
        assert!(is_whitelisted_value("calc(var(--space-md) + 0)"));
    }

    #[test]
    fn whitelist_rejects_calc_with_raw_value() {
        assert!(!is_whitelisted_value("calc(var(--space-md) + 4px)"));
    }

    #[test]
    fn whitelist_rejects_raw_dimension() {
        assert!(!is_whitelisted_value("12px"));
        assert!(!is_whitelisted_value("0.5rem"));
    }
}
