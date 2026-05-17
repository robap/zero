//! Render the static `index.html` for `zero build` output.

use std::collections::HashSet;
use std::path::Path;

/// Inject script + link tags into `root/index.html` and write to `out/index.html`.
///
/// For each manifest entry whose source path matches an existing `<link>` in the
/// source HTML, the href is rewritten in-place to the hashed output URL. Entries
/// without a matching `<link>` are injected before `</head>`.
///
/// # Parameters
/// - `root`: project root directory.
/// - `out`: build output directory.
/// - `manifest`: list of `(logical-name, output-relative-path)` pairs.
///
/// # Returns
/// `Ok(())` on success.
pub fn render(root: &Path, out: &Path, manifest: &[(String, String)]) -> anyhow::Result<()> {
    let src = std::fs::read_to_string(root.join("index.html"))?;

    let (html, rewritten) = rewrite_link_hrefs(&src, manifest);

    let mut snippet = String::new();
    for (logical, out_rel) in manifest {
        if logical == "app.js" {
            snippet.push_str(&format!(
                r#"<script type="module" src="/{out_rel}"></script>"#
            ));
            snippet.push('\n');
        } else if out_rel.ends_with(".css") && !rewritten.contains(logical) {
            snippet.push_str(&format!(r#"<link rel="stylesheet" href="/{out_rel}">"#));
            snippet.push('\n');
        }
    }

    let result = inject_before_head_close(&html, &snippet);
    std::fs::write(out.join("index.html"), result)?;
    Ok(())
}

/// Walk `html`, find every `<link ... href="..." ...>` tag, and for each
/// entry in `pairs` where the href matches `source_rel` (or `/source_rel`,
/// modulo `?query` and `#fragment`), replace the href with `/<output_rel>`.
///
/// Quote style is preserved. Other attributes on the same tag are preserved.
/// Match is on `href` attribute name only (lowercase).
///
/// # Parameters
/// - `html`: the source HTML string.
/// - `pairs`: list of `(source_rel, output_rel)` pairs to match against.
///
/// # Returns
/// The modified HTML string and the set of source paths that were rewritten.
fn rewrite_link_hrefs(html: &str, pairs: &[(String, String)]) -> (String, HashSet<String>) {
    let mut result = String::with_capacity(html.len());
    let mut rewritten = HashSet::new();
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Find `<link` (case-insensitive on tag name).
        let Some(tag_start) = find_link_tag(html, pos) else {
            result.push_str(&html[pos..]);
            break;
        };

        // Push everything before this tag start.
        result.push_str(&html[pos..tag_start]);

        // Find the closing `>` of this tag.
        let tag_end = match html[tag_start..].find('>') {
            Some(rel) => tag_start + rel + 1,
            None => {
                result.push_str(&html[tag_start..]);
                break;
            }
        };

        let tag_str = &html[tag_start..tag_end];

        // Find `href` attribute within this tag (lowercase only).
        if let Some((href_val_start, href_val_end, quote_char)) = find_href(tag_str) {
            let href_val = &tag_str[href_val_start..href_val_end];
            let normalized = normalize_href(href_val);

            if let Some((_, out_rel)) = pairs.iter().find(|(src, _)| src == normalized) {
                // Rewrite: splice in the hashed output path.
                let new_href = format!("/{out_rel}");
                let mut new_tag = String::with_capacity(tag_str.len());
                new_tag.push_str(&tag_str[..href_val_start]);
                new_tag.push_str(&new_href);
                new_tag.push_str(&tag_str[href_val_end..]);
                result.push_str(&new_tag);
                rewritten.insert(normalized.to_string());
                // Also mark with the quote char removed (quote_char is just for use if needed).
                let _ = quote_char;
            } else {
                result.push_str(tag_str);
            }
        } else {
            result.push_str(tag_str);
        }

        pos = tag_end;
    }

    (result, rewritten)
}

/// Find the next `<link` start position at or after `from` (ASCII case-insensitive on `link`).
fn find_link_tag(html: &str, from: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut i = from;
    while i + 5 <= bytes.len() {
        if bytes[i] == b'<'
            && bytes[i + 1].eq_ignore_ascii_case(&b'l')
            && bytes[i + 2].eq_ignore_ascii_case(&b'i')
            && bytes[i + 3].eq_ignore_ascii_case(&b'n')
            && bytes[i + 4].eq_ignore_ascii_case(&b'k')
            && (bytes.get(i + 5).copied().unwrap_or(b'>') == b' '
                || bytes.get(i + 5).copied().unwrap_or(b'>') == b'\t'
                || bytes.get(i + 5).copied().unwrap_or(b'>') == b'\n'
                || bytes.get(i + 5).copied().unwrap_or(b'>') == b'>')
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the `href` attribute value range within a `<link ...>` tag string.
///
/// Returns `(value_start, value_end, quote_char)` where the range is the
/// byte range of the href value (without quotes) within `tag`.
fn find_href(tag: &str) -> Option<(usize, usize, u8)> {
    let bytes = tag.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 5 < len {
        // Look for `href` (lowercase only, per spec).
        if bytes[i] == b'h' && bytes[i + 1] == b'r' && bytes[i + 2] == b'e' && bytes[i + 3] == b'f'
        {
            let after = i + 4;
            // Skip optional whitespace before `=`.
            let mut j = after;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if j < len && bytes[j] == b'=' {
                j += 1;
                // Skip optional whitespace after `=`.
                while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                if j < len && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    let val_start = j + 1;
                    let close = tag[val_start..].find(quote as char)?;
                    let val_end = val_start + close;
                    return Some((val_start, val_end, quote));
                }
            }
        }
        i += 1;
    }
    None
}

/// Normalize an href for comparison with manifest source paths.
///
/// Strips `?query` and `#fragment`, then strips a leading `/`.
fn normalize_href(href: &str) -> &str {
    let stripped = href
        .split('?')
        .next()
        .unwrap_or(href)
        .split('#')
        .next()
        .unwrap_or(href);
    stripped.trim_start_matches('/')
}

/// Insert `snippet` immediately before the first case-insensitive `</head>` match.
///
/// Falls back to prepending if `</head>` is absent.
///
/// # Parameters
/// - `html`: the source HTML string.
/// - `snippet`: the text to insert.
///
/// # Returns
/// The modified HTML string.
pub fn inject_before_head_close(html: &str, snippet: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(pos) = lower.find("</head>") {
        let mut out = String::with_capacity(html.len() + snippet.len());
        out.push_str(&html[..pos]);
        out.push_str(snippet);
        out.push_str(&html[pos..]);
        return out;
    }
    format!("{snippet}{html}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn inject_before_head_close_inserts_before_head() {
        let html = "<html><head><title>X</title></head><body></body></html>";
        let result = inject_before_head_close(html, "<script src='/a.js'></script>\n");
        let script_pos = result.find("<script src='/a.js'>").unwrap();
        let head_pos = result.find("</head>").unwrap();
        assert!(script_pos < head_pos);
    }

    #[test]
    fn render_injects_script_and_link() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            "<html><head><title>X</title></head><body></body></html>",
        )
        .unwrap();
        let manifest = vec![
            ("app.js".to_string(), "assets/app.abc123.js".to_string()),
            (
                "styles/app.css".to_string(),
                "assets/app.5e8d9f01.css".to_string(),
            ),
        ];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(result.contains(r#"<script type="module" src="/assets/app.abc123.js">"#));
        assert!(result.contains(r#"<link rel="stylesheet" href="/assets/app.5e8d9f01.css">"#));
        assert!(
            result.contains("</head>"),
            "head close tag must still be present"
        );
    }

    #[test]
    fn render_rewrites_scss_link() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            r#"<html><head><link rel="stylesheet" href="/styles/app.scss"></head><body></body></html>"#,
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.scss".to_string(),
            "assets/app.abc12345.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains(r#"href="/assets/app.abc12345.css""#),
            "hashed href missing: {result}"
        );
        assert!(
            !result.contains("app.scss"),
            "source .scss still present: {result}"
        );
    }

    #[test]
    fn render_rewrites_css_link_with_single_quotes() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            r#"<html><head><link rel="stylesheet" href='/styles/app.scss'></head><body></body></html>"#,
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.scss".to_string(),
            "assets/app.abc12345.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains("href='/assets/app.abc12345.css'"),
            "single-quoted href not preserved: {result}"
        );
    }

    #[test]
    fn render_rewrites_link_without_leading_slash() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            r#"<html><head><link rel="stylesheet" href="styles/app.scss"></head><body></body></html>"#,
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.scss".to_string(),
            "assets/app.hash1234.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains(r#"href="/assets/app.hash1234.css""#),
            "leading slash not added: {result}"
        );
    }

    #[test]
    fn render_rewrites_link_stripping_query_and_fragment() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            r#"<html><head><link rel="stylesheet" href="/styles/app.scss?v=1"></head><body></body></html>"#,
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.scss".to_string(),
            "assets/app.hash1234.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains(r#"href="/assets/app.hash1234.css""#),
            "query not stripped: {result}"
        );
    }

    #[test]
    fn render_rewrites_css_link() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            r#"<html><head><link rel="stylesheet" href="/styles/app.css"></head><body></body></html>"#,
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.css".to_string(),
            "assets/app.hash5678.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains(r#"href="/assets/app.hash5678.css""#),
            "css link not rewritten: {result}"
        );
    }

    #[test]
    fn render_falls_back_to_injection_when_no_link() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            "<html><head><title>X</title></head><body></body></html>",
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.css".to_string(),
            "assets/app.fallback.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains(r#"href="/assets/app.fallback.css""#),
            "injected link missing: {result}"
        );
    }

    #[test]
    fn render_does_not_mutate_unrelated_links() {
        let root = tempdir().unwrap();
        let out = tempdir().unwrap();
        std::fs::write(
            root.path().join("index.html"),
            r#"<html><head><link rel="icon" href="/favicon.ico"><link rel="stylesheet" href="/styles/app.scss"></head><body></body></html>"#,
        )
        .unwrap();
        let manifest = vec![(
            "styles/app.scss".to_string(),
            "assets/app.abc12345.css".to_string(),
        )];
        render(root.path(), out.path(), &manifest).unwrap();
        let result = std::fs::read_to_string(out.path().join("index.html")).unwrap();
        assert!(
            result.contains(r#"href="/favicon.ico""#),
            "icon link was mutated: {result}"
        );
        assert!(
            result.contains(r#"href="/assets/app.abc12345.css""#),
            "scss link not rewritten: {result}"
        );
    }

    #[test]
    fn rewrite_link_hrefs_returns_match_set() {
        let html = r#"<html><head><link rel="stylesheet" href="/styles/app.scss"></head></html>"#;
        let pairs = vec![(
            "styles/app.scss".to_string(),
            "assets/app.abc123.css".to_string(),
        )];
        let (_, rewritten) = rewrite_link_hrefs(html, &pairs);
        assert!(
            rewritten.contains("styles/app.scss"),
            "rewritten set missing source path: {rewritten:?}"
        );
    }
}
