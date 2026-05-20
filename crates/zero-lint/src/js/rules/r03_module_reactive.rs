//! R03 — module-level `signal(...)` / `computed(...)` / `effect(...)`.
//!
//! Reactive primitives created at module scope have no owning scope —
//! they leak listeners across reloads, sit in memory for the life of
//! the bundle, and confuse ownership. Move them into a function or a
//! store factory under `src/stores/**`.

use swc_core::ecma::ast::{
    ArrowExpr, CallExpr, Callee, ClassMethod, Constructor, Expr, FnDecl, FnExpr, GetterProp,
    MethodProp, PrivateMethod, SetterProp,
};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run R03 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if ctx.is_under_stores || ctx.is_test_file || ctx.is_app_entry || ctx.zero_imports.is_empty() {
        return Vec::new();
    }
    let mut v = R03Visitor {
        ctx,
        depth: 0,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct R03Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    depth: u32,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> R03Visitor<'a, 'b> {
    fn enter<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.depth += 1;
        f(self);
        self.depth -= 1;
    }
}

impl<'a, 'b> Visit for R03Visitor<'a, 'b> {
    fn visit_fn_decl(&mut self, n: &FnDecl) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_fn_expr(&mut self, n: &FnExpr) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_arrow_expr(&mut self, n: &ArrowExpr) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_class_method(&mut self, n: &ClassMethod) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_private_method(&mut self, n: &PrivateMethod) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_constructor(&mut self, n: &Constructor) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_method_prop(&mut self, n: &MethodProp) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_getter_prop(&mut self, n: &GetterProp) {
        self.enter(|v| n.visit_children_with(v));
    }
    fn visit_setter_prop(&mut self, n: &SetterProp) {
        self.enter(|v| n.visit_children_with(v));
    }

    fn visit_call_expr(&mut self, c: &CallExpr) {
        if self.depth == 0
            && let Callee::Expr(e) = &c.callee
            && let Expr::Ident(i) = e.as_ref()
        {
            let local = i.sym.to_string();
            if let Some(original) = self.ctx.zero_imports.get(&local)
                && matches!(original.as_str(), "signal" | "computed" | "effect")
            {
                let message = format!(
                    "module-level `{original}(...)` leaks — it has no owning scope. \
                     Move into a function, the app entry (`src/app.ts`), or a \
                     store factory under `src/stores/**`."
                );
                self.diags.push(diag_at(
                    "R03",
                    self.ctx,
                    c.span.lo,
                    original.clone(),
                    "module-scope",
                    message,
                ));
            }
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
        // Default to a non-exempt path: `src/lib/`. `src/app.ts` triggers
        // the dedicated app-entry exemption.
        run_at(source, "/tmp/src/lib/main.ts")
    }

    #[test]
    fn fires_at_top_level() {
        let d = run("import { signal } from \"zero\"; const c = signal(0);");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "R03");
        assert_eq!(d[0].property, "signal");
    }

    #[test]
    fn does_not_fire_inside_function() {
        let d = run("import { signal } from \"zero\"; function f(){ signal(0); }");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_inside_arrow() {
        let d = run("import { signal } from \"zero\"; const f = () => signal(0);");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_inside_class_method() {
        let d = run("import { signal } from \"zero\"; class C { m(){ signal(0); } }");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_app_entry() {
        let d = run_at(
            "import { signal, effect } from \"zero\"; const t = signal(0); effect(() => t.val);",
            "/tmp/src/app.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_stores_directory() {
        let d = run_at(
            "import { signal } from \"zero\"; const c = signal(0);",
            "/tmp/src/stores/foo.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_test_file() {
        let d = run_at(
            "import { signal } from \"zero\"; const c = signal(0);",
            "/tmp/src/app.test.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_without_zero_import() {
        let d = run("const c = signal(0);");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn respects_import_alias() {
        let d = run("import { signal as makeSig } from \"zero\"; const c = makeSig(0);");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].property, "signal");
    }
}
