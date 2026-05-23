//! SCSS → CSS compiler used by the dev server and the build pipeline.
//!
//! Wraps `grass` with a narrow function-call API. Compressed output style;
//! output is minified.

/// Options controlling a single `compile_scss` invocation.
pub struct SassOptions<'a> {
    /// Logical filename used for diagnostics and source-map source paths.
    pub filename: &'a str,
    /// Append `/*# sourceMappingURL=data:application/json;base64,... */`
    /// to the CSS body.
    pub inline_source_map: bool,
    /// Also return the raw source-map JSON string on the result.
    pub emit_source_map: bool,
    /// Extra directories to search for `@use` / `@forward` targets.
    /// The importing file's directory is always searched first.
    pub load_paths: &'a [std::path::PathBuf],
}

/// Result of a successful compile.
#[derive(Debug)]
pub struct SassOutput {
    /// The emitted CSS source (always compressed).
    pub code: String,
    /// Present only when `opts.emit_source_map == true`. JSON text.
    pub source_map: Option<String>,
}

/// Structured compile error: parser or resolution failure with location.
#[derive(Debug)]
pub struct SassError {
    /// Logical filename the error originated from.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
    /// Diagnostic message text.
    pub message: String,
}

impl std::fmt::Display for SassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.column, self.message
        )
    }
}

impl std::error::Error for SassError {}

/// Compile a `.scss` source string and return CSS.
///
/// `abs_path` is the on-disk path of `source` and is used as the root for
/// `@use` resolution. The file at `abs_path` does NOT need to exist on
/// disk — `grass::from_string` is called with the in-memory source.
///
/// # Parameters
/// - `source`: the SCSS source text.
/// - `abs_path`: absolute path used to resolve `@use` imports.
/// - `opts`: filename, sourcemap controls, and extra load paths.
///
/// # Returns
/// `Ok(SassOutput)` on success, `Err(SassError)` on compile failure.
pub fn compile_scss(
    source: &str,
    abs_path: &std::path::Path,
    opts: &SassOptions<'_>,
) -> Result<SassOutput, SassError> {
    let parent = abs_path.parent().unwrap_or(abs_path);
    let mut options = grass::Options::default()
        .style(grass::OutputStyle::Compressed)
        .quiet(true)
        .load_path(parent);
    for p in opts.load_paths {
        options = options.load_path(p);
    }

    let css = grass::from_string(source.to_string(), &options).map_err(|e| {
        let rendered = e.to_string();
        let (line, column, message) = parse_diag(&rendered);
        SassError {
            file: opts.filename.to_string(),
            line,
            column,
            message,
        }
    })?;

    let source_map_json = if opts.inline_source_map || opts.emit_source_map {
        Some(build_degenerate_source_map(opts.filename, source))
    } else {
        None
    };

    let mut code = css;
    if opts.inline_source_map
        && let Some(ref json) = source_map_json
    {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
        if !code.ends_with('\n') {
            code.push('\n');
        }
        code.push_str("/*# sourceMappingURL=data:application/json;base64,");
        code.push_str(&b64);
        code.push_str(" */\n");
    }

    let source_map = if opts.emit_source_map {
        source_map_json
    } else {
        None
    };

    Ok(SassOutput { code, source_map })
}

/// Build a degenerate (no real mappings) source map JSON string.
fn build_degenerate_source_map(filename: &str, source: &str) -> String {
    let escaped_source = source
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!(
        r#"{{"version":3,"sources":["{filename}"],"sourcesContent":["{escaped_source}"],"names":[],"mappings":""}}"#
    )
}

/// Extract `line`, `column`, and a trimmed message from a grass diagnostic string.
fn parse_diag(s: &str) -> (u32, u32, String) {
    let mut line: u32 = 1;
    let mut column: u32 = 1;
    let message = s.lines().next().unwrap_or(s).trim().to_string();
    for raw in s.lines() {
        if let Some(captures) = find_line_col(raw) {
            line = captures.0;
            column = captures.1;
            break;
        }
    }
    (line, column, message)
}

