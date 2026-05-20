//! T02 — unknown `@event.modifier` inside `html\`\`` templates.
//!
//! SOURCE OF TRUTH: `runtime/template.js` — see `_wrapEventHandler` and
//! the `KEY_MODIFIERS` map. Unknown modifiers are silently dropped at
//! runtime (the handler still fires for every event), so a typo
//! becomes invisible until someone notices the modifier didn't take.

use regex::Regex;
use std::sync::OnceLock;
use swc_core::ecma::ast::{Expr, TaggedTpl};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::Diagnostic;
use crate::js::context::FileCtx;
use crate::js::diag::diag_at;

/// Allowed modifier set as supported by `runtime/template.js`.
const ALLOWED_MODIFIERS: &[&str] = &[
    "prevent", "stop", "once", "throttle", "debounce", "enter", "escape", "space", "tab", "up",
    "down", "left", "right",
];

fn modifier_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"@(\w+)((?:\.\w+)+)\s*=").unwrap())
}

/// Run T02 over `ctx.module`.
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if !ctx.is_under_components_or_routes || ctx.is_test_file {
        return Vec::new();
    }
    let mut v = T02Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}

struct T02Visitor<'a, 'b> {
    ctx: &'a FileCtx<'b>,
    diags: Vec<Diagnostic>,
}

impl<'a, 'b> Visit for T02Visitor<'a, 'b> {
    fn visit_tagged_tpl(&mut self, t: &TaggedTpl) {
        if let Expr::Ident(i) = t.tag.as_ref()
            && i.sym.as_str() == "html"
        {
            for quasi in &t.tpl.quasis {
                let raw = quasi.raw.as_str();
                for cap in modifier_regex().captures_iter(raw) {
                    let modifiers_group = cap.get(2).expect("modifier group");
                    let segment_start_in_raw = modifiers_group.start();
                    let modifiers_text = modifiers_group.as_str();
                    let mut cursor = 0usize;
                    for seg in modifiers_text.split('.') {
                        if seg.is_empty() {
                            cursor += 1;
                            continue;
                        }
                        let seg_offset_in_group = cursor;
                        if !ALLOWED_MODIFIERS.contains(&seg) {
                            let dot_offset_in_raw = segment_start_in_raw + seg_offset_in_group - 1;
                            let pos =
                                quasi.span.lo + swc_core::common::BytePos(dot_offset_in_raw as u32);
                            let property = format!(".{seg}");
                            self.diags.push(diag_at(
                                "T02",
                                self.ctx,
                                pos,
                                property,
                                "modifier",
                                format!(
                                    "unknown event modifier `.{seg}` — see §3 \
                                     'Event Handling' in the spec for the supported set."
                                ),
                            ));
                        }
                        cursor += seg.len() + 1; // +1 for the '.'
                    }
                }
            }
        }
        t.visit_children_with(self);
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
        run_at(source, "/tmp/src/components/A.ts")
    }

    #[test]
    fn fires_on_unknown_modifier() {
        let d = run("const x = html`<button @click.foo=${h}></button>`;");
        assert_eq!(d.len(), 1, "expected 1, got {d:?}");
        assert_eq!(d[0].rule, "T02");
        assert_eq!(d[0].property, ".foo");
    }

    #[test]
    fn does_not_fire_on_known_modifier() {
        let d = run("const x = html`<button @click.prevent=${h}></button>`;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn does_not_fire_on_no_modifier() {
        let d = run("const x = html`<button @click=${h}></button>`;");
        assert!(d.is_empty(), "expected none, got {d:?}");
    }

    #[test]
    fn fires_on_unknown_in_multi_modifier() {
        let d = run("const x = html`<input @keydown.enter.foo=${h}/>`;");
        assert_eq!(d.len(), 1, "expected only .foo, got {d:?}");
        assert_eq!(d[0].property, ".foo");
    }

    #[test]
    fn column_points_at_the_dot_of_bad_modifier() {
        // Find expected dot position from source.
        let src = "const x = html`<button @click.foo=${h}></button>`;";
        let d = run(src);
        assert_eq!(d.len(), 1);
        // Compute expected 1-based column of the dot before `foo`.
        let bytes = src.as_bytes();
        let mut dot_col = 0u32;
        for (idx, b) in bytes.iter().enumerate() {
            if *b == b'.' && bytes.get(idx + 1) == Some(&b'f') {
                dot_col = (idx as u32) + 1;
                break;
            }
        }
        assert!(dot_col > 0, "couldn't locate dot in source");
        assert_eq!(d[0].column, dot_col, "column mismatch: {:?}", d[0]);
    }

    #[test]
    fn does_not_fire_in_test_file() {
        let d = run_at(
            "const x = html`<button @click.foo=${h}></button>`;",
            "/tmp/src/components/A.test.ts",
        );
        assert!(d.is_empty(), "expected none, got {d:?}");
    }
}
