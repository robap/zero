//! Render the static `index.html` for `zero build` output.

use std::path::Path;

/// Inject script + link tags into `root/index.html` and write to `out/index.html`.
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

    let mut snippet = String::new();
    for (logical, out_rel) in manifest {
        if logical == "app.js" {
            snippet.push_str(&format!(
                r#"<script type="module" src="/{out_rel}"></script>"#
            ));
            snippet.push('\n');
        } else if out_rel.ends_with(".css") {
            snippet.push_str(&format!(r#"<link rel="stylesheet" href="/{out_rel}">"#));
            snippet.push('\n');
        }
    }

    let result = inject_before_head_close(&src, &snippet);
    std::fs::write(out.join("index.html"), result)?;
    Ok(())
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
}
