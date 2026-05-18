//! Collect every `--name: …` declaration across the project so L13 can
//! tell `var(--name)` from `var(--typo)`.
//!
//! Walks both user SCSS (via [`crate::walk::user_scss_files`]) and the
//! framework's own SCSS under `.zero/styles/` (which the user walker
//! intentionally skips). Returns the union of declared custom-property
//! names — every `--*` is fair game whether it was declared in
//! `_tokens.scss`, a theme partial, or a user override in `styles/app.scss`.

use crate::scan;
use crate::walk::user_scss_files;
use std::collections::HashSet;
use std::path::Path;

/// Set of `--name` strings (including the leading `--`) declared anywhere
/// in the project's compiled SCSS. The L13 rule uses this to validate
/// every `var(--name)` reference in user code.
pub fn collect_defined_vars(root: &Path) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for file in user_scss_files(root) {
        absorb_file(&file, &mut out);
    }
    for file in framework_scss_files(root) {
        absorb_file(&file, &mut out);
    }
    out
}

fn absorb_file(path: &Path, out: &mut HashSet<String>) {
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let (decls, _) = scan::scan(&source);
    for d in decls {
        if d.property.starts_with("--") {
            out.insert(d.property);
        }
    }
}

/// Walk `.zero/styles/**/*.scss`. The user walker excludes `.zero/`
/// unconditionally; this helper bypasses that for the var-collection pass.
fn framework_scss_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let dir = root.join(".zero").join("styles");
    if !dir.is_dir() {
        return out;
    }
    walk_dir(&dir, &mut out);
    out
}

fn walk_dir(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out);
        } else if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("scss") | Some("css")
        ) {
            out.push(path);
        }
    }
}

/// Extract every `var(--name…)` reference from a CSS value. Captures the
/// `--name` part only (no surrounding `var(` or any fallback after `,`).
pub fn extract_var_refs(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 4] == b"var(" {
            i += 4;
            // Skip whitespace.
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            // Capture `--ident`.
            if i + 2 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
                let start = i;
                i += 2;
                while i < bytes.len() && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                if let Ok(name) = std::str::from_utf8(&bytes[start..i]) {
                    out.push(name.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_var_refs_single() {
        assert_eq!(extract_var_refs("var(--space-md)"), vec!["--space-md"]);
    }

    #[test]
    fn extract_var_refs_multiple_with_fallback() {
        let refs = extract_var_refs("calc(var(--a) + var(--b, 4px))");
        assert_eq!(refs, vec!["--a", "--b"]);
    }

    #[test]
    fn extract_var_refs_none() {
        assert!(extract_var_refs("12px solid red").is_empty());
    }
}
