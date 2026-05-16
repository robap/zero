//! Module graph walker and CommonJS-style bundle emitter.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::build::resolver::{ModuleId, resolve};
use crate::config::Config;
use crate::runtime::{ZERO_HTTP_BODY, ZERO_HTTP_EXPORTS, ZERO_RUNTIME_BODY, ZERO_RUNTIME_EXPORTS};
use crate::transpile::{TranspileOptions, transpile_typescript};

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

    // BFS to collect modules in visit order (dependencies first via post-order).
    let mut sources: HashMap<ModuleId, String> = HashMap::new();
    sources.insert(ModuleId::Runtime, ZERO_RUNTIME_BODY.to_string());
    sources.insert(ModuleId::Http, ZERO_HTTP_BODY.to_string());
    let mut order: Vec<ModuleId> = Vec::new();
    let mut visited: HashSet<ModuleId> = HashSet::new();
    let mut queue: VecDeque<(ModuleId, PathBuf)> = VecDeque::new();
    queue.push_back((entry_id.clone(), entry_path));

    while let Some((id, path)) = queue.pop_front() {
        if visited.contains(&id) {
            continue;
        }
        visited.insert(id.clone());

        let src = match &id {
            ModuleId::Runtime => ZERO_RUNTIME_BODY.to_string(),
            ModuleId::Http => ZERO_HTTP_BODY.to_string(),
            ModuleId::User(rel) => {
                let raw = std::fs::read_to_string(&path)
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
                    out.code
                } else {
                    raw
                }
            }
        };
        sources.insert(id.clone(), src.clone());

        let importer_dir = if matches!(id, ModuleId::Runtime | ModuleId::Http) {
            root.to_path_buf()
        } else {
            path.parent().unwrap_or(&root).to_path_buf()
        };

        // Discover imports and enqueue dependencies before this module.
        for specifier in extract_imports(&src) {
            let dep_id = resolve(&specifier, &importer_dir, &root)?;
            if !visited.contains(&dep_id) {
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
        }
        order.push(id);
    }

    // Emit the bundle.
    let mut out = String::from(PREAMBLE);

    // Emit in order (synthetic modules first, then user modules in dependency order).
    let mut emit_order = order.clone();
    if emit_order.contains(&ModuleId::Http) {
        emit_order.retain(|id| *id != ModuleId::Http);
        emit_order.insert(0, ModuleId::Http);
    }
    if emit_order.contains(&ModuleId::Runtime) {
        emit_order.retain(|id| *id != ModuleId::Runtime);
        emit_order.insert(0, ModuleId::Runtime);
    }

    for id in &emit_order {
        let module_id_str = module_id_string(id);
        let raw_src = sources.get(id).map(|s| s.as_str()).unwrap_or("");
        let factory_body = rewrite_module(raw_src, &root, id, &sources)?;
        out.push_str(&format!(
            "\n__zero_define({module_id_str}, function(exports, __zero_require) {{\n{factory_body}\n}});\n"
        ));
    }

    // Bootstrap call.
    let bootstrap_id = match &entry_id {
        ModuleId::User(rel) => rel.to_string_lossy().into_owned(),
        ModuleId::Runtime | ModuleId::Http => unreachable!(),
    };
    out.push_str(&format!("\n__zero_require('{bootstrap_id}');\n"));

    let source_map = if emit_sourcemap {
        Some(build_combined_sourcemap(&emit_order)?)
    } else {
        None
    };

    Ok(BundleOutput {
        code: out,
        source_map,
    })
}

