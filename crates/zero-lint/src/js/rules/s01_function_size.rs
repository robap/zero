//! S01 — function body exceeds 80 lines (open brace to close brace
//! inclusive).
//!
//! Counts only block bodies. Arrow expressions with no braces are
//! excluded (their body is the expression itself).

use swc_core::common::Span;
use swc_core::ecma::ast::{
    ArrowExpr, BlockStmtOrExpr, ClassMethod, Constructor, FnDecl, FnExpr, GetterProp, MethodProp,
    PrivateMethod, PropName, SetterProp,
};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

const MAX_LINES: u32 = 80;

/// Run S01 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    let mut v = S01Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct S01Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> S01Visitor<'a, 'b> {
    fn report(&mut self, name: &str, body: Span) {
        let lo = self.ctx.source_map.lookup_char_pos(body.lo);
        let hi = self.ctx.source_map.lookup_char_pos(body.hi);
        let lines = (hi.line as u32).saturating_sub(lo.line as u32) + 1;
        if lines > MAX_LINES {
            let message = format!(
                "function `{name}` is {lines} lines (open brace to close brace inclusive); \
                 zero targets <= {MAX_LINES} — split into named helpers."
            );
            self.diags.push(diag_at(
                "S01",
                self.ctx,
                body.lo,
                name.to_string(),
                format!("size:{lines}"),
                message,
            ));
        }
    }
}

fn prop_name(n: &PropName) -> String {
    match n {
        PropName::Ident(i) => i.sym.to_string(),
        PropName::Str(s) => s.value.as_str().map(|v| v.to_string()).unwrap_or_default(),
        _ => "<anonymous>".to_string(),
    }
}

impl<'a, 'b> Visit for S01Visitor<'a, 'b> {
    fn visit_fn_decl(&mut self, n: &FnDecl) {
        if let Some(b) = &n.function.body {
            let name = n.ident.sym.to_string();
            self.report(&name, b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_fn_expr(&mut self, n: &FnExpr) {
        if let Some(b) = &n.function.body {
            let name = n
                .ident
                .as_ref()
                .map(|i| i.sym.to_string())
                .unwrap_or_else(|| "<anonymous>".to_string());
            self.report(&name, b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_arrow_expr(&mut self, n: &ArrowExpr) {
        if let BlockStmtOrExpr::BlockStmt(b) = n.body.as_ref() {
            self.report("<anonymous>", b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_class_method(&mut self, n: &ClassMethod) {
        if let Some(b) = &n.function.body {
            let name = prop_name(&n.key);
            self.report(&name, b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_private_method(&mut self, n: &PrivateMethod) {
        if let Some(b) = &n.function.body {
            let name = format!("#{}", n.key.name);
            self.report(&name, b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_constructor(&mut self, n: &Constructor) {
        if let Some(b) = &n.body {
            self.report("constructor", b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_method_prop(&mut self, n: &MethodProp) {
        if let Some(b) = &n.function.body {
            let name = prop_name(&n.key);
            self.report(&name, b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_getter_prop(&mut self, n: &GetterProp) {
        if let Some(b) = &n.body {
            let name = prop_name(&n.key);
            self.report(&name, b.span);
        }
        n.visit_children_with(self);
    }

    fn visit_setter_prop(&mut self, n: &SetterProp) {
        if let Some(b) = &n.body {
            let name = prop_name(&n.key);
            self.report(&name, b.span);
        }
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

    fn run(source: &str) -> Vec<Diagnostic> {
        run_at(source, "/tmp/src/app.ts")
    }

    fn big_body(lines: usize) -> String {
        // First line is `{`, last is `}`. Inner lines: `void 0;`
        let mut s = String::from("{\n");
        for _ in 0..lines.saturating_sub(2) {
            s.push_str("  void 0;\n");
        }
        s.push('}');
        s
    }

    #[test]
    fn fires_on_oversized_function_decl() {
        let body = big_body(90);
        let src = format!("function f() {body}");
        let d = run(&src);
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "S01");
        assert_eq!(d[0].property, "f");
    }

    #[test]
    fn does_not_fire_on_eighty_line_function() {
        let body = big_body(80);
        let src = format!("function f() {body}");
        let d = run(&src);
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_on_oversized_arrow_with_block_body() {
        let body = big_body(90);
        let src = format!("const f = () => {body};");
        let d = run(&src);
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].property, "<anonymous>");
    }

    #[test]
    fn does_not_fire_on_short_arrow_expression_body() {
        let d = run("const f = x => x + 1;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_on_oversized_class_method() {
        let body = big_body(90);
        let src = format!("class C {{ m() {body} }}");
        let d = run(&src);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].property, "m");
    }

    #[test]
    fn reports_function_name_when_named() {
        let body = big_body(85);
        let src = format!("function myFn() {body}");
        let d = run(&src);
        assert_eq!(d[0].property, "myFn");
    }

    #[test]
    fn reports_anonymous_when_unnamed() {
        let body = big_body(85);
        let src = format!("const x = function() {body};");
        let d = run(&src);
        assert_eq!(d[0].property, "<anonymous>");
    }
}
