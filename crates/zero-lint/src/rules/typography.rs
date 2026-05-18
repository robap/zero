//! Typography rules L01–L04.
//!
//! - L01: `font-weight` numeric or keyword (excluding `normal`).
//! - L02: `font-size` raw dimension.
//! - L03: `line-height` raw numeric.
//! - L04: `letter-spacing` raw dimension.

use super::is_whitelisted_value;
use crate::Diagnostic;
use crate::scan::Decl;
use crate::tokens::{
    self, FONT_SIZE, LEADING, ScaleKind, TRACKING, Unit, WEIGHT, nearest, nearest_keyword,
    parse_dimension, to_base,
};
use std::path::Path;

/// Run every typography rule that applies to `decl`. Returns the first
/// diagnostic; only one rule per declaration ever applies (the rule key is
/// the property name).
pub fn check(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    if is_whitelisted_value(&decl.value) {
        return None;
    }
    match decl.property.as_str() {
        "font-weight" => check_weight(file, decl),
        "font-size" => check_size(file, decl),
        "line-height" => check_leading(file, decl),
        "letter-spacing" => check_tracking(file, decl),
        _ => None,
    }
}

fn check_weight(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    let first_token = decl.value.split_whitespace().next()?;
    // Skip if the first token is itself a var(...) reference.
    if first_token.starts_with("var(") {
        return None;
    }
    let suggested = if let Ok(n) = first_token.parse::<f64>() {
        nearest(WEIGHT, n).token
    } else {
        nearest_keyword(WEIGHT, first_token)?
    };
    Some(diagnostic(
        "L01",
        file,
        decl,
        format!("use var({suggested})"),
    ))
}

fn check_size(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    let first_token = decl.value.split_whitespace().next()?;
    if first_token.starts_with("var(") {
        return None;
    }
    let (value, unit) = parse_dimension(first_token)?;
    if matches!(unit, Unit::Percent) {
        return None;
    }
    let base = to_base(value, unit, ScaleKind::Length);
    let result = nearest(FONT_SIZE, base);
    let mut msg = format!("use var({})", result.token);
    if result.outside_scale {
        msg.push_str(" — outside the scale");
    }
    msg.push_str(", or consider a text-* utility for body/heading text");
    Some(diagnostic("L02", file, decl, msg))
}

fn check_leading(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    let first_token = decl.value.split_whitespace().next()?;
    if first_token.starts_with("var(") {
        return None;
    }
    let (value, unit) = parse_dimension(first_token)?;
    let base = match unit {
        Unit::Unitless => value,
        Unit::Px => value / 16.0,
        Unit::Rem | Unit::Em => value,
        Unit::Percent => value / 100.0,
    };
    let result = nearest(LEADING, base);
    let mut msg = format!("use var({})", result.token);
    if result.outside_scale {
        msg.push_str(" — outside the scale");
    }
    Some(diagnostic("L03", file, decl, msg))
}

fn check_tracking(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    let first_token = decl.value.split_whitespace().next()?;
    if first_token.starts_with("var(") {
        return None;
    }
    let (value, unit) = parse_dimension(first_token)?;
    let base = to_base(value, unit, ScaleKind::Tracking);
    let result = nearest(TRACKING, base);
    let mut msg = format!("use var({})", result.token);
    if result.outside_scale {
        msg.push_str(" — outside the scale");
    }
    Some(diagnostic("L04", file, decl, msg))
}

fn diagnostic(rule: &'static str, file: &Path, decl: &Decl, message: String) -> Diagnostic {
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

// Re-export for tests that want to spot-check the underlying helpers.
#[allow(dead_code)]
pub(crate) const _UNIT_PX: Unit = Unit::Px;
#[allow(dead_code)]
pub(crate) fn _to_base_length(v: f64, u: Unit) -> f64 {
    tokens::to_base(v, u, tokens::ScaleKind::Length)
}
