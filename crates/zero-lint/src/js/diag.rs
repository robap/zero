//! Helpers for emitting `Diagnostic` values from SWC spans.

use zero_transpile::BytePos;

use crate::Diagnostic;
use crate::js::context::FileCtx;

/// Build a `Diagnostic` at `pos`.
///
/// Translates `pos` to a 1-based `(line, column)` pair via
/// `ctx.source_map`, matching the existing SCSS rules' position scheme
/// (and the `write_diag` helper's expectations).
pub fn diag_at(
    rule: &'static str,
    ctx: &FileCtx<'_>,
    pos: BytePos,
    property: impl Into<String>,
    value: impl Into<String>,
    message: impl Into<String>,
) -> Diagnostic {
    let loc = ctx.source_map.lookup_char_pos(pos);
    Diagnostic {
        rule,
        file: ctx.file.to_path_buf(),
        line: loc.line as u32,
        column: (loc.col_display + 1) as u32,
        property: property.into(),
        value: value.into(),
        message: message.into(),
    }
}