/// Build a coarse v3 sourcemap that lists each user module in `sources`.
///
/// The bundle is concatenated post-transpile, so we register source paths but
/// emit no row-level mappings — line-accurate stack traces inside the bundle
/// are not part of v1 scope. The map is well-formed JSON with `"version":3`
/// and a non-empty `"sources"` array.
fn build_combined_sourcemap(emit_order: &[ModuleId]) -> anyhow::Result<String> {
    let mut builder = sourcemap::SourceMapBuilder::new(None);
    for id in emit_order {
        if let ModuleId::User(rel) = id {
            let s = rel.to_string_lossy();
            builder.add_source(&s);
        }
    }
    // Runtime and Http synthetic modules are not user files; skip them.
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
    _id: &ModuleId,
    _sources: &HashMap<ModuleId, String>,
) -> anyhow::Result<String> {
    if _id == &ModuleId::Runtime {
        return rewrite_runtime_exports(src);
    }
    if _id == &ModuleId::Http {
        return rewrite_http_exports(src);
    }

    let single_line_import =
        Regex::new(r#"(?m)^[ \t]*import\s+\{([^}]+)\}\s*from\s+['"]([^'"]+)['"]\s*;?"#).unwrap();
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

    let export_default_fn = Regex::new(r"(?m)^export\s+default\s+function\s+(\w+)").unwrap();
    let export_default_val = Regex::new(r"(?m)^export\s+default\s+").unwrap();
    let export_named_fn = Regex::new(r"(?m)^export\s+function\s+(\w+)").unwrap();
    let export_named_const = Regex::new(r"(?m)^export\s+(const|let|var)\s+(\w+)").unwrap();
    let export_named_class = Regex::new(r"(?m)^export\s+class\s+(\w+)").unwrap();
    let export_block = Regex::new(r"(?ms)^export\s*\{\s*([^}]+?)\s*\}\s*;?").unwrap();

    let mut out = src.to_string();

    // Rewrite multi-line imports first.
    let multi_matches: Vec<_> = multi_line_import
        .find_iter(&out)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect();
    let mut offset_diff: i64 = 0;
    for (start, end, matched) in multi_matches {
        let start_adj = (start as i64 + offset_diff) as usize;
        let end_adj = (end as i64 + offset_diff) as usize;
        let replacement = if let Some(cap) = multi_single.captures(&matched) {
            let names = cap[1].replace('\n', " ");
            let spec = cap[2].to_string();
            format!("const {{{}}} = __zero_require('{}');", names.trim(), spec)
        } else {
            String::new()
        };
        let old_len = end_adj - start_adj;
        out.replace_range(start_adj..end_adj, &replacement);
        offset_diff += replacement.len() as i64 - old_len as i64;
    }

    // Single-line `import { a, b } from "..."`.
    out = single_line_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let names = &caps[1];
            let spec = &caps[2];
            format!("const {{{}}} = __zero_require('{spec}');", names.trim())
        })
        .into_owned();

    // Namespace import: `import * as Ns from "..."`.
    out = namespace_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            let spec = &caps[2];
            format!("const {name} = __zero_require('{spec}');")
        })
        .into_owned();

    // Default import: `import Foo from "..."`.
    out = default_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            let spec = &caps[2];
            format!("const {name} = __zero_require('{spec}').default;")
        })
        .into_owned();

    // Side-effect import: `import "..."`.
    out = side_effect_import
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let spec = &caps[1];
            format!("__zero_require('{spec}');")
        })
        .into_owned();

    // `export default function Foo(...) { ... }`.
    out = export_default_fn
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            format!("function {name}")
        })
        .into_owned();
    // Append `exports.default = Foo;` for named default functions.
    let default_fn_names: Vec<String> = export_default_fn
        .captures_iter(src)
        .map(|c| c[1].to_string())
        .collect();
    for name in &default_fn_names {
        out.push_str(&format!("\nexports.default = {name};\n"));
    }

    // `export default <expr>`.
    if default_fn_names.is_empty() {
        out = export_default_val
            .replace_all(&out, "exports.default = ")
            .into_owned();
    }

    // `export function foo(...) { ... }`.
    let named_fn_names: Vec<String> = export_named_fn
        .captures_iter(&out.clone())
        .map(|c| c[1].to_string())
        .collect();
    out = export_named_fn
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("function {}", &caps[1])
        })
        .into_owned();
    for name in &named_fn_names {
        out.push_str(&format!("\nexports.{name} = {name};\n"));
    }

    // `export class Foo { ... }`.
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

    // `export const/let/var foo = ...`.
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

    // `export { a, b as c }`.
    out = export_block
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let inner = &caps[1];
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
        })
        .into_owned();

    // Resolve relative specifiers in require calls to root-relative paths.
    let require_re = Regex::new(r#"__zero_require\('(\.[^']+)'\)"#).unwrap();
    let importer_dir = if let ModuleId::User(rel) = _id {
        let abs = root.join(rel.strip_prefix("./").unwrap_or(rel));
        abs.parent().unwrap_or(root).to_path_buf()
    } else {
        root.to_path_buf()
    };
    out = require_re
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let spec = &caps[1];
            if let Ok(resolved) = resolve(spec, &importer_dir, root) {
                match resolved {
                    ModuleId::Runtime => "__zero_require('zero')".to_string(),
                    ModuleId::Http => "__zero_require('zero/http')".to_string(),
                    ModuleId::User(rel) => {
                        format!("__zero_require('{}')", rel.to_str().unwrap_or(spec))
                    }
                }
            } else {
                format!("__zero_require('{spec}')")
            }
        })
        .into_owned();

    Ok(out)
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

/// Extract import specifiers from an ES module source file.
fn extract_imports(src: &str) -> Vec<String> {
    let re = Regex::new(
        r#"(?ms)import\s+(?:\{[^}]*\}|\*\s+as\s+\w+|\w+|\s*)\s*from\s+['"]([^'"]+)['"]|import\s+['"]([^'"]+)['"]"#,
    )
    .unwrap();
    re.captures_iter(src)
        .filter_map(|c| c.get(1).or_else(|| c.get(2)))
        .map(|m| m.as_str().to_string())
        .collect()
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
            bundled.contains("__zero_require('zero')"),
            "missing zero require: {bundled}"
        );
        let app_section_start = bundled
            .find("__zero_define('./src/app.ts'")
            .expect("expected app.ts module section");
        let app_section = &bundled[app_section_start..];
        let app_section_end = app_section
            .find("__zero_require('./src/app.ts')")
            .unwrap_or(app_section.len());
        let app_module = &app_section[..app_section_end];
        assert!(
            !app_module.contains(": number"),
            "type annotation leaked: {app_module}"
        );
        assert!(bundled.contains("__zero_require('./src/app.ts')"));
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
            bundled.contains("__zero_define('zero/http'"),
            "missing zero/http module definition: {bundled}"
        );
        assert!(
            bundled.contains("function createHttp("),
            "createHttp factory not inlined: {bundled}"
        );
        assert!(
            bundled.contains("__zero_require('zero/http')"),
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
        assert!(bundled.contains("__zero_define('./src/app.ts'"));
        assert!(bundled.contains("__zero_define('./src/util.js'"));
        assert!(bundled.contains("__zero_define('./src/inner.ts'"));
        assert!(!bundled.contains(": number"), "type leaked: {bundled}");
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
