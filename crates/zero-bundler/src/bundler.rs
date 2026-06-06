//! Module graph walker and CommonJS-style bundle emitter.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use regex::Regex;

use zero_config::Config;
use zero_runtime::{ZERO_HTTP_BODY, ZERO_HTTP_EXPORTS, ZERO_RUNTIME_BODY, ZERO_RUNTIME_EXPORTS};
use zero_transpile::{TranspileOptions, transpile_typescript};

use crate::resolver::{ModuleId, resolve};

/// Bundle product: the JS source and optional sourcemap JSON.
#[derive(Debug)]
pub struct BundleOutput {
    /// Bundled JS code.
    pub code: String,
    /// JSON-encoded v3 sourcemap. `Some` iff `emit_sourcemap` was true.
    pub source_map: Option<String>,
}

/// CommonJS preamble injected at the top of every bundle.
const PREAMBLE: &str = r#"const __zero_modules = {};
const __zero_cache = {};
function __zero_define(id, factory) { __zero_modules[id] = factory; }
function __zero_require(id) {
  if (__zero_cache[id]) return __zero_cache[id].exports;
  const mod = { exports: {} };
  __zero_cache[id] = mod;
  __zero_modules[id](mod.exports, __zero_require);
  return mod.exports;
}
"#;

/// Produce a single bundled JS string from the project root's entry module.
///
/// # Parameters
/// - `config`: the validated `zero.toml` configuration.
/// - `emit_sourcemap`: when `true`, build a v3 sourcemap alongside the bundle.
///
/// # Returns
/// `BundleOutput { code, source_map }`. `source_map` is `Some` iff `emit_sourcemap`.
pub fn bundle(config: &Config, emit_sourcemap: bool) -> anyhow::Result<BundleOutput> {
    let unmin = bundle_unminified_inner(config, emit_sourcemap)?;
    let crate::minify::MinifyOutput {
        code,
        source_map: composed_map,
    } = crate::minify::minify_js(&unmin.code, unmin.source_map.as_deref(), emit_sourcemap)?;
    Ok(BundleOutput {
        code,
        source_map: composed_map,
    })
}

/// Test-only entry point that produces the un-minified intermediate bundle —
/// used by the showcase size-budget integration test to compare minified vs.
/// un-minified bytes. Gated so it never appears in release builds of the
/// `zero` crate's `[dependencies]`.
#[cfg(any(test, feature = "test-internals"))]
pub fn bundle_unminified(config: &Config, emit_sourcemap: bool) -> anyhow::Result<BundleOutput> {
    bundle_unminified_inner(config, emit_sourcemap)
}

/// Produce the un-minified intermediate bundle that `bundle()` feeds to the
/// minifier. Returned `source_map` is the bundle→original map (when
/// `emit_sourcemap`). Used by `bundle()` and the `bundle_unminified`
/// test-only entry point.
fn bundle_unminified_inner(config: &Config, emit_sourcemap: bool) -> anyhow::Result<BundleOutput> {
    let (root, entry_path, entry_id) = resolve_entry(config)?;
    let (order, sources) = collect_module_graph(&root, entry_id.clone(), entry_path)?;
    let emit_order = sort_emit_order(order);
    let mut out = String::from(PREAMBLE);
    let factory_spans = emit_factories(&mut out, &emit_order, &sources, &root)?;
    let bootstrap_id = match &entry_id {
        ModuleId::User(rel) => rel.to_string_lossy().into_owned(),
        ModuleId::Runtime | ModuleId::Http => unreachable!(),
    };
    out.push_str(&format!("\n__zero_require('{bootstrap_id}');\n"));
    let source_map = if emit_sourcemap {
        Some(build_bundle_source_map(&factory_spans)?)
    } else {
        None
    };
    Ok(BundleOutput {
        code: out,
        source_map,
    })
}

/// Resolve the project root and entry module from `config`. Errors if both
/// `src/app.ts` and `src/app.js` exist, or neither.
fn resolve_entry(config: &Config) -> anyhow::Result<(PathBuf, PathBuf, ModuleId)> {
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root).canonicalize()?;
    let entry_ts = root.join("src").join("app.ts");
    let entry_js = root.join("src").join("app.js");
    let (entry_path, entry_id) = match (entry_ts.is_file(), entry_js.is_file()) {
        (true, true) => {
            anyhow::bail!("zero build: both src/app.ts and src/app.js exist; remove one")
        }
        (true, false) => (entry_ts, ModuleId::User(PathBuf::from("./src/app.ts"))),
        (false, true) => (entry_js, ModuleId::User(PathBuf::from("./src/app.js"))),
        (false, false) => anyhow::bail!("zero build: no entry point at src/app.ts or src/app.js"),
    };
    Ok((root, entry_path, entry_id))
}