/// Look for a `N:M` style line/column pair within a string.
fn find_line_col(s: &str) -> Option<(u32, u32)> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j + 1 {
                    let line: u32 = s[i..j].parse().ok()?;
                    let col: u32 = s[(j + 1)..k].parse().ok()?;
                    if line > 0 && col > 0 {
                        return Some((line, col));
                    }
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn opts(filename: &str) -> SassOptions<'_> {
        SassOptions {
            filename,
            inline_source_map: false,
            emit_source_map: false,
            load_paths: &[],
        }
    }

    fn abs_path() -> PathBuf {
        std::env::temp_dir().join("test.scss")
    }

    #[test]
    fn compiles_basic_scss() {
        let src = "$c: red; body { color: $c; }";
        let out = compile_scss(src, &abs_path(), &opts("test.scss")).expect("compile ok");
        assert!(
            out.code.contains("body{"),
            "missing body block: {}",
            out.code
        );
        assert!(
            out.code.contains("color:red"),
            "variable not resolved: {}",
            out.code
        );
        assert!(!out.code.contains('$'), "raw variable leaked: {}", out.code);
    }

    #[test]
    fn compiles_nested_selectors() {
        let src = ".outer { .inner { color: red; } }";
        let out = compile_scss(src, &abs_path(), &opts("test.scss")).expect("compile ok");
        assert!(
            out.code.contains(".outer .inner"),
            "nested selector not flattened: {}",
            out.code
        );
    }

    #[test]
    fn resolves_partial_via_at_use() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("_buttons.scss"), "$btn-padding: 8px;").unwrap();
        let main_path = dir.path().join("main.scss");
        let src = "@use 'buttons'; .btn { padding: buttons.$btn-padding; }";
        let opts = SassOptions {
            filename: "main.scss",
            inline_source_map: false,
            emit_source_map: false,
            load_paths: &[],
        };
        let out = compile_scss(src, &main_path, &opts).expect("compile ok");
        assert!(
            out.code.contains("padding:8px"),
            "partial not resolved: {}",
            out.code
        );
    }

    #[test]
    fn inline_source_map_appended_when_requested() {
        let opts = SassOptions {
            filename: "a.scss",
            inline_source_map: true,
            emit_source_map: false,
            load_paths: &[],
        };
        let out = compile_scss("body { color: red; }", &abs_path(), &opts).expect("ok");
        assert!(
            out.code
                .contains("/*# sourceMappingURL=data:application/json;base64,"),
            "inline sourcemap missing: {}",
            out.code
        );
        assert!(
            out.code.trim_end().ends_with("*/"),
            "sourcemap comment not closed: {}",
            out.code
        );
    }

    #[test]
    fn external_source_map_returned_when_requested() {
        let opts = SassOptions {
            filename: "a.scss",
            inline_source_map: false,
            emit_source_map: true,
            load_paths: &[],
        };
        let out = compile_scss("body { color: red; }", &abs_path(), &opts).expect("ok");
        let json = out.source_map.expect("source_map should be Some");
        assert!(
            json.contains("\"version\":3"),
            "source map missing version: {json}"
        );
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let sources = v["sources"].as_array().expect("sources array");
        assert_eq!(sources[0].as_str().unwrap(), "a.scss");
    }

    #[test]
    fn parse_error_returns_structured_error() {
        let src = "body { color: ; }";
        let err = compile_scss(src, &abs_path(), &opts("bad.scss")).expect_err("expected error");
        assert!(err.line >= 1, "line should be >= 1, got {}", err.line);
        assert!(err.column >= 1, "column should be >= 1, got {}", err.column);
        assert!(!err.message.is_empty(), "message empty");
    }

    #[test]
    fn unknown_at_use_returns_structured_error() {
        let src = "@use 'nonexistent';";
        let err = compile_scss(src, &abs_path(), &opts("bad.scss")).expect_err("expected error");
        assert!(
            err.message.to_lowercase().contains("nonexistent")
                || err.message.to_lowercase().contains("can't find"),
            "error message doesn't mention missing module: {}",
            err.message
        );
    }

    #[test]
    fn compressed_output_drops_whitespace() {
        let src = "body { color: red;\n  padding: 8px; }";
        let out = compile_scss(src, &abs_path(), &opts("test.scss")).expect("ok");
        assert!(
            !out.code.contains("\n  "),
            "expected no indented newlines in compressed output: {}",
            out.code
        );
        assert!(
            !out.code.contains("  "),
            "expected no double-space runs in compressed output: {}",
            out.code
        );
    }
}
