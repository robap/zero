//! The embedded zero runtime, concatenated at compile time from `runtime/*.js`.

/// Cleaned runtime body (imports stripped, exports converted to plain declarations,
/// the `export { createScope as _createScope }` alias flattened).
pub const ZERO_RUNTIME_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_runtime_body.js"));

/// Public names re-exported by the concatenated runtime.
pub const ZERO_RUNTIME_EXPORTS: &[&str] = &[
    "signal", "computed", "effect", "html", "commit", "each", "ref", "App", "inject", "navigate",
    "back", "forward", "route",
];

/// Compose `ZERO_RUNTIME_BODY` with a trailing `export { ... }` block of the public names.
///
/// # Returns
/// A complete ES module string ready to serve as `/zero.js`.
pub fn runtime_module() -> String {
    let mut s = String::from(ZERO_RUNTIME_BODY);
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("export { ");
    s.push_str(&ZERO_RUNTIME_EXPORTS.join(", "));
    s.push_str(" };\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_module_contains_signal_declaration() {
        assert!(
            runtime_module().contains("function signal("),
            "expected runtime module to contain `function signal(`"
        );
    }

    #[test]
    fn runtime_module_contains_class_app() {
        assert!(
            runtime_module().contains("class App"),
            "expected runtime module to contain `class App`"
        );
    }

    #[test]
    fn runtime_module_contains_html_commit_navigate_route() {
        let m = runtime_module();
        for needle in [
            "function html(",
            "function commit(",
            "function navigate(",
            "function route(",
        ] {
            assert!(m.contains(needle), "missing: {needle}");
        }
    }

    #[test]
    fn runtime_module_ends_with_aggregate_export_block() {
        let m = runtime_module();
        let trimmed = m.trim_end();
        assert!(
            trimmed.ends_with("};"),
            "expected trailing `}};` from the export block, got tail: {:?}",
            &trimmed[trimmed.len().saturating_sub(80)..]
        );
        let last_export = m.rfind("export {").expect("expected an `export {` block");
        for name in ZERO_RUNTIME_EXPORTS {
            assert!(
                m[last_export..].contains(name),
                "trailing export block should mention `{name}`"
            );
        }
    }

    #[test]
    fn runtime_body_has_no_top_level_imports() {
        let re = regex::Regex::new(r"(?m)^\s*import\s").unwrap();
        assert!(
            !re.is_match(ZERO_RUNTIME_BODY),
            "ZERO_RUNTIME_BODY should not contain a top-level `import` statement"
        );
    }

    #[test]
    fn runtime_body_has_no_intermediate_export_blocks() {
        // The body must contain no `export { ... }` re-export blocks; the
        // aggregate is appended by `runtime_module()`, not baked into the body.
        let re = regex::Regex::new(r"export\s*\{").unwrap();
        assert!(
            !re.is_match(ZERO_RUNTIME_BODY),
            "ZERO_RUNTIME_BODY should not contain any `export {{` block"
        );
    }

    #[test]
    fn runtime_body_flattens_create_scope_alias() {
        assert!(
            ZERO_RUNTIME_BODY.contains("const _createScope = createScope;"),
            "expected alias flattening `const _createScope = createScope;` in body"
        );
    }
}
