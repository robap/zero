//! L05 — color literals.
//!
//! Flags `color`, `background`, `background-color`, `border-color`,
//! `fill`, `stroke`, `outline-color` when the value is a hex literal,
//! `rgb()`/`rgba()`, `hsl()`/`hsla()`, or a named color. Suggests the
//! semantic `--color-*` token nearest in sRGB Euclidean distance.

use super::is_whitelisted_value;
use crate::Diagnostic;
use crate::scan::Decl;
use std::path::Path;

const COLOR_TOKENS: &[(&str, [u8; 3])] = &[
    ("--color-bg", [0xff, 0xff, 0xff]),
    ("--color-surface", [0xf1, 0xf3, 0xf5]),
    ("--color-surface-strong", [0xe9, 0xec, 0xef]),
    ("--color-text", [0x21, 0x25, 0x29]),
    ("--color-text-muted", [0x86, 0x8e, 0x96]),
    ("--color-text-subtle", [0xad, 0xb5, 0xbd]),
    ("--color-border", [0xe9, 0xec, 0xef]),
    ("--color-border-strong", [0xce, 0xd4, 0xda]),
    ("--color-primary", [0x22, 0x8b, 0xe6]),
    ("--color-primary-soft", [0xe7, 0xf5, 0xff]),
    ("--color-success", [0x37, 0xb2, 0x4d]),
    ("--color-warning", [0xff, 0x92, 0x2b]),
    ("--color-danger", [0xf0, 0x3e, 0x3e]),
];

const COLOR_PROPERTIES: &[&str] = &[
    "color",
    "background",
    "background-color",
    "border-color",
    "fill",
    "stroke",
    "outline-color",
];

pub fn check(file: &Path, decl: &Decl) -> Option<Diagnostic> {
    if !COLOR_PROPERTIES.iter().any(|p| *p == decl.property) {
        return None;
    }
    if is_whitelisted_value(&decl.value) {
        return None;
    }
    // If the value already references a --color-* var, skip.
    if decl.value.contains("var(--color-") {
        return None;
    }
    // For `background` shorthand we must scan the whole value for the
    // first color-shaped token; for the other properties we look at the
    // first token only.
    let candidate = first_color_token(&decl.value)?;
    let rgb = parse_color(&candidate)?;
    let token = nearest_color_token(rgb);
    Some(Diagnostic {
        rule: "L05",
        file: file.to_path_buf(),
        line: decl.line,
        column: decl.column,
        property: decl.property.clone(),
        value: decl.value.clone(),
        message: format!("use var({token})"),
    })
}

fn first_color_token(value: &str) -> Option<String> {
    // Walk top-level tokens; the first one that parses as a color wins.
    let mut depth = 0i32;
    let mut buf = String::new();
    let mut out: Vec<String> = Vec::new();
    for ch in value.chars() {
        match ch {
            '(' => {
                depth += 1;
                buf.push(ch);
            }
            ')' => {
                depth -= 1;
                buf.push(ch);
            }
            ' ' | ',' if depth == 0 => {
                if !buf.trim().is_empty() {
                    out.push(buf.trim().to_string());
                }
                buf.clear();
            }
            _ => buf.push(ch),
        }
    }
    if !buf.trim().is_empty() {
        out.push(buf.trim().to_string());
    }
    out.into_iter().find(|t| parse_color(t).is_some())
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('#') {
        return parse_hex(rest);
    }
    let lower = s.to_ascii_lowercase();
    if let Some(rgb) = parse_rgb_func(&lower) {
        return Some(rgb);
    }
    if let Some(rgb) = parse_hsl_func(&lower) {
        return Some(rgb);
    }
    parse_named_color(&lower)
}

fn parse_hex(rest: &str) -> Option<[u8; 3]> {
    let bytes = rest.as_bytes();
    let hex = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    match bytes.len() {
        3 | 4 => {
            let r = hex(bytes[0])?;
            let g = hex(bytes[1])?;
            let b = hex(bytes[2])?;
            Some([(r << 4) | r, (g << 4) | g, (b << 4) | b])
        }
        6 | 8 => {
            let r = (hex(bytes[0])? << 4) | hex(bytes[1])?;
            let g = (hex(bytes[2])? << 4) | hex(bytes[3])?;
            let b = (hex(bytes[4])? << 4) | hex(bytes[5])?;
            Some([r, g, b])
        }
        _ => None,
    }
}

