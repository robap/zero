//! Build-time concatenation of `runtime/*.js` into a single ES module body.
//!
//! Strips `import` statements (single-line and multi-line) and converts
//! `export` declarations (`export function`, `export class`, `export const`,
//! `export let`) to plain declarations. `export { x as y }` aliases become
//! `const y = x;` assignments. Bare `export { name };` re-export blocks are
//! dropped (the symbol is already in scope post-concat).

use std::env;
use std::fs;
use std::path::PathBuf;

use regex::Regex;

/// Runtime files in dependency order; `dom-shim.js` is excluded (test-only).
const RUNTIME_FILES: &[&str] = &["reactivity.js", "template.js", "router.js", "app.js"];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let runtime_dir = manifest_dir.join("runtime");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    for f in RUNTIME_FILES {
        println!("cargo:rerun-if-changed={}", runtime_dir.join(f).display());
    }
    println!("cargo:rerun-if-changed=build.rs");

    let single_line_import =
        Regex::new(r#"(?m)^[ \t]*import\s+[^;\n]*?\s+from\s+['"][^'"]+['"]\s*;?[ \t]*\r?\n?"#)
            .unwrap();
    let multi_line_import =
        Regex::new(r#"(?ms)^[ \t]*import\s*\{[^}]*\}\s*from\s+['"][^'"]+['"]\s*;?[ \t]*\r?\n?"#)
            .unwrap();
    let bare_import = Regex::new(r#"(?m)^[ \t]*import\s+['"][^'"]+['"]\s*;?[ \t]*\r?\n?"#).unwrap();

    let export_function = Regex::new(r"(?m)^([ \t]*)export\s+function\b").unwrap();
    let export_class = Regex::new(r"(?m)^([ \t]*)export\s+class\b").unwrap();
    let export_const = Regex::new(r"(?m)^([ \t]*)export\s+const\b").unwrap();
    let export_let = Regex::new(r"(?m)^([ \t]*)export\s+let\b").unwrap();

    let export_block =
        Regex::new(r"(?ms)^[ \t]*export\s*\{\s*([^}]+?)\s*\}\s*;?[ \t]*\r?\n?").unwrap();

    let mut body = String::new();
    for f in RUNTIME_FILES {
        let path = runtime_dir.join(f);
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        let mut cleaned = raw;
        cleaned = multi_line_import.replace_all(&cleaned, "").into_owned();
        cleaned = single_line_import.replace_all(&cleaned, "").into_owned();
        cleaned = bare_import.replace_all(&cleaned, "").into_owned();

        cleaned = export_function
            .replace_all(&cleaned, "${1}function")
            .into_owned();
        cleaned = export_class.replace_all(&cleaned, "${1}class").into_owned();
        cleaned = export_const.replace_all(&cleaned, "${1}const").into_owned();
        cleaned = export_let.replace_all(&cleaned, "${1}let").into_owned();

        let mut alias_lines = String::new();
        cleaned = export_block
            .replace_all(&cleaned, |caps: &regex::Captures<'_>| {
                let inner = caps.get(1).unwrap().as_str();
                for spec in inner.split(',') {
                    let spec = spec.trim();
                    if spec.is_empty() {
                        continue;
                    }
                    if let Some((orig, alias)) = split_as(spec) {
                        alias_lines.push_str(&format!("const {alias} = {orig};\n"));
                    }
                    // Bare `name` re-exports: symbol already in scope, drop.
                }
                String::new()
            })
            .into_owned();

        body.push_str(&format!("/* === {f} === */\n"));
        body.push_str(&cleaned);
        if !cleaned.ends_with('\n') {
            body.push('\n');
        }
        if !alias_lines.is_empty() {
            body.push_str(&alias_lines);
        }
    }

    let out_path = out_dir.join("zero_runtime_body.js");
    fs::write(&out_path, body)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_path.display()));
}

/// Split an `x as y` export specifier into `(x, y)`. Returns `None` for bare names.
fn split_as(spec: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = spec.split_whitespace().collect();
    match parts.as_slice() {
        [orig, "as", alias] => Some((orig, alias)),
        _ => None,
    }
}
