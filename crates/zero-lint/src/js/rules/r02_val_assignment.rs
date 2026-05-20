//! R02 — direct assignment to `signal.val` (`x.val = ...`, `x.val += ...`).
//!
//! Signals are immutable from the outside; mutating `.val` bypasses the
//! reactivity bookkeeping. Use `.set(...)` or `.update(fn)` instead.
//! Rule fires only when the file imports from `"zero"` — the
//! identifier `.val` shows up unrelated to the framework in plenty of
//! non-zero code.

use swc_core::ecma::ast::{AssignExpr, AssignTarget, MemberProp, SimpleAssignTarget};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run R02 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if ctx.zero_imports.is_empty() {
        return Vec::new();
    }
    let mut v = R02Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct R02Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for R02Visitor<'a, 'b> {
    fn visit_assign_expr(&mut self, a: &AssignExpr) {
        if let AssignTarget::Simple(SimpleAssignTarget::Member(m)) = &a.left
            && let MemberProp::Ident(prop) = &m.prop
            && prop.sym.as_str() == "val"
        {
            let obj_text = render_obj(&m.obj);
            let property = format!("{obj_text}.val");
            self.diags.push(diag_at(
                "R02",
                self.ctx,
                m.span.lo,
                property,
                "assignment",
                "signals are immutable from the outside — use `.set(...)` or `.update(fn)`.",
            ));
        }
        a.visit_children_with(self);
    }
}

fn render_obj(e: &swc_core::ecma::ast::Expr) -> String {
    match e {
        swc_core::ecma::ast::Expr::Ident(i) => i.sym.to_string(),
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

    fn run(source: &str) -> Vec<Diagnostic> {
        run_at(source, "/tmp/src/app.ts")
    }

    #[test]
    fn fires_on_simple_val_assignment() {
        let d = run("import { signal } from \"zero\"; count.val = 1;");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "R02");
        assert_eq!(d[0].property, "count.val");
    }

    #[test]
    fn fires_on_compound_val_assignment() {
        let d = run("import { signal } from \"zero\"; count.val += 1;");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].rule, "R02");
    }

    #[test]
    fn does_not_fire_without_zero_import() {
        let d = run("count.val = 1;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_on_non_val_assignment() {
        let d = run("import { signal } from \"zero\"; count.value = 1;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_in_test_file() {
        let d = run_at(
            "import { signal } from \"zero\"; count.val = 1;",
            "/tmp/src/app.test.ts",
        );
        assert_eq!(d.len(), 1, "R02 must apply in tests too");
    }
}
