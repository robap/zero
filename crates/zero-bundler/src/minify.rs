//! JS minifier wrapper around `swc_ecma_minifier`.

use swc_core::common::{
    BytePos, FileName, GLOBALS, Globals, LineCol, Mark, SourceMap,
    comments::SingleThreadedComments, source_map::DefaultSourceMapGenConfig, sync::Lrc,
};
use swc_core::ecma::{
    ast::{EsVersion, Program},
    codegen::{Config as CodegenConfig, Emitter, text_writer::JsWriter},
    minifier::{
        optimize,
        option::{CompressOptions, ExtraOptions, MangleOptions, MinifyOptions},
    },
    parser::{Parser, StringInput, Syntax, lexer::Lexer},
    transforms::base::{fixer::fixer, resolver},
};

/// Result of a minify pass: minified code + an optional v3 source map JSON.
#[derive(Debug)]
pub struct MinifyOutput {
    /// The minified JavaScript source.
    pub code: String,
    /// Composed source-map JSON. `Some` iff `emit_source_map` was true.
    pub source_map: Option<String>,
}

/// Names that must survive mangling — the CJS-shim contract.
const RESERVED_NAMES: &[&str] = &[
    "__zero_modules",
    "__zero_cache",
    "__zero_define",
    "__zero_require",
    "exports",
    "module",
    "default",
];

/// Minify a JS bundle string.
///
/// When `bundle_source_map` is provided AND `emit_source_map` is true, the
/// returned map is composed (bundle→original then minified→bundle, yielding
/// minified→original). When `bundle_source_map` is `None` but
/// `emit_source_map` is true, the raw minify map (with `sources = ["<bundle>"]`)
/// is returned. When `emit_source_map` is false, no map is produced and the
/// minifier runs without tracking positions.
pub fn minify_js(
    code: &str,
    bundle_source_map: Option<&str>,
    emit_source_map: bool,
) -> anyhow::Result<MinifyOutput> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom("<bundle>".into())),
        code.to_string(),
    );

    let comments = SingleThreadedComments::default();
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        EsVersion::EsNext,
        StringInput::from(&*fm),
        Some(&comments),
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|e| anyhow::anyhow!("minify parse failed: {:?}", e.kind()))?;

    let (code_out, srcmap_buf) = GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        let mut program = Program::Module(module);
        program.mutate(resolver(unresolved_mark, top_level_mark, false));

        let mangle = MangleOptions {
            reserved: RESERVED_NAMES.iter().map(|s| (*s).into()).collect(),
            ..MangleOptions::default()
        };
        let options = MinifyOptions {
            compress: Some(CompressOptions::default()),
            mangle: Some(mangle),
            ..MinifyOptions::default()
        };
        let extra = ExtraOptions {
            unresolved_mark,
            top_level_mark,
            mangle_name_cache: None,
        };
        let mut program = optimize(program, cm.clone(), Some(&comments), None, &options, &extra);
        program.mutate(fixer(Some(&comments)));

        filter_legal_comments(&comments);

        emit_program(&program, &cm, &comments, emit_source_map)
    })?;

    let source_map = if emit_source_map {
        Some(build_composed_source_map(
            &cm,
            &srcmap_buf,
            bundle_source_map,
        )?)
    } else {
        None
    };

    Ok(MinifyOutput {
        code: code_out,
        source_map,
    })
}

/// Strip every comment that is not a "legal" comment (`/*! ... */` or one
/// containing `@license`, `@preserve`, `@lic`, `@cc_on`). Mutates the shared
/// comments map in place.
fn filter_legal_comments(comments: &SingleThreadedComments) {
    let (mut leading, mut trailing) = comments.borrow_all_mut();
    for (_, list) in leading.iter_mut() {
        list.retain(|c| is_legal_comment(&c.text));
    }
    for (_, list) in trailing.iter_mut() {
        list.retain(|c| is_legal_comment(&c.text));
    }
}

/// True iff `text` looks like a license / preserve banner.
fn is_legal_comment(text: &str) -> bool {
    if text.starts_with('!') {
        return true;
    }
    let lower = text.to_ascii_lowercase();
    lower.contains("@license")
        || lower.contains("@preserve")
        || lower.contains("@lic")
        || lower.contains("@cc_on")
}

/// Emit `program` as minified JS. Returns the code and the position buffer
/// (only populated when `emit_source_map` is true).
fn emit_program(
    program: &Program,
    cm: &Lrc<SourceMap>,
    comments: &SingleThreadedComments,
    emit_source_map: bool,
) -> anyhow::Result<(String, Vec<(BytePos, LineCol)>)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut srcmap_buf: Vec<(BytePos, LineCol)> = Vec::new();
    {
        let writer_srcmap = if emit_source_map {
            Some(&mut srcmap_buf)
        } else {
            None
        };
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, writer_srcmap);
        let mut emitter = Emitter {
            cfg: CodegenConfig::default().with_minify(true),
            cm: cm.clone(),
            comments: Some(comments),
            wr: writer,
        };
        match program {
            Program::Module(m) => emitter
                .emit_module(m)
                .map_err(|e| anyhow::anyhow!("minify codegen failed: {e}"))?,
            Program::Script(s) => emitter
                .emit_script(s)
                .map_err(|e| anyhow::anyhow!("minify codegen failed: {e}"))?,
        }
    }
    let code =
        String::from_utf8(buf).map_err(|e| anyhow::anyhow!("minified output not UTF-8: {e}"))?;
    Ok((code, srcmap_buf))
}

