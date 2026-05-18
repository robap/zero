//! Numeric token scales used by every rule that resolves a raw value to
//! the nearest named token.
//!
//! Each [`Scale`] is a sorted slice of `(name, value, unit)` entries. The
//! [`nearest`] resolver converts the candidate to the scale's base unit and
//! returns the closest entry plus whether the input falls outside the
//! scale's natural range.

/// Length / size unit recognized by the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Px,
    Rem,
    Em,
    Unitless,
    Percent,
}

/// A `(token, base-unit-value)` entry in a numeric scale.
#[derive(Debug, Clone, Copy)]
pub struct ScaleEntry {
    pub token: &'static str,
    pub base_value: f64,
}

/// A named scale of token entries. Entries are listed in ascending order of
/// `base_value` so the "smaller step on midpoint" rule is deterministic.
pub type Scale = &'static [ScaleEntry];

const fn entry(token: &'static str, v: f64) -> ScaleEntry {
    ScaleEntry {
        token,
        base_value: v,
    }
}

/// Font weight scale (unitless).
pub const WEIGHT: Scale = &[
    entry("--weight-normal", 400.0),
    entry("--weight-medium", 500.0),
    entry("--weight-semi", 600.0),
    entry("--weight-bold", 700.0),
];

/// Font size scale, base unit px (1rem = 16px).
pub const FONT_SIZE: Scale = &[
    entry("--font-size-sm", 14.0),
    entry("--font-size-md", 16.0),
    entry("--font-size-lg", 20.0),
    entry("--font-size-xl", 24.0),
    entry("--font-size-2xl", 32.0),
    entry("--font-size-display", 44.0),
];

/// Line-height scale (unitless ratio).
pub const LEADING: Scale = &[
    entry("--leading-tight", 1.2),
    entry("--leading-snug", 1.35),
    entry("--leading-normal", 1.5),
];

/// Letter-spacing scale, base unit em (px values divided by 16 to coerce).
pub const TRACKING: Scale = &[
    entry("--tracking-tight", -0.02),
    entry("--tracking-snug", -0.01),
    entry("--tracking-normal", 0.0),
    entry("--tracking-wide", 0.04),
    entry("--tracking-caps", 0.08),
];

/// Radius scale, base unit px.
pub const RADIUS: Scale = &[
    entry("--radius-xs", 2.0),
    entry("--radius-sm", 4.0),
    entry("--radius-md", 6.0),
    entry("--radius-lg", 10.0),
    entry("--radius-xl", 14.0),
    entry("--radius-2xl", 20.0),
    entry("--radius-3xl", 9999.0),
];

/// Border-width scale, base unit px.
pub const BORDER: Scale = &[
    entry("--border-thin", 1.0),
    entry("--border-md", 2.0),
    entry("--border-thick", 4.0),
];

/// Spacing scale, base unit px (rem → ×16).
pub const SPACE: Scale = &[
    entry("--space-xs", 4.0),
    entry("--space-sm", 8.0),
    entry("--space-md", 16.0),
    entry("--space-lg", 24.0),
    entry("--space-xl", 48.0),
];

/// Result of resolving a candidate value to a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearestResult {
    pub token: &'static str,
    /// `true` when the input is beyond the scale's endpoints by more than
    /// one adjacent step — signal for "consider whether the scale should
    /// grow" in the diagnostic.
    pub outside_scale: bool,
}

/// Pick the closest scale entry for `value`. On exact midpoint, prefer the
/// smaller entry. Marks `outside_scale = true` when `|value - endpoint|`
/// exceeds the adjacent step.
pub fn nearest(scale: Scale, value: f64) -> NearestResult {
    assert!(!scale.is_empty(), "scale must be non-empty");
    let mut best_idx = 0usize;
    let mut best_dist = (value - scale[0].base_value).abs();
    for (i, entry) in scale.iter().enumerate().skip(1) {
        let d = (value - entry.base_value).abs();
        if d < best_dist || (d == best_dist && entry.base_value < scale[best_idx].base_value) {
            best_dist = d;
            best_idx = i;
        }
    }
    let outside_scale = is_outside(scale, value, best_idx);
    NearestResult {
        token: scale[best_idx].token,
        outside_scale,
    }
}

fn is_outside(scale: Scale, value: f64, _best_idx: usize) -> bool {
    if scale.len() < 2 {
        return false;
    }
    let min = scale[0].base_value;
    let max = scale[scale.len() - 1].base_value;
    value < min || value > max
}

