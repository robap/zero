//! HTML injection: insert the dev-mode script tags before `</head>`.

/// The script tags injected into every HTML response in dev mode.
pub const DEV_SCRIPTS: &str = concat!(
    r#"<script type="importmap">{"imports":{"zero":"/zero.js"}}</script>"#,
    "\n",
    r#"<script type="module" src="/src/app.js"></script>"#,
    "\n",
    "<script>\n",
    "(function(){\n",
    "  if (typeof EventSource === \"undefined\") return;\n",
    "  var es = new EventSource(\"/_zero/events\");\n",
    "  es.addEventListener(\"reload\", function(e){\n",
    "    try { console.log(\"[zero] reloading: \" + (e.data || \"\")); } catch(_) {}\n",
    "    location.reload();\n",
    "  });\n",
    "})();\n",
    "</script>"
);

/// Inject `DEV_SCRIPTS` into `body` before `</head>`, with fallbacks.
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
///
/// # Returns
/// Modified bytes with `DEV_SCRIPTS` inserted.
pub fn inject(body: &[u8]) -> Vec<u8> {
    let text = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("zero dev: HTML response was not valid UTF-8; scripts not injected");
            return body.to_vec();
        }
    };

    let lower = text.to_ascii_lowercase();

    if let Some(pos) = lower.find("</head>") {
        let snippet = format!("{DEV_SCRIPTS}\n");
        let mut out = Vec::with_capacity(body.len() + snippet.len());
        out.extend_from_slice(&body[..pos]);
        out.extend_from_slice(snippet.as_bytes());
        out.extend_from_slice(&body[pos..]);
        return out;
    }

    if let Some(pos) = lower.find("<body") {
        let snippet = format!("{DEV_SCRIPTS}\n");
        let mut out = Vec::with_capacity(body.len() + snippet.len());
        out.extend_from_slice(&body[..pos]);
        out.extend_from_slice(snippet.as_bytes());
        out.extend_from_slice(&body[pos..]);
        return out;
    }

    eprintln!("zero dev: HTML response had no <head> or <body>; scripts prepended");
    let mut out = DEV_SCRIPTS.as_bytes().to_vec();
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

    #[test]
    fn injects_reload_client_alongside_other_scripts() {
        // DEV_SCRIPTS must include all three script tags
        assert!(
            DEV_SCRIPTS.contains(r#"new EventSource("/_zero/events")"#),
            "DEV_SCRIPTS must contain the EventSource constructor"
        );
        assert!(
            DEV_SCRIPTS.contains(r#"addEventListener("reload""#),
            "DEV_SCRIPTS must contain the reload event listener"
        );
        assert!(
            DEV_SCRIPTS.contains("location.reload()"),
            "DEV_SCRIPTS must contain location.reload()"
        );
        // Existing scripts must still be present
        assert!(DEV_SCRIPTS.contains(r#"type="importmap""#));
        assert!(DEV_SCRIPTS.contains(r#"src="/src/app.js""#));
    }

    #[test]
    fn injects_before_closing_head() {
        let html = "<html><head><title>X</title></head><body></body></html>";
        let out = inject(html.as_bytes());
        let result = s(&out);
        let head_close = result.find("</head>").expect("</head> must remain");
        let script_pos = result.find(DEV_SCRIPTS).expect("DEV_SCRIPTS must appear");
        assert!(
            script_pos < head_close,
            "scripts must appear before </head>"
        );
    }

    #[test]
    fn injects_before_uppercase_head_close() {
        let html = "<HTML><HEAD><TITLE>X</TITLE></HEAD><BODY></BODY></HTML>";
        let out = inject(html.as_bytes());
        let result = s(&out);
        // Case-insensitive: `</HEAD>` was the target. The injected snippet
        // appears before whatever case the closing tag is.
        assert!(result.contains(DEV_SCRIPTS));
        let script_pos = result.find(DEV_SCRIPTS).unwrap();
        let head_close = result.find("</HEAD>").unwrap();
        assert!(script_pos < head_close);
    }

    #[test]
    fn falls_back_to_body_when_no_head_close() {
        let html = r#"<html><body class="main">hi</body></html>"#;
        let out = inject(html.as_bytes());
        let result = s(&out);
        assert!(result.contains(DEV_SCRIPTS));
        let script_pos = result.find(DEV_SCRIPTS).unwrap();
        let body_pos = result.find("<body").unwrap();
        assert!(script_pos < body_pos, "scripts must precede <body");
    }

    #[test]
    fn falls_back_to_prepend_when_no_markers() {
        let html = b"<p>just a fragment</p>";
        let out = inject(html);
        let result = s(&out);
        assert!(result.starts_with(DEV_SCRIPTS), "scripts must be prepended");
    }

    #[test]
    fn non_utf8_input_is_returned_unchanged() {
        let bad: Vec<u8> = vec![0xFF, 0xFE, 0xFD];
        let out = inject(&bad);
        assert_eq!(out, bad, "non-UTF-8 input must be returned unchanged");
    }

    #[test]
    fn false_positive_in_comment_before_real_head_close() {
        // Known limitation: the first match wins even if it's inside a comment.
        // This test documents the deterministic (wrong-but-acceptable) behavior.
        let html = "<html><head><!-- </head> fake --><title>X</title></head><body></body></html>";
        let out = inject(html.as_bytes());
        let result = s(&out);
        // Injection happens before the comment's `</head>`, not the real one.
        // Both `</head>` occurrences are still in the output.
        assert!(result.contains(DEV_SCRIPTS));
        // The real `</head>` is still present somewhere after the injected snippet.
        assert!(result.contains("</head>"));
    }
}