/// BFS the import graph starting at `entry_id`, returning the post-order
/// visit list and the source text for every reachable module.
fn collect_module_graph(
    root: &Path,
    entry_id: ModuleId,
    entry_path: PathBuf,
) -> anyhow::Result<(Vec<ModuleId>, HashMap<ModuleId, String>)> {
    let mut sources: HashMap<ModuleId, String> = HashMap::new();
    sources.insert(ModuleId::Runtime, ZERO_RUNTIME_BODY.to_string());
    sources.insert(ModuleId::Http, ZERO_HTTP_BODY.to_string());
    let mut order: Vec<ModuleId> = Vec::new();
    let mut visited: HashSet<ModuleId> = HashSet::new();
    let mut queue: VecDeque<(ModuleId, PathBuf)> = VecDeque::new();
    queue.push_back((entry_id, entry_path));

    while let Some((id, path)) = queue.pop_front() {
        if visited.contains(&id) {
            continue;
        }
        visited.insert(id.clone());

        let src = load_module_source(&id, &path)?;
        sources.insert(id.clone(), src.clone());

        let importer_dir = if matches!(id, ModuleId::Runtime | ModuleId::Http) {
            root.to_path_buf()
        } else {
            path.parent().unwrap_or(root).to_path_buf()
        };

        enqueue_dependencies(&src, &importer_dir, root, &visited, &mut queue)?;
        order.push(id);
    }
    Ok((order, sources))
}

/// Read (and transpile, for `.ts`) the source for a single module.
fn load_module_source(id: &ModuleId, path: &Path) -> anyhow::Result<String> {
    match id {
        ModuleId::Runtime => Ok(ZERO_RUNTIME_BODY.to_string()),
        ModuleId::Http => Ok(ZERO_HTTP_BODY.to_string()),
        ModuleId::User(rel) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
            if rel.extension().and_then(|e| e.to_str()) == Some("ts") {
                let logical = rel.to_string_lossy().into_owned();
                let out = transpile_typescript(
                    &raw,
                    &TranspileOptions {
                        filename: &logical,
                        inline_source_map: false,
                        emit_source_map: false,
                    },
                )
                .map_err(|e| anyhow::anyhow!("transpile failed for {logical}: {e}"))?;
                Ok(out.code)
            } else {
                Ok(raw)
            }
        }
    }
}

/// Resolve every `import` specifier in `src` and enqueue any unvisited
/// dependency for later processing.
fn enqueue_dependencies(
    src: &str,
    importer_dir: &Path,
    root: &Path,
    visited: &HashSet<ModuleId>,
    queue: &mut VecDeque<(ModuleId, PathBuf)>,
) -> anyhow::Result<()> {
    for specifier in extract_imports(src) {
        let dep_id = resolve(&specifier, importer_dir, root)?;
        if visited.contains(&dep_id) {
            continue;
        }
        match &dep_id {
            ModuleId::Runtime | ModuleId::Http => {
                queue.push_front((dep_id, PathBuf::new()));
            }
            ModuleId::User(rel) => {
                let dep_path = root.join(rel.strip_prefix("./").unwrap_or(rel));
                queue.push_front((dep_id, dep_path));
            }
        }
    }
    Ok(())
}

/// Move synthetic `Runtime` and `Http` modules to the front of the emit order
/// so they are defined before any user module references them.
fn sort_emit_order(order: Vec<ModuleId>) -> Vec<ModuleId> {
    let mut emit_order = order;
    if emit_order.contains(&ModuleId::Http) {
        emit_order.retain(|id| *id != ModuleId::Http);
        emit_order.insert(0, ModuleId::Http);
    }
    if emit_order.contains(&ModuleId::Runtime) {
        emit_order.retain(|id| *id != ModuleId::Runtime);
        emit_order.insert(0, ModuleId::Runtime);
    }
    emit_order
}

/// Append a `__zero_define(...)` block to `out` for each module in
/// `emit_order`, in order. Returns a span per module recording the inclusive-
/// first and exclusive-last bundle line indices of its factory body.
fn emit_factories(
    out: &mut String,
    emit_order: &[ModuleId],
    sources: &HashMap<ModuleId, String>,
    root: &Path,
) -> anyhow::Result<Vec<(ModuleId, usize, usize)>> {
    let mut spans: Vec<(ModuleId, usize, usize)> = Vec::with_capacity(emit_order.len());
    for id in emit_order {
        let module_id_str = module_id_string(id);
        let raw_src = sources.get(id).map(|s| s.as_str()).unwrap_or("");
        let factory_body = rewrite_module(raw_src, root, id, sources)?;
        out.push_str(&format!(
            "\n__zero_define({module_id_str}, function(exports, __zero_require) {{\n"
        ));
        let body_first = count_lines(out);
        out.push_str(&factory_body);
        out.push('\n');
        let body_last_exclusive = count_lines(out);
        out.push_str("});\n");
        spans.push((id.clone(), body_first, body_last_exclusive));
    }
    Ok(spans)
}

