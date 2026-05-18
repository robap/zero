//! L06–L10 — radius, border, padding, margin, gap.
//!
//! All five rules share the "first dimension in the value" shape: scan
//! the value for the first dimension token, resolve it against the
//! appropriate scale, name both the token and (where one exists) a
//! utility class.

use super::is_whitelisted_value;
use crate::Diagnostic;
use crate::scan::Decl;
use crate::tokens::{
    BORDER, RADIUS, SPACE, Scale, ScaleKind, Unit, nearest, parse_dimension, to_base,
};
use std::path::Path;

const PADDING_PROPS: &[&str] = &[
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "padding-block",
    "padding-inline",
    "padding-block-start",
    "padding-block-end",
    "padding-inline-start",
    "padding-inline-end",
];

const MARGIN_PROPS: &[&str] = &[
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "margin-block",
    "margin-inline",
    "margin-block-start",
    "margin-block-end",
    "margin-inline-start",
    "margin-inline-end",
];

const GAP_PROPS: &[&str] = &["gap", "row-gap", "column-gap"];

const BORDER_SHORTHAND_PROPS: &[&str] = &[
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
];

const BORDER_WIDTH_PROPS: &[&str] = &[
    "border-width",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
];

pub fn check(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    if is_whitelisted_value(&decl.value) {
        return None;
    }
    let prop = decl.property.as_str();
    if prop == "border-radius" {
        return check_radius(file, decl);
    }
    if BORDER_WIDTH_PROPS.contains(&prop) || BORDER_SHORTHAND_PROPS.contains(&prop) {
        return check_border(file, decl);
    }
    if PADDING_PROPS.contains(&prop) {
        return check_space(file, decl, "L08", SPACE, Some("pad-"));
    }
    if MARGIN_PROPS.contains(&prop) {
        return check_space(file, decl, "L09", SPACE, None);
    }
    if GAP_PROPS.contains(&prop) {
        return check_space(file, decl, "L10", SPACE, Some("gap-"));
    }
    None
}

fn check_radius(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    let v = decl.value.trim();
    // 50% or numeric ≥ 999px → suggest --radius-3xl directly.
    if v == "50%" {
        return Some(make(file, decl, "L06", "use var(--radius-3xl)".to_string()));
    }
    let first = v.split_whitespace().next()?;
    if first.starts_with("var(") {
        return None;
    }
    let (val, unit) = parse_dimension(first)?;
    if matches!(unit, Unit::Percent) {
        // Non-50 percent: don't fire.
        return None;
    }
    let base = to_base(val, unit, ScaleKind::Length);
    if base >= 999.0 {
        return Some(make(file, decl, "L06", "use var(--radius-3xl)".to_string()));
    }
    let r = nearest(RADIUS, base);
    let mut msg = format!("use var({})", r.token);
    if r.outside_scale {
        msg.push_str(" — outside the scale");
    }
    Some(make(file, decl, "L06", msg))
}

fn check_border(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    let first = first_dim(&decl.value)?;
    let (val, unit) = parse_dimension(&first)?;
    let base = to_base(val, unit, ScaleKind::Length);
    let r = nearest(BORDER, base);
    let msg = if BORDER_WIDTH_PROPS.contains(&decl.property.as_str()) {
        format!("use var({})", r.token)
    } else {
        format!(
            "use var({}) or the .border / .border-{{t,r,b,l}} utility instead of writing the shorthand inline",
            r.token
        )
    };
    Some(make(file, decl, "L07", msg))
}

fn check_space(
    file: &Path,
    decl: &Decl,
    rule: &'static str,
    scale: Scale,
    utility_prefix: Option<&'static str>,
) -> Option<Diagnostic> {
    let tokens: Vec<&str> = decl.value.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    // If every top-level token is a var or sentinel, no fire.
    if tokens
        .iter()
        .all(|t| t.starts_with("var(") || is_whitelisted_value(t))
    {
        return None;
    }
    // Search across the whole value (including inside `calc(...)`) for
    // the first raw dimension that isn't a percent. Tokens are stripped
    // of arithmetic / paren punctuation before being parsed.
    let mut suggestion: Option<&'static str> = None;
    for raw in scan_dimension_tokens(&decl.value) {
        if let Some((v, u)) = parse_dimension(&raw) {
            if matches!(u, Unit::Percent) {
                continue;
            }
            let base = to_base(v, u, ScaleKind::Length);
            suggestion = Some(nearest(scale, base).token);
            break;
        }
    }
    let token = suggestion?;
    let msg = if tokens.len() == 1 && !decl.value.contains("calc(") {
        match utility_prefix {
            Some(prefix) => format!(
                "use var({token}) or the {prefix}{step} utility",
                step = utility_step(token)
            ),
            None => format!("use var({token})"),
        }
    } else if decl.value.contains("calc(") {
        format!("use var({token}) instead of mixing tokens with raw values in calc()")
    } else {
        format!("use var({token}) (multi-value shorthand — name a token per axis)")
    };
    Some(make(file, decl, rule, msg))
}

/// Yield candidate dimension tokens from a CSS value. Splits on whitespace
/// and any non-alphanumeric / non-`%` / non-`.` / non-`-` character, so
/// `calc(var(--x) + 4px)` yields `var`, `--x`, `4px`.
fn scan_dimension_tokens(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '(' => {
                // If the previous buffer was `var`, we're entering a
                // var(...) and want to skip its body wholesale.
                if buf == "var" {
                    let mut depth = 1i32;
                    buf.clear();
                    for c in chars.by_ref() {
                        if c == '(' {
                            depth += 1;
                        } else if c == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                    continue;
                }
                if !buf.is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
            }
            ')' | ',' | '+' | '*' | '/' => {
                if !buf.is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
            }
            c if c.is_whitespace() => {
                if !buf.is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
            }
            c => buf.push(c),
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

fn utility_step(token: &str) -> &'static str {
    match token {
        "--space-xs" => "xs",
        "--space-sm" => "sm",
        "--space-md" => "md",
        "--space-lg" => "lg",
        "--space-xl" => "xl",
        _ => "md",
    }
}

fn first_dim(value: &str) -> Option<String> {
    for t in value.split_whitespace() {
        if t.starts_with("var(") {
            continue;
        }
        if parse_dimension(t).is_some() {
            return Some(t.to_string());
        }
    }
    None
}

fn make(file: &Path, decl: &Decl, rule: &'static str, message: String) -> Diagnostic {
    Diagnostic {
        rule,
        file: file.to_path_buf(),
        line: decl.line,
        column: decl.column,
        property: decl.property.clone(),
        value: decl.value.clone(),
        message,
    }
}
