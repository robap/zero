//! T01 — `addEventListener` / `removeEventListener` in components/routes.
//!
//! Direct event-listener wiring bypasses zero's `@event=` template
//! syntax, which automatically tears down handlers when the owning
//! scope unmounts. The lint applies only inside `src/components/**`
//! and `src/routes/**`; tests are exempt.

use swc_core::ecma::ast::{CallExpr, Callee, Expr, MemberProp};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run T01 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if !ctx.is_under_components_or_routes || ctx.is_test_file {
        return Vec::new();
    }
    let mut v = T01Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct T01Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for T01Visitor<'a, 'b> {
    fn visit_call_expr(&mut self, c: &CallExpr) {
        if let Callee::Expr(e) = &c.callee
            && let Expr::Member(m) = e.as_ref()
            && let MemberProp::Ident(prop) = &m.prop
            && matches!(
                prop.sym.as_str(),
                "addEventListener" | "removeEventListener"
            )
        {
            let receiver = render_receiver(&m.obj);
            let method = prop.sym.to_string();
            let property = format!("{receiver}.{method}");
            self.diags.push(diag_at(
                "T01",
                self.ctx,
                c.span.lo,
                property,
                "event",
                "use the `@event=` syntax inside `html` — \
                 direct `addEventListener` bypasses scope cleanup.",
            ));
        }
        c.visit_children_with(self);
    }
}

fn render_receiver(e: &Expr) -> String {
    match e {
        Expr::Ident(i) => i.sym.to_string(),
        Expr::This(_) => "this".to_string(),
        _ => "<expr>".to_string(),
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
    fn fires_on_add_event_listener_in_components() {
        let d = run_at(
            "function C(){ el.addEventListener('click', h); }",
            "/tmp/src/components/Btn.ts",
        );
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "T01");
        assert_eq!(d[0].property, "el.addEventListener");
    }

    #[test]
    fn fires_on_remove_event_listener_in_routes() {
        let d = run_at(
            "function R(){ el.removeEventListener('click', h); }",
            "/tmp/src/routes/home.ts",
        );
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].property, "el.removeEventListener");
    }

    #[test]
    fn does_not_fire_in_lib_directory() {
        let d = run_at(
            "function f(){ el.addEventListener('click', h); }",
            "/tmp/src/lib/util.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_test_file() {
        let d = run_at(
            "function f(){ el.addEventListener('click', h); }",
            "/tmp/src/components/Btn.test.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }
}
