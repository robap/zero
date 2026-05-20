//! TypeScript → JavaScript transpiler used by the dev server, bundler, and test runner.
//!
//! Wraps `swc_core` with a narrow function-call API. Type-strip only; decorators
//! and JSX are intentionally disabled.

pub use swc_core::common::{BytePos, SourceMap, Span, sync::Lrc};
pub use swc_core::ecma::ast;

/// Parsed module + its `SourceMap`, returned by [`parse_module`].
pub struct ParsedModule {
    /// The parsed SWC AST module.
    pub module: ast::Module,
    /// The `SourceMap` the module was parsed against. Callers use it to
    /// translate `Span`s into line / column positions.
    pub source_map: Lrc<SourceMap>,
}

/// Parse `source` as a TypeScript module and return the AST + its `SourceMap`.
///
/// # Parameters
/// - `source`: the TS/JS source text.
/// - `filename`: logical filename used for diagnostics + source-map source paths.
///
/// # Returns
/// `Ok(ParsedModule)` on success, `Err(TranspileError)` with line / column /
/// message on parse failure.
pub fn parse_module(source: &str, filename: &str) -> Result<ParsedModule, TranspileError> {
    let cm: Lrc<SourceMap> = Default::default();
    let module = parse_ts_source(&cm, source, filename)?;
    Ok(ParsedModule {
        module,
        source_map: cm,
    })
}

/// Options controlling a single `transpile_typescript` invocation.
pub struct TranspileOptions<'a> {
    /// Logical filename used for diagnostics and source-map source paths.
    pub filename: &'a str,
    /// Append `//# sourceMappingURL=data:application/json;base64,...` to the JS.
    pub inline_source_map: bool,
    /// Also return the raw source-map JSON string on the result.
    pub emit_source_map: bool,
}

/// Result of a successful transpile.
#[derive(Debug)]
pub struct TranspileOutput {
    /// The emitted JavaScript source.
    pub code: String,
    /// Present only when `opts.emit_source_map == true`. JSON text.
    pub source_map: Option<String>,
}

/// Structured transpile error: parser or transform failure with location.
#[derive(Debug)]
pub struct TranspileError {
    /// Logical filename the error originated from.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
    /// Diagnostic message text.
    pub message: String,
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.column, self.message
        )
    }
}

impl std::error::Error for TranspileError {}

/// Strip TypeScript types from `source` and return JavaScript.
///
/// # Parameters
/// - `source`: the TS source text.
/// - `opts`: filename + source-map controls.
///
/// # Returns
/// `Ok(TranspileOutput)` on success, `Err(TranspileError)` on parse/transform failure.
pub fn transpile_typescript(
    source: &str,
    opts: &TranspileOptions<'_>,
) -> Result<TranspileOutput, TranspileError> {
    let ParsedModule {
        module,
        source_map: cm,
    } = parse_module(source, opts.filename)?;
    let module = strip_types(module);
    let (mut code, srcmap_buf) = emit_js(&cm, &module, opts.filename)?;
    let source_map_json = if opts.inline_source_map || opts.emit_source_map {
        Some(serialize_source_map(&cm, &srcmap_buf, opts.filename)?)
    } else {
        None
    };
    if opts.inline_source_map
        && let Some(json) = &source_map_json
    {
        append_inline_source_map(&mut code, json);
    }
    let source_map = if opts.emit_source_map {
        source_map_json
    } else {
        None
    };
    Ok(TranspileOutput { code, source_map })
}

/// Parse `source` as a TypeScript module, surfacing the first SWC diagnostic
/// as a `TranspileError`.
fn parse_ts_source(
    cm: &swc_core::common::sync::Lrc<swc_core::common::SourceMap>,
    source: &str,
    filename: &str,
) -> Result<swc_core::ecma::ast::Module, TranspileError> {
    use std::sync::Arc;

    use swc_core::common::FileName;
    use swc_core::common::errors::{Handler, HandlerFlags};
    use swc_core::common::sync::Lrc;
    use swc_core::ecma::ast::EsVersion;
    use swc_core::ecma::parser::{Parser, StringInput, Syntax, TsSyntax, lexer::Lexer};

    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom(filename.to_string())),
        source.to_string(),
    );
    let buf: Arc<std::sync::Mutex<Vec<u8>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let handler = Handler::with_emitter_and_flags(
        Box::new(EmitterAdapter {
            cm: cm.clone(),
            writer: ErrorWriter { buf: buf.clone() },
        }),
        HandlerFlags {
            can_emit_warnings: false,
            treat_err_as_bug: false,
            ..Default::default()
        },
    );
    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            decorators: false,
            tsx: false,
            ..Default::default()
        }),
        EsVersion::EsNext,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module_result = parser.parse_module();
    for err in parser.take_errors() {
        err.into_diagnostic(&handler).emit();
    }
    let module = match module_result {
        Ok(m) => m,
        Err(e) => {
            e.into_diagnostic(&handler).emit();
            return Err(first_diagnostic(&buf, cm, filename));
        }
    };
    if handler.has_errors() {
        return Err(first_diagnostic(&buf, cm, filename));
    }
    Ok(module)
}

