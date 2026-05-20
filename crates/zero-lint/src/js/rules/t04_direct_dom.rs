//! T04 — direct DOM access inside components / routes.
//!
//! Two heuristics:
//!   a) Member chains rooted at the global `document` identifier with
//!      DOM-query properties (`querySelector`, `getElementById`,
//!      `querySelectorAll`).
//!   b) Mutating method calls (`appendChild`, `removeChild`,
//!      `insertBefore`, `replaceChild`) and `innerHTML` assignments
//!      whose immediate receiver isn't a `.el` member access — the
//!      `ref().el` escape hatch is intentional and accepted.

use swc_core::ecma::ast::{
    AssignExpr, AssignTarget, CallExpr, Callee, Expr, MemberExpr, MemberProp, SimpleAssignTarget,
};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run T04 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if !ctx.is_under_components_or_routes || ctx.is_test_file {
        return Vec::new();
    }
    let mut v = T04Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

const DOM_QUERY_PROPS: &[&str] = &["querySelector", "getElementById", "querySelectorAll"];
const MUTATING_METHODS: &[&str] = &["appendChild", "removeChild", "insertBefore", "replaceChild"];

struct T04Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for T04Visitor<'a, 'b> {
    fn visit_member_expr(&mut self, m: &MemberExpr) {
        // Heuristic (a): document.<query>
        if let MemberProp::Ident(prop) = &m.prop
            && DOM_QUERY_PROPS.contains(&prop.sym.as_str())
            && leftmost_is_document(&m.obj)
        {
            let property = format!("document.{}", prop.sym);
            self.diags.push(diag_at(
                "T04",
                self.ctx,
                m.span.lo,
                property,
                "dom",
                "direct DOM access inside a component bypasses the reactivity \
                 system — use `ref()` for element handles.",
            ));
        }
        m.visit_children_with(self);
    }

    fn visit_call_expr(&mut self, c: &CallExpr) {
        // Heuristic (b): receiver.appendChild(...) etc.
        if let Callee::Expr(e) = &c.callee
            && let Expr::Member(m) = e.as_ref()
            && let MemberProp::Ident(prop) = &m.prop
            && MUTATING_METHODS.contains(&prop.sym.as_str())
            && !receiver_is_ref_el(&m.obj)
        {
            let receiver = render_receiver(&m.obj);
            let property = format!("{receiver}.{}", prop.sym);
            self.diags.push(diag_at(
                "T04",
                self.ctx,
                c.span.lo,
                property,
                "dom",
                "direct DOM mutation inside a component bypasses the reactivity \
                 system — use `ref()` for element handles.",
            ));
        }
        c.visit_children_with(self);
    }

    fn visit_assign_expr(&mut self, a: &AssignExpr) {
        // Heuristic (b'): receiver.innerHTML = ...
        if let AssignTarget::Simple(SimpleAssignTarget::Member(m)) = &a.left
            && let MemberProp::Ident(prop) = &m.prop
            && prop.sym.as_str() == "innerHTML"
            && !receiver_is_ref_el(&m.obj)
        {
            let receiver = render_receiver(&m.obj);
            let property = format!("{receiver}.innerHTML");
            self.diags.push(diag_at(
                "T04",
                self.ctx,
                a.span.lo,
                property,
                "dom",
                "direct DOM mutation inside a component bypasses the reactivity \
                 system — use `ref()` for element handles.",
            ));
        }
        a.visit_children_with(self);
    }
}

fn leftmost_is_document(e: &Expr) -> bool {
    let mut cur = e;
    loop {
        match cur {
            Expr::Member(m) => cur = m.obj.as_ref(),
            Expr::Ident(i) => return i.sym.as_str() == "document",
            _ => return false,
        }
    }
}

fn receiver_is_ref_el(e: &Expr) -> bool {
    let Expr::Member(inner) = e else {
        return false;
    };
    let MemberProp::Ident(p) = &inner.prop else {
        return false;
    };
    p.sym.as_str() == "el"
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
    fn fires_on_document_query_selector() {
        let d = run_at(
            "function C(){ document.querySelector('.x'); }",
            "/tmp/src/components/A.ts",
        );
        assert!(d.iter().any(|x| x.rule == "T04"), "expected T04 in {d:?}");
    }

    #[test]
    fn fires_on_document_get_element_by_id() {
        let d = run_at(
            "function C(){ document.getElementById('x'); }",
            "/tmp/src/components/A.ts",
        );
        assert!(d.iter().any(|x| x.rule == "T04"));
    }

    #[test]
    fn fires_on_append_child_without_ref() {
        let d = run_at(
            "function C(){ containerEl.appendChild(child); }",
            "/tmp/src/components/A.ts",
        );
        assert!(d.iter().any(|x| x.rule == "T04"));
    }

    #[test]
    fn does_not_fire_on_ref_dot_el_append_child() {
        let d = run_at(
            "function C(){ myRef.el.appendChild(child); }",
            "/tmp/src/components/A.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_on_inner_html_assignment_without_ref() {
        let d = run_at(
            "function C(){ el.innerHTML = 'x'; }",
            "/tmp/src/components/A.ts",
        );
        assert!(d.iter().any(|x| x.rule == "T04"));
    }

    #[test]
    fn does_not_fire_in_lib_directory() {
        let d = run_at(
            "function f(){ document.querySelector('.x'); }",
            "/tmp/src/lib/util.ts",
        );
        assert!(d.is_empty());
    }

    #[test]
    fn does_not_fire_in_test_file() {
        let d = run_at(
            "function f(){ document.querySelector('.x'); }",
            "/tmp/src/components/A.test.ts",
        );
        assert!(d.is_empty());
    }
}
