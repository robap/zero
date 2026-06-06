//! Per-file lint context: parsed module, source map, and pre-computed
//! file-shape flags used by multiple rules.

use std::collections::HashMap;
use std::path::Path;

use zero_transpile::{Lrc, SourceMap, ast};

/// Per-file context passed to every JS/TS rule.
pub struct FileCtx<'a> {
    /// Absolute path of the file being linted.
    pub file: &'a Path,
    /// File's original source text.
    pub source: &'a str,
    /// Project root (the directory that contains `src/`).
    pub root: &'a Path,
    /// `SourceMap` from the parse; used to translate `BytePos` to
    /// 1-based `(line, column)` pairs.
    pub source_map: Lrc<SourceMap>,
    /// Parsed AST module.
    pub module: ast::Module,
    /// `true` when the file's basename matches `*.test.{ts,js,tsx,jsx}` or
    /// `*.spec.{ts,js,tsx,jsx}`.
    pub is_test_file: bool,
    /// `true` when the file lives under `<root>/src/components/` or
    /// `<root>/src/routes/`.
    pub is_under_components_or_routes: bool,
    /// `true` when the file is the app entry point —
    /// `<root>/src/app.{ts,tsx,js,jsx}`. R03 exempts this file because
    /// the entry module IS the bootstrap scope.
    pub is_app_entry: bool,
    /// Map of *local* binding name → *original* exported name for every
    /// specifier imported from `"zero"`. Lets rules detect aliased
    /// imports like `import { signal as makeSig } from "zero"`.
    pub zero_imports: HashMap<String, String>,
}

impl<'a> FileCtx<'a> {
    /// Build a fresh `FileCtx`. `file` must be an absolute path under
    /// `<root>/src/`; `module` and `source_map` are the parse output.
    pub fn new(
        file: &'a Path,
        source: &'a str,
        root: &'a Path,
        source_map: Lrc<SourceMap>,
        module: ast::Module,
    ) -> Self {
        let is_test_file = is_test_basename(file);
        let is_under_components_or_routes = classify_dir(file, root);
        let is_app_entry = is_app_entry(file, root);
        let zero_imports = collect_zero_imports(&module);
        Self {
            file,
            source,
            root,
            source_map,
            module,
            is_test_file,
            is_under_components_or_routes,
            is_app_entry,
            zero_imports,
        }
    }
}

fn is_test_basename(file: &Path) -> bool {
    let Some(name) = file.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    for suffix in [
        ".test.ts",
        ".test.tsx",
        ".test.js",
        ".test.jsx",
        ".spec.ts",
        ".spec.tsx",
        ".spec.js",
        ".spec.jsx",
    ] {
        if lower.ends_with(suffix) {
            return true;
        }
    }
    false
}

fn is_app_entry(file: &Path, root: &Path) -> bool {
    let Ok(rel) = file.strip_prefix(root) else {
        return false;
    };
    let mut comps = rel.components();
    if comps.next().map(|c| c.as_os_str()) != Some(std::ffi::OsStr::new("src")) {
        return false;
    }
    let Some(name) = comps.next().and_then(|c| c.as_os_str().to_str()) else {
        return false;
    };
    if comps.next().is_some() {
        return false; // Nested files don't count.
    }
    matches!(name, "app.ts" | "app.tsx" | "app.js" | "app.jsx")
}

fn classify_dir(file: &Path, root: &Path) -> bool {
    let Ok(rel) = file.strip_prefix(root) else {
        return false;
    };
    let mut comps = rel.components();
    if comps.next().map(|c| c.as_os_str()) != Some(std::ffi::OsStr::new("src")) {
        return false;
    }
    let first = comps.next().and_then(|c| c.as_os_str().to_str());
    matches!(first, Some("components") | Some("routes"))
}

fn collect_zero_imports(module: &ast::Module) -> HashMap<String, String> {
    use ast::{ImportSpecifier, ModuleDecl, ModuleExportName, ModuleItem};
    let mut map: HashMap<String, String> = HashMap::new();
    for item in &module.body {
        let ModuleItem::ModuleDecl(ModuleDecl::Import(decl)) = item else {
            continue;
        };
        if decl.src.value.as_str() != Some("zero") {
            continue;
        }
        for spec in &decl.specifiers {
            match spec {
                ImportSpecifier::Named(n) => {
                    let local = n.local.sym.to_string();
                    let original = match &n.imported {
                        Some(ModuleExportName::Ident(i)) => i.sym.to_string(),
                        Some(ModuleExportName::Str(s)) => {
                            s.value.as_str().map(|v| v.to_string()).unwrap_or_default()
                        }
                        None => local.clone(),
                    };
                    map.insert(local, original);
                }
                ImportSpecifier::Default(d) => {
                    map.insert(d.local.sym.to_string(), "default".to_string());
                }
                ImportSpecifier::Namespace(ns) => {
                    map.insert(ns.local.sym.to_string(), "*".to_string());
                }
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse(source: &str) -> (ast::Module, Lrc<SourceMap>) {
        let parsed = zero_transpile::parse_module(source, "test.ts").expect("parse");
        (parsed.module, parsed.source_map)
    }

    #[test]
    fn detects_test_file_by_basename() {
        let (m, cm) = parse("");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/foo.test.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(ctx.is_test_file);
    }

    #[test]
    fn detects_spec_file_by_basename() {
        let (m, cm) = parse("");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/bar.spec.js");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(ctx.is_test_file);
    }

    #[test]
    fn non_test_file_not_flagged() {
        let (m, cm) = parse("");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/app.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(!ctx.is_test_file);
    }

    #[test]
    fn classifies_components_dir() {
        let (m, cm) = parse("");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/components/Btn.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(ctx.is_under_components_or_routes);
    }

    #[test]
    fn classifies_routes_dir() {
        let (m, cm) = parse("");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/routes/home.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(ctx.is_under_components_or_routes);
    }

    #[test]
    fn classifies_stores_dir_as_not_components_or_routes() {
        let (m, cm) = parse("");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/stores/auth.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(!ctx.is_under_components_or_routes);
    }

    #[test]
    fn collects_named_zero_imports() {
        let (m, cm) = parse("import { signal, computed } from \"zero\";");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/app.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert_eq!(
            ctx.zero_imports.get("signal").map(|s| s.as_str()),
            Some("signal")
        );
        assert_eq!(
            ctx.zero_imports.get("computed").map(|s| s.as_str()),
            Some("computed")
        );
    }

    #[test]
    fn collects_aliased_zero_imports() {
        let (m, cm) = parse("import { signal as makeSig } from \"zero\";");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/app.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert_eq!(
            ctx.zero_imports.get("makeSig").map(|s| s.as_str()),
            Some("signal")
        );
    }

    #[test]
    fn ignores_non_zero_imports() {
        let (m, cm) = parse("import { each } from \"./util.ts\";");
        let root = PathBuf::from("/tmp");
        let file = PathBuf::from("/tmp/src/app.ts");
        let ctx = FileCtx::new(&file, "", &root, cm, m);
        assert!(ctx.zero_imports.is_empty());
    }
}