/// Number of bundle lines emitted so far (0-based line index for the next char).
fn count_lines(s: &str) -> usize {
    s.bytes().filter(|b| *b == b'\n').count()
}

/// Build a v3 sourcemap from bundle line spans to original sources.
///
/// For each user module's factory-body line range, emits one mapping per line
/// pointing at the source file's line 0, column 0. Column is always 0 and
/// intra-module line offset is collapsed to 0 — enough for stack-trace tooling
/// to point at the right file, not for column-accurate debugging.
/// Lines outside any user span (PREAMBLE, wrapper boilerplate, synthetic
/// runtime/http factories) get no mapping.
fn build_bundle_source_map(factory_spans: &[(ModuleId, usize, usize)]) -> anyhow::Result<String> {
    let mut builder = sourcemap::SourceMapBuilder::new(None);
    for (id, first, last_exclusive) in factory_spans {
        let ModuleId::User(rel) = id else { continue };
        let src_id = builder.add_source(&rel.to_string_lossy());
        for dst_line in *first..*last_exclusive {
            builder.add_raw(dst_line as u32, 0, 0, 0, Some(src_id), None, false);
        }
    }
    let sm = builder.into_sourcemap();
    let mut buf: Vec<u8> = Vec::new();
    sm.to_writer(&mut buf)
        .map_err(|e| anyhow::anyhow!("sourcemap serialization failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| anyhow::anyhow!("sourcemap not UTF-8: {e}"))
}

/// The string key used in `__zero_define` / `__zero_require` calls.
fn module_id_string(id: &ModuleId) -> String {
    match id {
        ModuleId::Runtime => "'zero'".to_string(),
        ModuleId::Http => "'zero/http'".to_string(),
        ModuleId::User(rel) => format!("'{}'", rel.to_str().unwrap_or("?")),
    }
}

/// Rewrite ES module source into a CJS factory body.
fn rewrite_module(
    src: &str,
    root: &Path,
    id: &ModuleId,
    _sources: &HashMap<ModuleId, String>,
) -> anyhow::Result<String> {
    if id == &ModuleId::Runtime {
        return rewrite_runtime_exports(src);
    }
    if id == &ModuleId::Http {
        return rewrite_http_exports(src);
    }

    let normalized = normalize_for_rewrite(src);
    let out = rewrite_imports(&normalized);
    let out = rewrite_reexport_from(out);
    let out = rewrite_exports(out, &normalized);
    let out = resolve_relative_requires(out, root, id);
    Ok(out)
}

/// Insert a newline before `import` / `export` keywords that the SWC codegen
/// has placed on the same line as a preceding `*/` block-comment close. Lets
/// the line-anchored regex rewrites match in the presence of preserved
/// leading comments.
fn normalize_for_rewrite(src: &str) -> String {
    let re = Regex::new(r#"\*/[ \t]+(import|export)\b"#).unwrap();
    re.replace_all(src, "*/\n$1").into_owned()
}

/// Translate `{ a, b as c }` (an ES module named-binding list) into a JS
/// destructuring pattern: `a, b: c`.
fn named_to_destructure(inner: &str) -> String {
    let mut parts = Vec::new();
    for raw in inner.replace('\n', " ").split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let toks: Vec<&str> = raw.split_whitespace().collect();
        match toks.as_slice() {
            [orig, "as", alias] => parts.push(format!("{orig}: {alias}")),
            [name] => parts.push((*name).to_string()),
            _ => parts.push(raw.to_string()),
        }
    }
    parts.join(", ")
}

/// Rewrite `export { … } from "<spec>";` (and `export * from`) re-exports
/// into CJS factory body lines. Runs *before* `rewrite_exports` so the
/// trailing `from "<spec>"` is consumed alongside its `export { … }` head.
fn rewrite_reexport_from(src: String) -> String {
    let re_named =
        Regex::new(r#"(?ms)^[ \t]*export\s*\{\s*([^}]+?)\s*\}\s*from\s+['"]([^'"]+)['"]\s*;?"#)
            .unwrap();
    let mut out = re_named
        .replace_all(&src, |caps: &regex::Captures<'_>| {
            rewrite_reexport_named(&caps[1], &caps[2])
        })
        .into_owned();
    let re_all = Regex::new(r#"(?m)^[ \t]*export\s*\*\s*from\s+['"]([^'"]+)['"]\s*;?"#).unwrap();
    out = re_all
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let spec = &caps[1];
            let id = next_reexport_id();
            format!(
                "const {id} = __zero_require('{spec}'); for (const __k in {id}) {{ exports[__k] = {id}[__k]; }}"
            )
        })
        .into_owned();
    out
}

/// Build the body of `export { a, b as c } from '<spec>'`: pull the module
/// via `__zero_require` and reassign each name onto `exports`.
fn rewrite_reexport_named(inner: &str, spec: &str) -> String {
    let id = next_reexport_id();
    let mut lines = format!("const {id} = __zero_require('{spec}');");
    for raw in inner.split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let parts: Vec<&str> = raw.split_whitespace().collect();
        match parts.as_slice() {
            [orig, "as", alias] => {
                lines.push_str(&format!("\nexports.{alias} = {id}.{orig};"));
            }
            [name] => {
                lines.push_str(&format!("\nexports.{name} = {id}.{name};"));
            }
            _ => {}
        }
    }
    lines
}