/// Run the resolver → strip → hygiene → fixer pipeline on `module`.
fn strip_types(module: swc_core::ecma::ast::Module) -> swc_core::ecma::ast::Module {
    use swc_core::common::{GLOBALS, Globals, Mark};
    use swc_core::ecma::ast::Program;
    use swc_core::ecma::transforms::base::{fixer::fixer, hygiene::hygiene, resolver};
    use swc_core::ecma::transforms::typescript::strip;

    GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();
        let mut program = Program::Module(module);
        program.mutate(resolver(unresolved_mark, top_level_mark, true));
        program.mutate(strip(unresolved_mark, top_level_mark));
        program.mutate(hygiene());
        program.mutate(fixer(None));
        match program {
            Program::Module(m) => m,
            Program::Script(_) => unreachable!("input parsed as module"),
        }
    })
}

/// Emit the post-strip module as JS, returning the code and the SWC sourcemap
/// position buffer.
type SrcmapBuf = Vec<(swc_core::common::BytePos, swc_core::common::LineCol)>;

fn emit_js(
    cm: &swc_core::common::sync::Lrc<swc_core::common::SourceMap>,
    module: &swc_core::ecma::ast::Module,
    filename: &str,
) -> Result<(String, SrcmapBuf), TranspileError> {
    use swc_core::ecma::codegen::Emitter;
    use swc_core::ecma::codegen::text_writer::JsWriter;

    let mut code_buf: Vec<u8> = Vec::new();
    let mut srcmap_buf: SrcmapBuf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut code_buf, Some(&mut srcmap_buf));
        let mut emitter = Emitter {
            cfg: swc_core::ecma::codegen::Config::default(),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        emitter.emit_module(module).map_err(|e| TranspileError {
            file: filename.to_string(),
            line: 0,
            column: 0,
            message: format!("codegen error: {e}"),
        })?;
    }
    let code = String::from_utf8(code_buf).map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("non-UTF-8 codegen output: {e}"),
    })?;
    Ok((code, srcmap_buf))
}

/// Build a JSON source-map string from `srcmap_buf`.
fn serialize_source_map(
    cm: &swc_core::common::sync::Lrc<swc_core::common::SourceMap>,
    srcmap_buf: &SrcmapBuf,
    filename: &str,
) -> Result<String, TranspileError> {
    let map = cm.build_source_map(
        srcmap_buf,
        None,
        swc_core::common::source_map::DefaultSourceMapGenConfig,
    );
    let mut json: Vec<u8> = Vec::new();
    map.to_writer(&mut json).map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("sourcemap serialization failed: {e}"),
    })?;
    String::from_utf8(json).map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("non-UTF-8 sourcemap json: {e}"),
    })
}

/// Append `//# sourceMappingURL=data:application/json;base64,…` to `code`.
fn append_inline_source_map(code: &mut String, json: &str) {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
    if !code.ends_with('\n') {
        code.push('\n');
    }
    code.push_str("//# sourceMappingURL=data:application/json;base64,");
    code.push_str(&b64);
    code.push('\n');
}

/// Convert the first emitted diagnostic in `buf` into a `TranspileError`.
fn first_diagnostic(
    buf: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    _cm: &swc_core::common::sync::Lrc<swc_core::common::SourceMap>,
    file: &str,
) -> TranspileError {
    let data = buf.lock().unwrap();
    let text = String::from_utf8_lossy(&data).to_string();
    let (line, column, message) = parse_diag(&text);
    TranspileError {
        file: file.to_string(),
        line,
        column,
        message,
    }
}

/// Extract `line`, `column`, and a trimmed message from a swc diagnostic blob.
fn parse_diag(s: &str) -> (u32, u32, String) {
    let mut line: u32 = 1;
    let mut column: u32 = 1;
    let mut message = s.trim().to_string();
    for raw in s.lines() {
        if let Some(captures) = find_line_col(raw) {
            line = captures.0;
            column = captures.1;
            break;
        }
    }
    if let Some(first) = s.lines().next() {
        message = first.trim().to_string();
    }
    (line, column, message)
}

