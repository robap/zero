//! I01 — bare import specifiers outside the public allowlist.
//!
//! SOURCE OF TRUTH: `crates/zero-bundler/src/resolver.rs` defines the
//! exact set of bare specifiers the runtime resolves
//! (`"zero"`, `"zero/components"`, `"zero/http"`); `runtime/test.js`
//! adds `"zero/test"`. Any other bare specifier (npm names, `node:` /
//! `npm:` protocols) breaks at build time — surface it here so the
//! agent sees the rule before the bundler does.

use swc_core::ecma::ast::{CallExpr, Callee, Expr, ImportDecl, Lit};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Allowed bare specifiers — must stay in lockstep with the bundler resolver
/// and the test runner.
const ALLOWED_BARE_SPECIFIERS: &[&str] = &["zero", "zero/components", "zero/http", "zero/test"];

/// Run I01 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    let mut v = I01Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct I01Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> I01Visitor<'a, 'b> {
    fn check_specifier(&mut self, spec: &str, pos: swc_core::common::BytePos) {
        if is_path_specifier(spec) || ALLOWED_BARE_SPECIFIERS.contains(&spec) {
            return;
        }
        let message =
            format!("zero has no node_modules — `{spec}` is not part of the framework runtime.");
        self.diags.push(diag_at(
            "I01",
            self.ctx,
            pos,
            spec.to_string(),
            "import",
            message,
        ));
    }
}

fn is_path_specifier(s: &str) -> bool {
    s.starts_with("./") || s.starts_with("../") || s.starts_with('/')
}

impl<'a, 'b> Visit for I01Visitor<'a, 'b> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use zero_transpile::parse_module;

    fn run_at(source: &str, file: &str) -> Vec<Diagnostic> {
        let parsed = parse_module(source, "x.ts").expect("parse");
        let file = PathBuf::from(file);
        let root = PathBuf::from("/tmp");
        let ctx = FileCtx::new(&file, source, &root, parsed.source_map, parsed.module);
        check(&ctx)
    }

    fn run(source: &str) -> Vec<Diagnostic> {
        run_at(source, "/tmp/src/app.ts")
    }

    #[test]
    fn fires_on_npm_bare_specifier() {
        let d = run("import x from \"lodash\";");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "I01");
        assert_eq!(d[0].property, "lodash");
    }

    #[test]
    fn fires_on_node_protocol() {
        let d = run("import fs from \"node:fs\";");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].property, "node:fs");
    }

    #[test]
    fn fires_on_npm_protocol() {
        let d = run("import x from \"npm:left-pad\";");
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn does_not_fire_on_zero() {
        assert!(run("import { signal } from \"zero\";").is_empty());
    }

    #[test]
    fn does_not_fire_on_zero_components() {
        assert!(run("import x from \"zero/components\";").is_empty());
    }

    #[test]
    fn does_not_fire_on_zero_http() {
        assert!(run("import { http } from \"zero/http\";").is_empty());
    }

    #[test]
    fn does_not_fire_on_zero_test() {
        assert!(run("import { test } from \"zero/test\";").is_empty());
    }

    #[test]
    fn does_not_fire_on_relative_path() {
        assert!(run("import x from \"./util.ts\";").is_empty());
    }

    #[test]
    fn fires_on_dynamic_import_npm() {
        let d = run("async function f(){ await import(\"lodash\"); }");
        assert_eq!(d.len(), 1, "got {d:?}");
        assert_eq!(d[0].property, "lodash");
    }
}
