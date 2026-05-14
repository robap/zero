//! Module graph walker and CommonJS-style bundle emitter.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::build::resolver::{ModuleId, resolve};
use crate::config::Config;
use crate::runtime::{ZERO_RUNTIME_BODY, ZERO_RUNTIME_EXPORTS};

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

/// Produce a single bundled JS string from the project root's `src/app.js`.
///
/// # Parameters
/// - `config`: the validated `zero.toml` configuration.
///
/// # Returns
/// A string containing the complete CommonJS-style bundle.
pub fn bundle(config: &Config) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root).canonicalize()?;
    let entry_path = root.join("src").join("app.js");

    let entry_id = ModuleId::User(PathBuf::from("./src/app.js"));

    // BFS to collect modules in visit order (dependencies first via post-order).
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
            ModuleId::User(_) => std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?,
        };
        let importer_dir = if id == ModuleId::Runtime {
            root.to_path_buf()
        } else {
            path.parent().unwrap_or(&root).to_path_buf()
        };

        // Discover imports and enqueue dependencies before this module.
        for specifier in extract_imports(&src) {
            let dep_id = resolve(&specifier, &importer_dir, &root)?;
            if !visited.contains(&dep_id) {
                match &dep_id {
                    ModuleId::Runtime => {
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

        let _ = src;
    }

    // Build module source map.
    let mut sources: HashMap<ModuleId, String> = HashMap::new();
    sources.insert(ModuleId::Runtime, ZERO_RUNTIME_BODY.to_string());
    for id in &order {
        if let ModuleId::User(rel) = id {
            let path = root.join(rel.strip_prefix("./").unwrap_or(rel));
            let src = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
            sources.insert(id.clone(), src);
        }
    }

    // Emit the bundle.
    let mut out = String::from(PREAMBLE);

    // Emit in order (Runtime first if present, then user modules in dependency order).
    let mut emit_order = order.clone();
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
    out.push_str("\n__zero_require('./src/app.js');\n");

    Ok(out)
}

/// The string key used in `__zero_define` / `__zero_require` calls.
fn module_id_string(id: &ModuleId) -> String {
    match id {
        ModuleId::Runtime => "'zero'".to_string(),
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