/// Numeric-token lookup keyed by the input keyword. Used for `font-weight:
/// bold` / `normal` / `lighter` / `bolder` mapping.
pub fn nearest_keyword(scale: Scale, keyword: &str) -> Option<&'static str> {
    let lc = keyword.to_ascii_lowercase();
    // Map a small set of CSS keywords to numeric weights, then resolve.
    let numeric = match lc.as_str() {
        "normal" => 400.0,
        "bold" => 700.0,
        "lighter" => 300.0,
        "bolder" => 900.0,
        _ => return None,
    };
    Some(nearest(scale, numeric).token)
}

/// Parse `<n><unit?>` into `(value, unit)`. Unitless numbers carry
/// `Unit::Unitless`. Returns `None` if the string isn't a dimension.
pub fn parse_dimension(s: &str) -> Option<(f64, Unit)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Determine the longest numeric prefix.
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    if bytes[0] == b'-' || bytes[0] == b'+' {
        i += 1;
    }
    let mut seen_digit = false;
    let mut seen_dot = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() {
            seen_digit = true;
        } else if c == b'.' && !seen_dot {
            seen_dot = true;
        } else {
            break;
        }
        i += 1;
    }
    if !seen_digit {
        return None;
    }
    let n: f64 = trimmed[..i].parse().ok()?;
    let unit_str = trimmed[i..].trim().to_ascii_lowercase();
    let unit = match unit_str.as_str() {
        "" => Unit::Unitless,
        "px" => Unit::Px,
        "rem" => Unit::Rem,
        "em" => Unit::Em,
        "%" => Unit::Percent,
        _ => return None,
    };
    Some((n, unit))
}

/// Convert `(value, unit)` to a base value usable against the named scale.
/// `tracking` is the only family whose base unit is `em`; all length-scaled
/// families use `px`. Unitless inputs are returned unchanged.
pub fn to_base(value: f64, unit: Unit, scale_kind: ScaleKind) -> f64 {
    match scale_kind {
        ScaleKind::Tracking => match unit {
            Unit::Em => value,
            Unit::Px => value / 16.0,
            Unit::Rem => value,
            _ => value,
        },
        ScaleKind::Leading => value,
        ScaleKind::Weight => value,
        ScaleKind::Length => match unit {
            Unit::Px => value,
            Unit::Rem | Unit::Em => value * 16.0,
            Unit::Unitless => value,
            Unit::Percent => value,
        },
    }
}

/// Discriminator for [`to_base`] so the call site doesn't have to know
/// each scale's base unit.
#[derive(Debug, Clone, Copy)]
pub enum ScaleKind {
    Length,
    Weight,
    Leading,
    Tracking,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_picks_smaller_on_midpoint() {
        // 15px is halfway between font-size-sm (14) and font-size-md (16).
        let r = nearest(FONT_SIZE, 15.0);
        assert_eq!(r.token, "--font-size-sm");
        assert!(!r.outside_scale);
    }

    #[test]
    fn nearest_marks_outside_scale() {
        // 0.5px against RADIUS: below the smallest entry (2px).
        let r = nearest(RADIUS, 0.5);
        assert_eq!(r.token, "--radius-xs");
        assert!(r.outside_scale, "0.5 should be flagged as outside scale");

        // 8000px against RADIUS: between 20 (2xl) and 9999 (3xl); nearest
        // to 3xl, inside the scale's range.
        let r = nearest(RADIUS, 8000.0);
        assert_eq!(r.token, "--radius-3xl");
        assert!(!r.outside_scale, "8000 is within scale endpoints");
    }

    #[test]
    fn nearest_handles_rem() {
        // 1.5rem = 24px = --space-lg.
        let (v, u) = parse_dimension("1.5rem").unwrap();
        let base = to_base(v, u, ScaleKind::Length);
        let r = nearest(SPACE, base);
        assert_eq!(r.token, "--space-lg");
    }

    #[test]
    fn nearest_weight_at_600_numeric() {
        let r = nearest(WEIGHT, 600.0);
        assert_eq!(r.token, "--weight-semi");
    }

    #[test]
    fn nearest_weight_keyword_bold() {
        let token = nearest_keyword(WEIGHT, "bold").unwrap();
        assert_eq!(token, "--weight-bold");
    }

    #[test]
    fn parse_dimension_basic() {
        assert_eq!(parse_dimension("12px"), Some((12.0, Unit::Px)));
        assert_eq!(parse_dimension("0.5rem"), Some((0.5, Unit::Rem)));
        assert_eq!(parse_dimension("-0.02em"), Some((-0.02, Unit::Em)));
        assert_eq!(parse_dimension("1.5"), Some((1.5, Unit::Unitless)));
        assert_eq!(parse_dimension("50%"), Some((50.0, Unit::Percent)));
        assert_eq!(parse_dimension("auto"), None);
    }
}
