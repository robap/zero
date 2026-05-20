//! I02 — relative import whose resolved path lands under `<root>/.zero/`.
//!
//! `.zero/` is framework-owned; the user reaches into it only through
//! the public surface (`"zero"`, `"zero/components"`, etc.). Climbing
//! into `.zero/` with a relative path bypasses the contract.

use std::path::{Path, PathBuf};
use swc_core::ecma::ast::{CallExpr, Callee, Expr, ImportDecl, Lit};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run I02 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    let dot_zero = match ctx.root.join(".zero").canonicalize() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let importer_dir = match ctx.file.parent() {
        Some(p) => p.to_path_buf(),
        None => return Vec::new(),
    };
    let mut v = I02Visitor {
        ctx,
        dot_zero,
        importer_dir,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct I02Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    dot_zero: PathBuf,
    importer_dir: PathBuf,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> I02Visitor<'a, 'b> {
    fn check_specifier(&mut self, spec: &str, pos: swc_core::common::BytePos) {
        if !(spec.starts_with("./") || spec.starts_with("../")) {
            return;
        }
        let raw = self.importer_dir.join(spec);
        let resolved = match raw.canonicalize() {
            Ok(p) => p,
            Err(_) => return,
        };
        if resolved.starts_with(&self.dot_zero) {
            self.diags.push(diag_at(
                "I02",
                self.ctx,
                pos,
                spec.to_string(),
                "import",
                "`.zero/` is framework-owned — import from the public surface \
                 (`'zero'`, `'zero/test'`, `'zero/http'`, `'zero/components'`).",
            ));
        }
    }
}

impl<'a, 'b> Visit for I02Visitor<'a, 'b> {
    fn visit_import_decl(&mut self, d: &ImportDecl) {
        let spec = d.src.value.as_str().unwrap_or("");
        self.check_specifier(spec, d.src.span.lo);
        d.visit_children_with(self);
    }

    fn visit_call_expr(&mut self, c: &CallExpr) {
        if let Callee::Import(_) = &c.callee
            && let Some(first) = c.args.first()
            && let Expr::Lit(Lit::Str(s)) = first.expr.as_ref()
        {
            let spec = s.value.as_str().unwrap_or("");
            self.check_specifier(spec, s.span.lo);
        }
        c.visit_children_with(self);
    }
}

#[allow(dead_code)]
fn _types_use(_p: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn lint_in(root: &Path, file_rel: &str, source: &str) -> Vec<Diagnostic> {
        let file = root.join(file_rel);
        fs::write(&file, source).unwrap();
        let parsed = zero_transpile::parse_module(source, file.to_str().unwrap()).expect("parse");
        let ctx = FileCtx::new(&file, source, root, parsed.source_map, parsed.module);
        check(&ctx)
    }

    #[test]
    fn fires_on_relative_climb_into_dot_zero() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".zero/components")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join(".zero/components/Button.ts"), "").unwrap();
        let d = lint_in(
            root,
            "src/foo.ts",
            "import x from \"../.zero/components/Button.ts\";",
        );
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "I02");
    }

    #[test]
    fn does_not_fire_on_in_src_relative_import() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".zero")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/util.ts"), "").unwrap();
        let d = lint_in(root, "src/foo.ts", "import x from \"./util.ts\";");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_when_dot_zero_dir_does_not_exist() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        let d = lint_in(root, "src/foo.ts", "import x from \"../something.ts\";");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }
}
