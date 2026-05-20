//! T03 — `each(items, render)` called with no key function.
//!
//! Without a key fn, `each` falls back to index-based reconciliation
//! and identity is lost when items shift. Pass a third argument
//! `item => item.id` for stable diffing.

use swc_core::ecma::ast::{CallExpr, Callee, Expr};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run T03 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if !ctx.is_under_components_or_routes || ctx.is_test_file {
        return Vec::new();
    }
    let mut v = T03Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct T03Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for T03Visitor<'a, 'b> {
    fn visit_call_expr(&mut self, c: &CallExpr) {
        if let Callee::Expr(e) = &c.callee
            && let Expr::Ident(i) = e.as_ref()
        {
            let local = i.sym.to_string();
            if let Some(original) = self.ctx.zero_imports.get(&local)
                && original == "each"
                && c.args.len() == 2
            {
                self.diags.push(diag_at(
                    "T03",
                    self.ctx,
                    c.span.lo,
                    "each(<arg-count>=2)".to_string(),
                    "each",
                    "`each()` without a key function falls back to index-based \
                     reconciliation — pass a `keyFn` \
                     (`each(items, render, item => item.id)`) for stable identity.",
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

    #[test]
    fn fires_on_two_arg_each_in_components() {
        let d = run_at(
            "import { each } from \"zero\"; function C(){ each(items, render); }",
            "/tmp/src/components/List.ts",
        );
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "T03");
    }

    #[test]
    fn does_not_fire_on_three_arg_each() {
        let d = run_at(
            "import { each } from \"zero\"; function C(){ each(items, render, x => x.id); }",
            "/tmp/src/components/List.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_test_file() {
        let d = run_at(
            "import { each } from \"zero\"; function C(){ each(items, render); }",
            "/tmp/src/components/List.test.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_lib_directory() {
        let d = run_at(
            "import { each } from \"zero\"; function C(){ each(items, render); }",
            "/tmp/src/lib/util.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_on_non_zero_each() {
        let d = run_at(
            "import { each } from \"./util\"; function C(){ each(items, render); }",
            "/tmp/src/components/List.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }
}