/// Serialize the minify→bundle map, optionally composing it with the supplied
/// bundle→original map to yield a single minified→original map.
fn build_composed_source_map(
    cm: &Lrc<SourceMap>,
    srcmap_buf: &[(BytePos, LineCol)],
    bundle_source_map: Option<&str>,
) -> anyhow::Result<String> {
    let minify_map = cm.build_source_map(srcmap_buf, None, DefaultSourceMapGenConfig);
    let mut minify_buf: Vec<u8> = Vec::new();
    minify_map
        .to_writer(&mut minify_buf)
        .map_err(|e| anyhow::anyhow!("minify sourcemap serialization failed: {e}"))?;
    let minify_json = String::from_utf8(minify_buf)
        .map_err(|e| anyhow::anyhow!("minify sourcemap not UTF-8: {e}"))?;

    let Some(bundle_json) = bundle_source_map else {
        return Ok(minify_json);
    };

    let bundle_sm = sourcemap::SourceMap::from_reader(bundle_json.as_bytes())
        .map_err(|e| anyhow::anyhow!("invalid bundle sourcemap: {e}"))?;
    let minify_sm = sourcemap::SourceMap::from_reader(minify_json.as_bytes())
        .map_err(|e| anyhow::anyhow!("invalid minify sourcemap: {e}"))?;

    let mut builder = sourcemap::SourceMapBuilder::new(None);
    let mut source_indices: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for token in minify_sm.tokens() {
        // For each minify token, look up the bundle map at its src position.
        let bundle_token = match bundle_sm.lookup_token(token.get_src_line(), token.get_src_col()) {
            Some(t) => t,
            None => continue,
        };
        let Some(source) = bundle_token.get_source() else {
            continue;
        };
        let src_id = match source_indices.get(source) {
            Some(id) => *id,
            None => {
                let id = builder.add_source(source);
                source_indices.insert(source.to_string(), id);
                id
            }
        };
        builder.add_raw(
            token.get_dst_line(),
            token.get_dst_col(),
            bundle_token.get_src_line(),
            bundle_token.get_src_col(),
            Some(src_id),
            None,
            false,
        );
    }
    let composed = builder.into_sourcemap();
    let mut buf: Vec<u8> = Vec::new();
    composed
        .to_writer(&mut buf)
        .map_err(|e| anyhow::anyhow!("composed sourcemap serialization failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| anyhow::anyhow!("composed sourcemap not UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_with_bundle_source_map() {
        // Hand-build a bundle source map: line 0 of the bundle maps to
        // `./fake/app.ts`.
        let mut builder = sourcemap::SourceMapBuilder::new(None);
        let src_id = builder.add_source("./fake/app.ts");
        builder.add_raw(0, 0, 0, 0, Some(src_id), None, false);
        let bundle_sm = builder.into_sourcemap();
        let mut buf: Vec<u8> = Vec::new();
        bundle_sm.to_writer(&mut buf).unwrap();
        let bundle_json = String::from_utf8(buf).unwrap();

        let out = minify_js("const x = 1; x;\n", Some(&bundle_json), true).expect("minify ok");
        let composed_json = out.source_map.expect("composed source_map");
        let composed =
            sourcemap::SourceMap::from_reader(composed_json.as_bytes()).expect("composed parses");
        let mut saw = false;
        for token in composed.tokens() {
            if let Some(src) = token.get_source()
                && src == "./fake/app.ts"
            {
                saw = true;
                break;
            }
        }
        assert!(
            saw,
            "composed map does not point back to ./fake/app.ts: {composed_json}"
        );
    }

    #[test]
    fn source_map_none_when_not_requested() {
        let out = minify_js("const x = 1; x;", None, false).expect("minify ok");
        assert!(out.source_map.is_none());
    }

    #[test]
    fn source_map_request_returns_some() {
        let out = minify_js("const x = 1; x;", None, true).expect("minify ok");
        assert!(
            out.source_map.is_some(),
            "expected source_map when emit_source_map=true"
        );
    }

    #[test]
    fn preserves_legal_comments() {
        let input = "/*! @license MIT */ const x = 1; /* regular */ console.log(x);";
        let out = minify_js(input, None, false).expect("minify ok");
        assert!(
            out.code.contains("@license"),
            "legal comment was dropped: {}",
            out.code
        );
        assert!(
            !out.code.contains("regular"),
            "regular comment was retained: {}",
            out.code
        );
    }

    #[test]
    fn does_not_mangle_property_names() {
        let input = "const o = { someProperty: 1 }; console.log(o['someProperty']);";
        let out = minify_js(input, None, false).expect("minify ok");
        assert!(
            out.code.contains("someProperty"),
            "property name was mangled: {}",
            out.code
        );
    }

    #[test]
    fn preserves_reserved_names() {
        let input = "__zero_define('x', function(exports, __zero_require) { exports.y = 1; }); \
                     __zero_require('x');";
        let out = minify_js(input, None, false).expect("minify ok");
        for name in ["__zero_define", "__zero_require", "exports"] {
            assert!(
                out.code.contains(name),
                "reserved name {name} missing from minified output: {}",
                out.code
            );
        }
    }

    #[test]
    fn minifies_simple_function() {
        let input = "function add(a, b) { return a + b; } console.log(add(1, 2));\n";
        let out = minify_js(input, None, false).expect("minify ok");
        assert!(
            out.code.len() <= (input.len() as f64 * 0.90) as usize,
            "expected minified to be <= 90% of input length; got {} vs {}",
            out.code.len(),
            input.len()
        );
        assert!(
            !out.code.contains("\n  "),
            "minified output still has indented newlines: {}",
            out.code
        );
    }
}