/// Generate a unique local-binding name for each rewritten re-export so the
/// bundler doesn't collide when the same factory body has multiple
/// `export … from` lines.
fn next_reexport_id() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("__zero_reexport_{n}")
}

/// Convert every ES import form into a `const … = __zero_require(...)`
/// equivalent.
fn rewrite_imports(src: &str) -> String {
    let single_line_import =
        Regex::new(r#"(?m)^[ \t]*import\s+\{([^}]+)\}\s*from\s+['"]([^'"]+)['"]\s*;?"#).unwrap();
    let combined_import =
        Regex::new(r#"(?m)^[ \t]*import\s+(\w+)\s*,\s*\{([^}]+)\}\s*from\s+['"]([^'"]+)['"]\s*;?"#)
            .unwrap();
    let default_import =
        Regex::new(r#"(?m)^[ \t]*import\s+(\w+)\s+from\s+['"]([^'"]+)['"]\s*;?"#).unwrap();
    let namespace_import =
        Regex::new(r#"(?m)^[ \t]*import\s+\*\s+as\s+(\w+)\s+from\s+['"]([^'"]+)['"]\s*;?"#)
            .unwrap();
    let side_effect_import = Regex::new(r#"(?m)^[ \t]*import\s+['"]([^'"]+)['"]\s*;?"#).unwrap();
    let multi_line_import =
        Regex::new(r#"(?ms)^[ \t]*import\s*\{[^}]*\}\s*from\s+['"][^'"]+['"]\s*;?"#).unwrap();
    let multi_single =
        Regex::new(r#"import\s*\{([^}]+)\}\s*from\s+['"]([^'"]+)['"]\s*;?"#).unwrap();

    let mut out = src.to_string();

    let multi_matches: Vec<_> = multi_line_import
        .find_iter(&out)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect();
    let mut offset_diff: i64 = 0;
    for (start, end, matched) in multi_matches {
        let start_adj = (start as i64 + offset_diff) as usize;
        let end_adj = (end as i64 + offset_diff) as usize;
        let replacement = if let Some(cap) = multi_single.captures(&matched) {
            let names = named_to_destructure(&cap[1]);
            let spec = cap[2].to_string();
            format!("const {{{}}} = __zero_require('{}');", names, spec)
        } else {
            String::new()
        };
        let old_len = end_adj - start_adj;
        out.replace_range(start_adj..end_adj, &replacement);
        offset_diff += replacement.len() as i64 - old_len as i64;
    }

    out = combined_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!(
                "const {{ default: {}, {} }} = __zero_require('{}');",
                &caps[1],
                named_to_destructure(&caps[2]),
                &caps[3]
            )
        })
        .into_owned();
    out = single_line_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!(
                "const {{{}}} = __zero_require('{}');",
                named_to_destructure(&caps[1]),
                &caps[2]
            )
        })
        .into_owned();
    out = namespace_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("const {} = __zero_require('{}');", &caps[1], &caps[2])
        })
        .into_owned();
    out = default_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!(
                "const {} = __zero_require('{}').default;",
                &caps[1], &caps[2]
            )
        })
        .into_owned();
    out = side_effect_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("__zero_require('{}');", &caps[1])
        })
        .into_owned();

    out
}

