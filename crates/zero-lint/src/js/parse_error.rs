//! P01: SWC parse errors surfaced as one diagnostic per file.

use std::path::Path;

use crate::Diagnostic;
use crate::js::context::FileCtx;

/// Parse `source` and return either a fully-built [`FileCtx`] or a single
/// `P01` diagnostic describing the parse failure.
///
/// # Parameters
/// - `file`: absolute path of the file being parsed.
/// - `source`: file contents.
/// - `root`: project root (the directory containing `src/`).
pub fn check<'a>(
    file: &'a Path,
    source: &'a str,
    root: &'a Path,
) -> Result<FileCtx<'a>, Diagnostic> {
    let filename = file.to_string_lossy().to_string();
    match zero_transpile::parse_module(source, &filename) {
        Ok(parsed) => Ok(FileCtx::new(
            file,
            source,
            root,
            parsed.source_map,
            parsed.module,
        )),
        Err(err) => Err(Diagnostic {
            rule: "P01",
            file: file.to_path_buf(),
            line: err.line.max(1),
            column: err.column.max(1),
            property: String::new(),
            value: "parse".to_string(),
            message: err.message,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn valid_file_returns_ctx() {
        let file = PathBuf::from("/tmp/src/app.ts");
        let root = PathBuf::from("/tmp");
        let ctx = check(&file, "export const x = 1;", &root).expect("ok");
        assert_eq!(ctx.module.body.len(), 1);
    }

    #[test]
    fn parse_error_emits_p01() {
        let file = PathBuf::from("/tmp/src/bad.ts");
        let root = PathBuf::from("/tmp");
        let diag = match check(&file, "const x: = ;", &root) {
            Ok(_) => panic!("expected err"),
            Err(d) => d,
        };
        assert_eq!(diag.rule, "P01");
        assert!(diag.line >= 1);
        assert!(diag.column >= 1);
        assert!(!diag.message.is_empty());
        assert_eq!(diag.value, "parse");
    }
}
