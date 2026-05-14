//! HTML injection: insert the dev-mode script tags before `</head>`.

/// Build the script tags injected into HTML responses in dev mode.
///
/// # Parameters
/// - `app_entry_href`: URL the bootstrap `<script type="module">` should load.
///
/// # Returns
/// Concatenated HTML string with importmap, app-entry script, and reload-client.
pub fn dev_scripts(app_entry_href: &str) -> String {
    let mut s = String::new();
    s.push_str(r#"<script type="importmap">{"imports":{"zero":"/zero.js"}}</script>"#);
    s.push('\n');
    s.push_str(&format!(
        "<script type=\"module\" src=\"{app_entry_href}\"></script>\n"
    ));
    s.push_str("<script>\n");
    s.push_str("(function(){\n");
    s.push_str("  if (typeof EventSource === \"undefined\") return;\n");
    s.push_str("  var es = new EventSource(\"/_zero/events\");\n");
    s.push_str("  es.addEventListener(\"reload\", function(e){\n");
    s.push_str("    try { console.log(\"[zero] reloading: \" + (e.data || \"\")); } catch(_) {}\n");
    s.push_str("    location.reload();\n");
    s.push_str("  });\n");
    s.push_str("})();\n");
    s.push_str("</script>");
    s
}

/// Inject dev scripts into `body` before `</head>`, with fallbacks.
///
/// Strategy:
/// 1. Insert before the first case-insensitive `</head>` match.
/// 2. If absent, insert before the first case-insensitive `<body` match.
/// 3. If neither is found, prepend and emit a warning to stderr.
///
/// Non-UTF-8 input is returned unchanged with a warning logged to stderr.
///
/// # Parameters
/// - `body`: raw HTML bytes.
/// - `app_entry_href`: URL of the project's bootstrap script (e.g. `/src/app.ts`).
///
/// # Returns
/// Modified bytes with dev scripts inserted.
pub fn inject(body: &[u8], app_entry_href: &str) -> Vec<u8> {
    let dev_scripts_str = dev_scripts(app_entry_href);
    let text = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("zero dev: HTML response was not valid UTF-8; scripts not injected");
            return body.to_vec();
        }
    };

    let lower = text.to_ascii_lowercase();

    if let Some(pos) = lower.find("</head>") {
        let snippet = format!("{dev_scripts_str}\n");
        let mut out = Vec::with_capacity(body.len() + snippet.len());
        out.extend_from_slice(&body[..pos]);
        out.extend_from_slice(snippet.as_bytes());
        out.extend_from_slice(&body[pos..]);
        return out;
    }

    if let Some(pos) = lower.find("<body") {
        let snippet = format!("{dev_scripts_str}\n");
        let mut out = Vec::with_capacity(body.len() + snippet.len());
        out.extend_from_slice(&body[..pos]);
        out.extend_from_slice(snippet.as_bytes());
        out.extend_from_slice(&body[pos..]);
        return out;
    }

    eprintln!("zero dev: HTML response had no <head> or <body>; scripts prepended");
    let mut out = dev_scripts_str.into_bytes();
    out.push(b'\n');
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(bytes: &[u8]) -> &str {
        std::str::from_utf8(bytes).unwrap()
    }

    fn default_scripts() -> String {
        dev_scripts("/src/app.js")
    }

    #[test]
    fn dev_scripts_uses_ts_entry_when_provided() {
        let s = dev_scripts("/src/app.ts");
        assert!(s.contains(r#"src="/src/app.ts""#));
        assert!(!s.contains(r#"src="/src/app.js""#));
    }

    #[test]
    fn dev_scripts_uses_js_entry_when_provided() {
        let s = dev_scripts("/src/app.js");
        assert!(s.contains(r#"src="/src/app.js""#));
    }

    #[test]
    fn injects_reload_client_alongside_other_scripts() {
        let s = default_scripts();
        assert!(s.contains(r#"new EventSource("/_zero/events")"#));
        assert!(s.contains(r#"addEventListener("reload""#));
        assert!(s.contains("location.reload()"));
        assert!(s.contains(r#"type="importmap""#));
        assert!(s.contains(r#"src="/src/app.js""#));
    }

    #[test]
    fn injects_before_closing_head() {
        let scripts = default_scripts();
        let html = "<html><head><title>X</title></head><body></body></html>";
        let out = inject(html.as_bytes(), "/src/app.js");
        let result = s(&out);
        let head_close = result.find("</head>").expect("</head> must remain");
        let script_pos = result.find(&scripts).expect("scripts must appear");
        assert!(script_pos < head_close);
    }

    #[test]
    fn injects_before_uppercase_head_close() {
        let scripts = default_scripts();
        let html = "<HTML><HEAD><TITLE>X</TITLE></HEAD><BODY></BODY></HTML>";
        let out = inject(html.as_bytes(), "/src/app.js");
        let result = s(&out);
        assert!(result.contains(&scripts));
        let script_pos = result.find(&scripts).unwrap();
        let head_close = result.find("</HEAD>").unwrap();
        assert!(script_pos < head_close);
    }

    #[test]
    fn falls_back_to_body_when_no_head_close() {
        let scripts = default_scripts();
        let html = r#"<html><body class="main">hi</body></html>"#;
        let out = inject(html.as_bytes(), "/src/app.js");
        let result = s(&out);
        assert!(result.contains(&scripts));
        let script_pos = result.find(&scripts).unwrap();
        let body_pos = result.find("<body").unwrap();
        assert!(script_pos < body_pos);
    }

    #[test]
    fn falls_back_to_prepend_when_no_markers() {
        let scripts = default_scripts();
        let html = b"<p>just a fragment</p>";
        let out = inject(html, "/src/app.js");
        let result = s(&out);
        assert!(result.starts_with(&scripts));
    }

    #[test]
    fn non_utf8_input_is_returned_unchanged() {
        let bad: Vec<u8> = vec![0xFF, 0xFE, 0xFD];
        let out = inject(&bad, "/src/app.js");
        assert_eq!(out, bad);
    }

    #[test]
    fn false_positive_in_comment_before_real_head_close() {
        let scripts = default_scripts();
        let html = "<html><head><!-- </head> fake --><title>X</title></head><body></body></html>";
        let out = inject(html.as_bytes(), "/src/app.js");
        let result = s(&out);
        assert!(result.contains(&scripts));
        assert!(result.contains("</head>"));
    }
}
