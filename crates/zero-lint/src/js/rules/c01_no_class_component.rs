//! C01 — class declarations / expressions in `src/{components,routes}/**`.
//!
//! Zero components are plain functions returning `html\`\``. A class in
//! a component / route file is a strong signal that the author mistook
//! the component model. Test files are NOT exempt (per spec R3).

use swc_core::ecma::ast::{ClassDecl, ClassExpr};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run C01 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if !ctx.is_under_components_or_routes {
        return Vec::new();
    }
    let mut v = C01Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct C01Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for C01Visitor<'a, 'b> {
    fn visit_class_decl(&mut self, n: &ClassDecl) {
        let property = format!("class {}", n.ident.sym);
        self.diags.push(diag_at(
            "C01",
            self.ctx,
            n.class.span.lo,
            property,
            "component-model",
            "components are plain functions — no class-based components in zero. \
             See §3 'Component Model'.",
        ));
        n.visit_children_with(self);
    }

    fn visit_class_expr(&mut self, n: &ClassExpr) {
        let name = n
            .ident
            .as_ref()
            .map(|i| i.sym.to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());
        let property = format!("class {name}");
        self.diags.push(diag_at(
            "C01",
            self.ctx,
            n.class.span.lo,
            property,
            "component-model",
            "components are plain functions — no class-based components in zero. \
             See §3 'Component Model'.",
        ));
        n.visit_children_with(self);
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
    fn fires_on_class_declaration_in_components() {
        let d = run_at("class Widget {}", "/tmp/src/components/W.ts");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "C01");
        assert_eq!(d[0].property, "class Widget");
    }

    #[test]
    fn fires_on_class_expression_in_routes() {
        let d = run_at("const X = class Y {};", "/tmp/src/routes/r.ts");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].property, "class Y");
    }

    #[test]
    fn does_not_fire_in_lib_directory() {
        let d = run_at("class Widget {}", "/tmp/src/lib/util.ts");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_in_stores_directory() {
        let d = run_at("class Store {}", "/tmp/src/stores/auth.ts");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_in_components_test_file() {
        let d = run_at("class Widget {}", "/tmp/src/components/W.test.ts");
        assert_eq!(d.len(), 1, "C01 must apply in tests too");
    }
}