/// Convert every ES export form into CJS `exports.x = x;` assignments and
/// strip the `export` keyword from the original declarations.
fn rewrite_exports(mut out: String, src: &str) -> String {
    // Capture group 1 of the fn regexes is an optional `async ` modifier
    // (with trailing space) so it round-trips into the replacement; group 2
    // is the function name.
    let export_default_fn =
        Regex::new(r"(?m)^export\s+default\s+(async\s+)?function\s+(\w+)").unwrap();
    let export_default_val = Regex::new(r"(?m)^export\s+default\s+").unwrap();
    let export_named_fn = Regex::new(r"(?m)^export\s+(async\s+)?function\s+(\w+)").unwrap();
    let export_named_const = Regex::new(r"(?m)^export\s+(const|let|var)\s+(\w+)").unwrap();
    let export_named_class = Regex::new(r"(?m)^export\s+class\s+(\w+)").unwrap();
    let export_block = Regex::new(r"(?ms)^export\s*\{\s*([^}]+?)\s*\}\s*;?").unwrap();

    out = export_default_fn
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let async_kw = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("{async_kw}function {}", &caps[2])
        })
        .into_owned();
    let default_fn_names: Vec<String> = export_default_fn
        .captures_iter(src)
        .map(|c| c[2].to_string())
        .collect();
    for name in &default_fn_names {
        out.push_str(&format!("\nexports.default = {name};\n"));
    }

    if default_fn_names.is_empty() {
        out = export_default_val
            .replace_all(&out, "exports.default = ")
            .into_owned();
    }

    let named_fn_names: Vec<String> = export_named_fn
        .captures_iter(&out.clone())
        .map(|c| c[2].to_string())
        .collect();
    out = export_named_fn
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let async_kw = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("{async_kw}function {}", &caps[2])
        })
        .into_owned();
    for name in &named_fn_names {
        out.push_str(&format!("\nexports.{name} = {name};\n"));
    }

    let named_class_names: Vec<String> = export_named_class
        .captures_iter(&out.clone())
        .map(|c| c[1].to_string())
        .collect();
    out = export_named_class
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("class {}", &caps[1])
        })
        .into_owned();
    for name in &named_class_names {
        out.push_str(&format!("\nexports.{name} = {name};\n"));
    }

    let named_const_names: Vec<(String, String)> = export_named_const
        .captures_iter(&out.clone())
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect();
    out = export_named_const
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("{} {}", &caps[1], &caps[2])
        })
        .into_owned();
    for (_kw, name) in &named_const_names {
        out.push_str(&format!("\nexports.{name} = {name};\n"));
    }

    export_block
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            rewrite_export_block(&caps[1])
        })
        .into_owned()
}

/// Translate the body of an `export { a, b as c }` aggregate into one
/// `exports.<alias> = <orig>;` line per specifier.
fn rewrite_export_block(inner: &str) -> String {
    let mut lines = String::new();
    for spec in inner.split(',') {
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        let parts: Vec<&str> = spec.split_whitespace().collect();
        match parts.as_slice() {
            [orig, "as", alias] => {
                lines.push_str(&format!("exports.{alias} = {orig};\n"));
            }
            [name] => {
                lines.push_str(&format!("exports.{name} = {name};\n"));
            }
            _ => {}
        }
    }
    lines
}

/// Rewrite every `__zero_require('<spec>')` call to the resolved module ID
/// so the require key matches the `__zero_define` registration. Without this,
/// relative paths stay as `./…` (wrong key) and bare specifiers like
/// `'zero/components'` survive unrewritten despite registering under their
/// resolved path (`./.zero/components/index.ts`).
fn resolve_relative_requires(out: String, root: &Path, id: &ModuleId) -> String {
    let require_re = Regex::new(r#"__zero_require\('([^']+)'\)"#).unwrap();
    let importer_dir = if let ModuleId::User(rel) = id {
        let abs = root.join(rel.strip_prefix("./").unwrap_or(rel));
        abs.parent().unwrap_or(root).to_path_buf()
    } else {
        root.to_path_buf()
    };
    require_re
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let spec = &caps[1];
            match resolve(spec, &importer_dir, root) {
                Ok(ModuleId::Runtime) => "__zero_require('zero')".to_string(),
                Ok(ModuleId::Http) => "__zero_require('zero/http')".to_string(),
                Ok(ModuleId::User(rel)) => {
                    format!("__zero_require('{}')", rel.to_str().unwrap_or(spec))
                }
                Err(_) => format!("__zero_require('{spec}')"),
            }
        })
        .into_owned()
}

/// Rewrite the runtime body's final `export { ... }` aggregate into
/// `exports.x = x;` assignments for the CJS factory.
fn rewrite_runtime_exports(body: &str) -> anyhow::Result<String> {
    let mut out = body.to_string();
    for name in ZERO_RUNTIME_EXPORTS {
        out.push_str(&format!("\nexports.{name} = {name};\n"));
    }
    Ok(out)
}

