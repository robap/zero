//! C02 — calls to `customElements.define(...)`.
//!
//! Zero ships a documented `z/wc` escape hatch for web-component
//! interop. Direct `customElements.define()` skips that interop layer.
//! Applies to every file under `src/**`, including tests.

use swc_core::ecma::ast::{CallExpr, Callee, Expr, MemberProp};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run C02 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    let mut v = C02Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct C02Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for C02Visitor<'a, 'b> {
    fn visit_call_expr(&mut self, c: &CallExpr) {
        if let Callee::Expr(e) = &c.callee
            && let Expr::Member(m) = e.as_ref()
            && let Expr::Ident(obj) = m.obj.as_ref()
            && obj.sym.as_str() == "customElements"
            && let MemberProp::Ident(prop) = &m.prop
            && prop.sym.as_str() == "define"
        {
            self.diags.push(diag_at(
                "C02",
                self.ctx,
                c.span.lo,
                "customElements.define".to_string(),
                "component-model",
                "register web components only via `import { define } from 'z/wc'` — \
                 see §11 'Web Component Interop'.",
            ));
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

    #[test]
    fn fires_on_custom_elements_define() {
        let d = run_at(
            "customElements.define('x-el', class {});",
            "/tmp/src/lib/wc.ts",
        );
        assert!(d.iter().any(|x| x.rule == "C02"), "expected C02 in {d:?}");
    }

    #[test]
    fn does_not_fire_on_unrelated_member_call() {
        let d = run_at("foo.define('x-el');", "/tmp/src/lib/wc.ts");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_in_test_file() {
        let d = run_at(
            "customElements.define('x-el', class {});",
            "/tmp/src/lib/wc.test.ts",
        );
        assert!(d.iter().any(|x| x.rule == "C02"));
    }
}