fn parse_rgb_func(s: &str) -> Option<[u8; 3]> {
    let inner = strip_func(s, "rgb").or_else(|| strip_func(s, "rgba"))?;
    let parts: Vec<&str> = inner.split([',', '/']).collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parse_channel(parts[0])?;
    let g = parse_channel(parts[1])?;
    let b = parse_channel(parts[2])?;
    Some([r, g, b])
}

fn parse_channel(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let v: f64 = pct.trim().parse().ok()?;
        Some(((v / 100.0) * 255.0).round().clamp(0.0, 255.0) as u8)
    } else {
        let v: f64 = s.parse().ok()?;
        Some(v.round().clamp(0.0, 255.0) as u8)
    }
}

fn parse_hsl_func(s: &str) -> Option<[u8; 3]> {
    let inner = strip_func(s, "hsl").or_else(|| strip_func(s, "hsla"))?;
    let parts: Vec<&str> = inner.split([',', '/']).collect();
    if parts.len() < 3 {
        return None;
    }
    let h: f64 = parts[0].trim().trim_end_matches("deg").parse().ok()?;
    let s_pct: f64 = parts[1].trim().trim_end_matches('%').trim().parse().ok()?;
    let l_pct: f64 = parts[2].trim().trim_end_matches('%').trim().parse().ok()?;
    Some(hsl_to_rgb(h, s_pct / 100.0, l_pct / 100.0))
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_norm = (h % 360.0 + 360.0) % 360.0 / 60.0;
    let x = c * (1.0 - (h_norm % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_norm as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

fn strip_func<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}(");
    if s.starts_with(&prefix) && s.ends_with(')') {
        Some(&s[prefix.len()..s.len() - 1])
    } else {
        None
    }
}

fn parse_named_color(name: &str) -> Option<[u8; 3]> {
    // Top ~30 CSS named colors an agent is likely to type.
    let table: &[(&str, [u8; 3])] = &[
        ("white", [255, 255, 255]),
        ("black", [0, 0, 0]),
        ("red", [255, 0, 0]),
        ("green", [0, 128, 0]),
        ("blue", [0, 0, 255]),
        ("yellow", [255, 255, 0]),
        ("orange", [255, 165, 0]),
        ("purple", [128, 0, 128]),
        ("pink", [255, 192, 203]),
        ("brown", [165, 42, 42]),
        ("gray", [128, 128, 128]),
        ("grey", [128, 128, 128]),
        ("silver", [192, 192, 192]),
        ("gold", [255, 215, 0]),
        ("lime", [0, 255, 0]),
        ("teal", [0, 128, 128]),
        ("cyan", [0, 255, 255]),
        ("magenta", [255, 0, 255]),
        ("maroon", [128, 0, 0]),
        ("navy", [0, 0, 128]),
        ("olive", [128, 128, 0]),
        ("violet", [238, 130, 238]),
        ("indigo", [75, 0, 130]),
        ("salmon", [250, 128, 114]),
        ("coral", [255, 127, 80]),
        ("crimson", [220, 20, 60]),
        ("turquoise", [64, 224, 208]),
        ("khaki", [240, 230, 140]),
        ("beige", [245, 245, 220]),
        ("ivory", [255, 255, 240]),
    ];
    table.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

fn nearest_color_token(rgb: [u8; 3]) -> &'static str {
    let mut best = COLOR_TOKENS[0].0;
    let mut best_d = sq_dist(rgb, COLOR_TOKENS[0].1);
    for (name, c) in COLOR_TOKENS.iter().skip(1) {
        let d = sq_dist(rgb, *c);
        if d < best_d {
            best_d = d;
            best = name;
        }
    }
    best
}

fn sq_dist(a: [u8; 3], b: [u8; 3]) -> i64 {
    let dr = a[0] as i64 - b[0] as i64;
    let dg = a[1] as i64 - b[1] as i64;
    let db = a[2] as i64 - b[2] as i64;
    dr * dr + dg * dg + db * db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_long_and_short() {
        assert_eq!(parse_color("#fff"), Some([255, 255, 255]));
        assert_eq!(parse_color("#000000"), Some([0, 0, 0]));
        assert_eq!(parse_color("#228be6"), Some([0x22, 0x8b, 0xe6]));
    }

    #[test]
    fn parse_named() {
        assert_eq!(parse_color("red"), Some([255, 0, 0]));
        assert_eq!(parse_color("WHITE"), Some([255, 255, 255]));
    }

    #[test]
    fn parse_rgb() {
        assert_eq!(parse_color("rgb(255, 0, 0)"), Some([255, 0, 0]));
        assert_eq!(parse_color("rgba(0, 255, 0, 0.5)"), Some([0, 255, 0]));
    }
}