/// Rewrite the http body for the CJS factory: append `exports.x = x;` for
/// each public name.
fn rewrite_http_exports(body: &str) -> anyhow::Result<String> {
    let mut out = body.to_string();
    for name in ZERO_HTTP_EXPORTS {
        out.push_str(&format!("\nexports.{name} = {name};\n"));
    }
    Ok(out)
}

/// Extract import (and re-export) specifiers from an ES module source file.
fn extract_imports(src: &str) -> Vec<String> {
    // The clause set must stay in lock-step with `rewrite_imports`. The
    // combined form `import Default, { … }` is listed first because the
    // looser `\w+` clause would otherwise match `Default` and then fail at
    // the comma, dropping the specifier on the floor.
    let import_re = Regex::new(
        r#"(?ms)import\s+(?:\w+\s*,\s*\{[^}]*\}|\{[^}]*\}|\*\s+as\s+\w+|\w+|\s*)\s*from\s+['"]([^'"]+)['"]|import\s+['"]([^'"]+)['"]"#,
    )
    .unwrap();
    let reexport_re =
        Regex::new(r#"(?ms)export\s+(?:\{[^}]*\}|\*(?:\s+as\s+\w+)?)\s*from\s+['"]([^'"]+)['"]"#)
            .unwrap();
    let mut out: Vec<String> = import_re
        .captures_iter(src)
        .filter_map(|c| c.get(1).or_else(|| c.get(2)))
        .map(|m| m.as_str().to_string())
        .collect();
    for c in reexport_re.captures_iter(src) {
        if let Some(m) = c.get(1) {
            out.push(m.as_str().to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_imports_finds_named_and_default() {
        let src = r#"
import { html } from "zero";
import Home from "./routes/home.js";
"#;
        let imports = extract_imports(src);
        assert!(imports.contains(&"zero".to_string()));
        assert!(imports.contains(&"./routes/home.js".to_string()));
    }

    #[test]
    fn rewrite_rewrites_async_named_export() {
        // `export async function foo` must lose the `export` keyword and gain
        // an `exports.foo = foo` line, just like a plain `export function`.
        // Without this, the keyword survives into the bundle and SWC's
        // minifier rejects the file with `ImportExportInScript`.
        let src = "export async function load() { return 1; }\n";
        let result = rewrite_module(
            src,
            Path::new("/root"),
            &ModuleId::User(PathBuf::from("./src/routes/home.ts")),
            &HashMap::new(),
        )
        .unwrap();
        assert!(
            !result.contains("export async function"),
            "export keyword survived: {result}"
        );
        assert!(
            result.contains("async function load"),
            "async modifier was dropped: {result}"
        );
        assert!(
            result.contains("exports.load = load"),
            "named export binding missing: {result}"
        );
    }

    #[test]
    fn extract_imports_finds_combined_default_and_named() {
        // `import Default, { named } from "<spec>"` — rewrite_imports handles
        // this form, so extract_imports must too, or the dependency walker
        // skips the file and the bundle defines no factory for it.
        let src = r#"
import Home, { load as loadHome } from "./routes/home.ts";
"#;
        let imports = extract_imports(src);
        assert!(
            imports.contains(&"./routes/home.ts".to_string()),
            "combined-import specifier missing: {imports:?}"
        );
    }

    #[test]
    fn rewrite_aliases_named_import_bindings() {
        // `import { a, b as c }` must become `const { a, b: c }` — the bare
        // `as` is invalid inside a destructuring pattern, and the minifier
        // rejects the bundle with `Expected(",", "as")` if it leaks through.
        // Exercises both the single-line and multi-line import forms, since
        // the multi-line regex pass consumes single-line imports too.
        for src in [
            r#"import { html, load as loadHome } from "./routes/home.ts";"#,
            "import {\n  html,\n  load as loadHome,\n} from \"./routes/home.ts\";\n",
        ] {
            let result = rewrite_module(
                src,
                Path::new("/root"),
                &ModuleId::User(PathBuf::from("./src/app.ts")),
                &HashMap::new(),
            )
            .unwrap();
            assert!(
                result.contains("load: loadHome"),
                "alias not destructured: {result}"
            );
            assert!(
                !result.contains(" as "),
                "`as` leaked into destructuring: {result}"
            );
        }
    }

    #[test]
    fn rewrite_rewrites_named_import() {
        let src = r#"import { App } from "zero";
const app = new App();
"#;
        let result = rewrite_module(
            src,
            Path::new("/root"),
            &ModuleId::User(PathBuf::from("./src/app.js")),
            &HashMap::new(),
        )
        .unwrap();
        assert!(
            result.contains("__zero_require('zero')"),
            "import not rewritten: {result}"
        );
    }

    /// Serialize CWD-mutating tests within this module.
    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn write_minimal_config(root: &Path) -> Config {
        let toml = format!(
            "[project]\nroot = \"{}\"\n",
            root.file_name().unwrap().to_string_lossy()
        );
        Config::from_toml_str(&toml).unwrap()
    }

    fn with_cwd<F, R>(dir: &Path, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        let out = f();
        // Best-effort restore; ignore error so test failures aren't masked.
        let _ = std::env::set_current_dir(&prev);
        drop(guard);
        out
    }

    #[test]
    fn bundle_with_ts_entry_strips_types_and_imports_zero() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "import { signal } from \"zero\";\nconst n: number = 1; signal(n);\n",
        )
        .unwrap();
        let result = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false));
        let bundled = result.unwrap().code;
        assert!(
            bundled.contains(r#"__zero_require("zero")"#),
            "missing zero require: {bundled}"
        );
        let app_section_start = bundled
            .find(r#"__zero_define("./src/app.ts""#)
            .expect("expected app.ts module section");
        let app_section = &bundled[app_section_start..];
        let app_section_end = app_section
            .find(r#"__zero_require("./src/app.ts")"#)
            .unwrap_or(app_section.len());
        let app_module = &app_section[..app_section_end];
        assert!(
            !app_module.contains(": number"),
            "type annotation leaked: {app_module}"
        );
        assert!(bundled.contains(r#"__zero_require("./src/app.ts")"#));
    }

    #[test]
    fn bundle_errors_when_both_entries_present() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/app.ts"), "").unwrap();
        std::fs::write(root.join("src/app.js"), "").unwrap();
        let result = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false));
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("both src/app.ts and src/app.js"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn bundle_emits_no_source_map_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/app.ts"), "const x: number = 1; x;\n").unwrap();
        let out = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false))
            .expect("bundle ok");
        assert!(out.source_map.is_none());
    }

    #[test]
    fn bundle_emits_source_map_when_requested() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/app.ts"), "const x: number = 1; x;\n").unwrap();
        let out =
            with_cwd(dir.path(), || bundle(&write_minimal_config(&root), true)).expect("bundle ok");
        let json = out.source_map.expect("source_map should be Some");
        assert!(
            json.contains(r#""version":3"#) || json.contains(r#""version": 3"#),
            "missing version: {json}"
        );
        assert!(
            json.contains("./src/app.ts"),
            "sources missing entry: {json}"
        );
        assert!(
            !json.contains(r#""mappings":"""#),
            "expected non-empty mappings; got empty: {json}"
        );
    }

    #[test]
    fn bundle_source_map_resolves_user_line() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/app.ts"), "const x: number = 1; x;\n").unwrap();
        let out =
            with_cwd(dir.path(), || bundle(&write_minimal_config(&root), true)).expect("bundle ok");
        let json = out.source_map.expect("source_map should be Some");
        let sm = sourcemap::SourceMap::from_reader(json.as_bytes()).expect("valid map");
        let mut saw_app_ts = false;
        for token in sm.tokens() {
            if let Some(src) = token.get_source()
                && src.ends_with("./src/app.ts")
            {
                saw_app_ts = true;
                break;
            }
        }
        assert!(
            saw_app_ts,
            "no token resolves to ./src/app.ts in map: {json}"
        );
    }

    #[test]
    fn bundle_evaluates_under_quickjs() {
        use rquickjs::CatchResultExt;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "let result = 0;\nfunction add(a: number, b: number) { return a + b; }\nresult = add(2, 3);\n(globalThis as any).result = result;\n",
        )
        .unwrap();
        let out = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false))
            .expect("bundle ok");
        let rt = rquickjs::Runtime::new().expect("runtime");
        let context = rquickjs::Context::full(&rt).expect("context");
        let n: i32 = context.with(|ctx| {
            ctx.eval::<(), _>(out.code.clone())
                .catch(&ctx)
                .expect("bundle evaluates without error");
            ctx.globals()
                .get::<_, i32>("result")
                .expect("global `result` is a number")
        });
        assert_eq!(n, 5, "expected result == 5");
    }

    #[test]
    fn bundle_preserves_legal_comments() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        // The legal comment is attached to a statement that survives DCE
        // (`console.log` has side effects), so it should appear in the output.
        std::fs::write(
            root.join("src/app.ts"),
            "/*! KEEP-ME license banner */\nconsole.log(\"hello\");\n/* drop-me */\nconsole.log(\"world\");\n",
        )
        .unwrap();
        let out = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false))
            .expect("bundle ok");
        assert!(
            out.code.contains("KEEP-ME"),
            "legal comment missing from bundle: {}",
            out.code
        );
        assert!(
            !out.code.contains("drop-me"),
            "regular comment retained in bundle: {}",
            out.code
        );
    }

    #[test]
    fn bundle_preserves_reserved_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "import { signal } from \"zero\";\nsignal(1);\n",
        )
        .unwrap();
        let out = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false))
            .expect("bundle ok");
        for name in [
            "__zero_define",
            "__zero_require",
            "__zero_modules",
            "__zero_cache",
        ] {
            assert!(
                out.code.contains(name),
                "reserved name {name} missing from minified bundle: {}",
                out.code
            );
        }
        assert!(
            out.code.contains("./src/app.ts"),
            "module id literal missing: {}",
            out.code
        );
    }

    #[test]
    fn bundle_unminified_is_larger_than_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "const verbose: number = 1;\nfunction longerName(x: number) {\n    return x + verbose;\n}\nconsole.log(longerName(2));\n",
        )
        .unwrap();
        let (mini, unmini) = with_cwd(dir.path(), || {
            let mini = bundle(&write_minimal_config(&root), false).unwrap();
            let unmini = bundle_unminified(&write_minimal_config(&root), false).unwrap();
            (mini, unmini)
        });
        assert!(
            unmini.code.len() > mini.code.len(),
            "un-minified ({}) should be strictly longer than minified ({})",
            unmini.code.len(),
            mini.code.len()
        );
    }

    #[test]
    fn bundle_is_minified() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "const verbose: number = 1;\nfunction longerName(x: number) {\n    return x + verbose;\n}\nconsole.log(longerName(2));\n",
        )
        .unwrap();
        let out = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false))
            .expect("bundle ok");
        assert!(
            out.code.lines().count() <= 10,
            "expected <= 10 lines after minification, got {}: {}",
            out.code.lines().count(),
            out.code
        );
    }

    #[test]
    fn bundle_inlines_zero_http_when_imported() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "import { createHttp } from \"zero/http\";\nconst c = createHttp();\nconsole.log(c);\n",
        )
        .unwrap();
        let result = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false));
        let bundled = result.unwrap().code;
        assert!(
            bundled.contains(r#"__zero_define("zero/http""#),
            "missing zero/http module definition: {bundled}"
        );
        assert!(
            bundled.contains("createHttp"),
            "createHttp identifier not in bundle: {bundled}"
        );
        assert!(
            bundled.contains("exports.createHttp"),
            "createHttp not exported: {bundled}"
        );
        assert!(
            bundled.contains(r#"__zero_require("zero/http")"#),
            "user module did not require zero/http: {bundled}"
        );
    }

    #[test]
    fn bundle_mixed_ts_and_js_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "import { x } from \"./util.js\";\nconst v: number = x;\nconsole.log(v);\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/util.js"),
            "import { y } from \"./inner.ts\";\nexport const x = y;\n",
        )
        .unwrap();
        std::fs::write(root.join("src/inner.ts"), "export const y: number = 1;\n").unwrap();
        let result = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false));
        let bundled = result.unwrap().code;
        assert!(bundled.contains(r#"__zero_define("./src/app.ts""#));
        assert!(bundled.contains(r#"__zero_define("./src/util.js""#));
        assert!(bundled.contains(r#"__zero_define("./src/inner.ts""#));
        assert!(!bundled.contains(": number"), "type leaked: {bundled}");
    }

    #[test]
    fn bundle_rewrites_zero_components_require_to_resolved_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("web");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".zero/components")).unwrap();
        std::fs::write(
            root.join(".zero/components/index.ts"),
            "export const Button = 1;\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/app.ts"),
            "import { Button } from \"zero/components\";\nconsole.log(Button);\n",
        )
        .unwrap();
        let result = with_cwd(dir.path(), || bundle(&write_minimal_config(&root), false));
        let bundled = result.unwrap().code;
        // The require call must reference the registered module ID, not the
        // unresolved bare specifier — otherwise `__zero_modules[id]` is undef
        // at runtime and crashes with "is not a function".
        assert!(
            bundled.contains(r#"__zero_require("./.zero/components/index.ts")"#),
            "expected require to resolve to component index path: {bundled}"
        );
        assert!(
            !bundled.contains(r#"__zero_require("zero/components")"#),
            "unresolved bare specifier survived in bundle: {bundled}"
        );
    }

    #[test]
    fn rewrite_rewrites_default_export() {
        let src = r#"export default function Home() { return 42; }
"#;
        let result = rewrite_module(
            src,
            Path::new("/root"),
            &ModuleId::User(PathBuf::from("./src/routes/home.js")),
            &HashMap::new(),
        )
        .unwrap();
        assert!(
            result.contains("exports.default = Home"),
            "default export not rewritten: {result}"
        );
    }
}
