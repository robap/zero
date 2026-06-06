//! R03 — module-level `effect(...)`.
//!
//! An effect created at module scope starts running at import time and
//! has no owning scope — nothing ever disposes it, so it keeps firing
//! for the life of the bundle (and across tests). Move it into a
//! function, a component body, or the app entry (`src/app.ts`), where
//! the render scope owns its lifetime.
//!
//! Module-level `signal()` / `computed()` are deliberately *not*
//! flagged: top-of-module state is exactly what a store is, and a
//! store is wherever the project says it is — the rule does not guess
//! intent from directory names.

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
    if ctx.is_test_file || ctx.is_app_entry || ctx.zero_imports.is_empty() {
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
                && original.as_str() == "effect"
            {
                let message = "module-level `effect(...)` leaks — it starts at import \
                     time and nothing ever disposes it. Move into a function, a \
                     component body, or the app entry (`src/app.ts`)."
                    .to_string();
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
    fn fires_on_top_level_effect() {
        let d = run("import { effect } from \"zero\"; effect(() => {});");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "R03");
        assert_eq!(d[0].property, "effect");
    }

    #[test]
    fn does_not_fire_on_top_level_signal_or_computed() {
        let d = run("import { signal, computed } from \"zero\"; \
             const c = signal(0); const d = computed(() => c.val * 2);");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_inside_function() {
        let d = run("import { effect } from \"zero\"; function f(){ effect(() => {}); }");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_inside_arrow() {
        let d = run("import { effect } from \"zero\"; const f = () => effect(() => {});");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_inside_class_method() {
        let d = run("import { effect } from \"zero\"; class C { m(){ effect(() => {}); } }");
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
    fn fires_on_top_level_effect_in_stores_directory() {
        // Stores are no longer location-exempt: an effect at module scope
        // leaks regardless of which directory declares it.
        let d = run_at(
            "import { effect } from \"zero\"; effect(() => {});",
            "/tmp/src/stores/foo.ts",
        );
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
    }

    #[test]
    fn does_not_fire_on_signal_in_arbitrary_directory() {
        // A store is wherever the project says it is — feature-first
        // layouts must lint clean.
        let d = run_at(
            "import { signal } from \"zero\"; const c = signal(0);",
            "/tmp/src/features/parts/store.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_test_file() {
        let d = run_at(
            "import { effect } from \"zero\"; effect(() => {});",
            "/tmp/src/app.test.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_without_zero_import() {
        let d = run("effect(() => {});");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn respects_import_alias() {
        let d = run("import { effect as fx } from \"zero\"; fx(() => {});");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].property, "effect");
    }
}
