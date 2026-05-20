//! R01 — `${signal.val}` inside `html\`\`` tagged-template substitutions.
//!
//! Reading `.val` inside a template substitution captures the *current*
//! value at render time; the renderer never re-subscribes when the
//! signal changes, so the UI silently drifts. Pass the signal itself
//! and let the runtime track the dependency.

use swc_core::ecma::ast::{Expr, MemberProp, TaggedTpl};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Run R01 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    let mut v = R01Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct R01Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for R01Visitor<'a, 'b> {
    fn visit_tagged_tpl(&mut self, t: &TaggedTpl) {
        if is_html_tag(&t.tag) {
            for expr in &t.tpl.exprs {
                if let Some((obj_name, member_lo)) = val_read_on_ident(expr) {
                    let property = format!("{obj_name}.val");
                    self.diags.push(diag_at(
                        "R01",
                        self.ctx,
                        member_lo,
                        property,
                        "template",
                        "reading `.val` inside a template breaks reactivity — \
                         pass the signal itself: `${name}` not `${name.val}`.",
                    ));
                }
            }
        }
        // Recurse to catch nested `html\`...\`` composed templates.
        t.visit_children_with(self);
    }
}

fn is_html_tag(tag: &Expr) -> bool {
    matches!(tag, Expr::Ident(i) if i.sym.as_str() == "html")
}

fn val_read_on_ident(expr: &Expr) -> Option<(String, swc_core::common::BytePos)> {
    let Expr::Member(m) = expr else {
        return None;
    };
    let MemberProp::Ident(prop) = &m.prop else {
        return None;
    };
    if prop.sym.as_str() != "val" {
        return None;
    }
    let Expr::Ident(obj) = m.obj.as_ref() else {
        return None;
    };
    Some((obj.sym.to_string(), m.span.lo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use zero_transpile::parse_module;

    fn run(source: &str) -> Vec<Diagnostic> {
        let parsed = parse_module(source, "x.ts").expect("parse");
        let file = PathBuf::from("/tmp/src/app.ts");
        let root = PathBuf::from("/tmp");
        let ctx = FileCtx::new(&file, source, &root, parsed.source_map, parsed.module);
        check(&ctx)
    }

    #[test]
    fn fires_on_val_read_in_html_template() {
        let d = run("const x = html`${count.val}`;");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "R01");
        assert_eq!(d[0].property, "count.val");
    }

    #[test]
    fn does_not_fire_on_signal_pass() {
        let d = run("const x = html`${count}`;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_outside_html_tag() {
        let d = run("const x = css`${x.val}`;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_on_val_outside_template() {
        let d = run("const v = count.val;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_in_nested_html_template() {
        let d = run("const x = html`${html`${y.val}`}`;");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].property, "y.val");
    }
}