/// Look for a `N:M` or `:N:M:` style line/column pair within a string.
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

struct ErrorWriter {
    buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl std::io::Write for ErrorWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.buf.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct EmitterAdapter {
    cm: swc_core::common::sync::Lrc<swc_core::common::SourceMap>,
    writer: ErrorWriter,
}

impl swc_core::common::errors::Emitter for EmitterAdapter {
    fn emit(&mut self, db: &mut swc_core::common::errors::DiagnosticBuilder<'_>) {
        let mut msg = String::new();
        for m in &db.message {
            msg.push_str(&m.0);
        }
        let span = db.span.primary_span();
        let (line, column) = match span {
            Some(s) if !s.is_dummy() => {
                let pos = self.cm.lookup_char_pos(s.lo());
                (pos.line, pos.col_display + 1)
            }
            _ => (1, 1),
        };
        use std::io::Write as _;
        let _ = writeln!(self.writer, "{line}:{column}: {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(filename: &str) -> TranspileOptions<'_> {
        TranspileOptions {
            filename,
            inline_source_map: false,
            emit_source_map: false,
        }
    }

    #[test]
    fn strips_simple_type_annotations() {
        let out = transpile_typescript("const x: number = 1;", &opts("a.ts")).expect("ok");
        assert!(
            !out.code.contains(": number"),
            "type annotation not stripped: {}",
            out.code
        );
        assert!(out.code.contains("const x"));
        assert!(out.code.contains("= 1"));
    }

    #[test]
    fn strips_interface_and_type_alias() {
        let src = r#"interface I { x: number; }
type T = number;
export const v = 1;
"#;
        let out = transpile_typescript(src, &opts("a.ts")).expect("ok");
        assert!(
            !out.code.contains("interface"),
            "interface kept: {}",
            out.code
        );
        assert!(
            !out.code.contains("type T"),
            "type alias kept: {}",
            out.code
        );
    }

    #[test]
    fn default_export_function_is_preserved() {
        let src = "export default function f() {}";
        let out = transpile_typescript(src, &opts("a.ts")).expect("ok");
        assert!(
            out.code.contains("export default function f"),
            "default export function lost: {}",
            out.code
        );
    }

    #[test]
    fn inline_source_map_appended_when_requested() {
        let opts = TranspileOptions {
            filename: "a.ts",
            inline_source_map: true,
            emit_source_map: false,
        };
        let out = transpile_typescript("const x: number = 1;", &opts).expect("ok");
        assert!(
            out.code
                .contains("//# sourceMappingURL=data:application/json;base64,"),
            "inline source map missing: {}",
            out.code
        );
    }

    #[test]
    fn external_source_map_returned_when_requested() {
        let opts = TranspileOptions {
            filename: "a.ts",
            inline_source_map: false,
            emit_source_map: true,
        };
        let out = transpile_typescript("const x: number = 1;", &opts).expect("ok");
        let json = out.source_map.expect("source_map should be Some");
        assert!(
            json.contains(r#""version":3"#) || json.contains(r#""version": 3"#),
            "source map missing version: {json}"
        );
    }

    #[test]
    fn parse_error_returns_structured_error() {
        let src = "const x: = ;";
        let err = transpile_typescript(src, &opts("bad.ts")).expect_err("expected parse error");
        assert!(err.line >= 1, "line should be >= 1, got {}", err.line);
        assert!(err.column >= 1, "column should be >= 1, got {}", err.column);
        assert!(!err.message.is_empty(), "message empty");
    }

    #[test]
    fn decorator_syntax_is_a_parse_error() {
        let src = "@foo class C {}";
        let err = transpile_typescript(src, &opts("dec.ts"))
            .expect_err("expected error because decorators are disabled");
        assert!(!err.message.is_empty(), "message empty");
    }

    #[test]
    fn parse_module_returns_ast_and_source_map() {
        use swc_core::common::Spanned as _;
        let parsed = parse_module("const x: number = 1;", "a.ts").expect("ok");
        assert_eq!(
            parsed.module.body.len(),
            1,
            "expected exactly one top-level item"
        );
        let span = parsed.module.body[0].span();
        let pos = parsed.source_map.lookup_char_pos(span.lo);
        assert_eq!(pos.line, 1, "expected first item on line 1");
    }

    #[test]
    fn parse_module_surfaces_parse_error_with_position() {
        let err = match parse_module("const x: = ;", "bad.ts") {
            Ok(_) => panic!("expected parse error"),
            Err(e) => e,
        };
        assert!(err.line >= 1, "expected line >= 1, got {}", err.line);
        assert!(!err.message.is_empty(), "message empty");
    }
}
