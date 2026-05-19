//! The embedded zero runtime, concatenated at compile time from `runtime/*.js`.

/// Cleaned runtime body (imports stripped, exports converted to plain declarations,
/// the `export { createScope as _createScope }` alias flattened).
pub const ZERO_RUNTIME_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_runtime_body.js"));

/// Cleaned DOM shim body (imports stripped, exports flattened).
pub const ZERO_DOM_SHIM_BODY: &str =
    include_str!(concat!(env!("OUT_DIR"), "/zero_dom_shim_body.js"));

/// Cleaned test API body (imports stripped, exports flattened).
pub const ZERO_TEST_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_test_body.js"));

/// TypeScript declarations for the `"zero"` module, embedded verbatim from
/// `runtime/zero.d.ts`.
pub const ZERO_TYPES_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_types_body.d.ts"));

/// TypeScript declarations for the `"zero/test"` module, embedded verbatim
/// from `runtime/zero-test.d.ts`.
pub const ZERO_TEST_TYPES_BODY: &str =
    include_str!(concat!(env!("OUT_DIR"), "/zero_test_types_body.d.ts"));

/// Cleaned http module body (imports stripped, exports flattened). Synthetic
/// module served as `/zero-http.js` in dev and inlined by the bundler.
pub const ZERO_HTTP_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_http_body.js"));

/// TypeScript declarations for the `"zero/http"` module, embedded verbatim
/// from `runtime/zero-http.d.ts`.
pub const ZERO_HTTP_TYPES_BODY: &str =
    include_str!(concat!(env!("OUT_DIR"), "/zero_http_types_body.d.ts"));

/// Public names re-exported by the concatenated runtime.
pub const ZERO_RUNTIME_EXPORTS: &[&str] = &[
    "signal",
    "computed",
    "effect",
    "html",
    "commit",
    "each",
    "ref",
    "App",
    "inject",
    "navigate",
    "back",
    "forward",
    "route",
    // Internals needed by the test API; underscore prefix signals "not public API".
    "_setCurrentApp",
    "_createScope",
    "_getCurrentApp",
];

/// Names exported by the `zero/http` module.
pub const ZERO_HTTP_EXPORTS: &[&str] = &["createHttp", "HttpError"];

/// Names exported by the `zero/test` module.
pub const ZERO_TEST_EXPORTS: &[&str] = &[
    "describe",
    "it",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "expect",
    "render",
    "find",
    "findAll",
    "text",
    "fire",
    "cleanup",
    "spy",
    "__getTestTree__",
    "__resetTestTree__",
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

/// Compose `ZERO_HTTP_BODY` with a trailing `export { ... }` block of the
/// public names. The module is self-contained — no imports needed.
///
/// # Returns
/// A complete ES module string ready to register under `"zero/http"`.
pub fn http_module() -> String {
    let mut s = String::from(ZERO_HTTP_BODY);
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("export { ");
    s.push_str(&ZERO_HTTP_EXPORTS.join(", "));
    s.push_str(" };\n");
    s
}

/// Build the `zero/test` module string: import runtime helpers from `"zero"`,
/// then the test body, then a trailing `export { ... }` for the test exports.
///
/// Importing from `"zero"` rather than re-embedding the runtime body ensures
/// that `_currentApp` (and other mutable runtime state) is shared between
/// `"zero"` and `"zero/test"` within the same Boa context. This is necessary
/// for `render()` / `inject()` to cooperate when test components import from
/// `"zero"`.
///
/// # Returns
/// A complete ES module string ready to register under `"zero/test"`.
pub fn test_module() -> String {
    let mut s = String::from("import { _setCurrentApp, _createScope, commit } from \"zero\";\n");
    s.push_str(ZERO_TEST_BODY);
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("export { ");
    s.push_str(&ZERO_TEST_EXPORTS.join(", "));
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
    fn runtime_export_block_contains_internal_names() {
        let m = runtime_module();
        let last_export = m.rfind("export {").expect("expected an `export {` block");
        for name in ["_setCurrentApp", "_createScope", "_getCurrentApp"] {
            assert!(
                m[last_export..].contains(name),
                "trailing export block should mention internal `{name}`"
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

    #[test]
    fn zero_dom_shim_body_contains_create_element_and_installs_global() {
        assert!(
            ZERO_DOM_SHIM_BODY.contains("function createElement("),
            "ZERO_DOM_SHIM_BODY should contain `function createElement(`"
        );
        assert!(
            ZERO_DOM_SHIM_BODY.contains("globalThis.document = document"),
            "ZERO_DOM_SHIM_BODY should install globalThis.document"
        );
    }

    #[test]
    fn zero_dom_shim_body_concatenates_web_platform_sibling_files() {
        for f in [
            "fetch-shim.js",
            "url-shim.js",
            "encoding-shim.js",
            "binary-shim.js",
            "clone-shim.js",
        ] {
            let needle = format!("/* === {f} === */");
            assert!(
                ZERO_DOM_SHIM_BODY.contains(&needle),
                "ZERO_DOM_SHIM_BODY should contain separator `{needle}`"
            );
        }
    }

    #[test]
    fn test_module_contains_describe_and_expect() {
        let m = test_module();
        assert!(
            m.contains("function describe("),
            "missing `function describe(`"
        );
        assert!(m.contains("function expect("), "missing `function expect(`");
    }

    #[test]
    fn zero_types_body_declares_every_public_runtime_export() {
        for name in ZERO_RUNTIME_EXPORTS {
            if name.starts_with('_') {
                continue;
            }
            assert!(
                ZERO_TYPES_BODY.contains(name),
                "zero.d.ts missing declaration for runtime export `{name}`"
            );
        }
    }

    #[test]
    fn zero_test_types_body_declares_every_public_test_export() {
        for name in ZERO_TEST_EXPORTS {
            if name.starts_with('_') {
                continue;
            }
            assert!(
                ZERO_TEST_TYPES_BODY.contains(name),
                "zero-test.d.ts missing declaration for test export `{name}`"
            );
        }
    }

    #[test]
    fn zero_types_body_contains_signal_app_html_route() {
        for needle in ["signal", "class App", "function html", "function route"] {
            assert!(
                ZERO_TYPES_BODY.contains(needle),
                "zero.d.ts spot-check missing: {needle}"
            );
        }
    }

    #[test]
    fn zero_types_body_declares_state_types_registry() {
        assert!(
            ZERO_TYPES_BODY.contains("interface StateTypes"),
            "zero.d.ts should declare `interface StateTypes` for the typed inject registry"
        );
        assert!(
            ZERO_TYPES_BODY.contains("inject<K extends keyof StateTypes>"),
            "zero.d.ts should declare the typed inject overload `inject<K extends keyof StateTypes>`"
        );
    }

    #[test]
    fn test_module_ends_with_aggregate_export_block_for_test_exports() {
        let m = test_module();
        let last_export = m.rfind("export {").expect("expected an `export {` block");
        for name in ZERO_TEST_EXPORTS {
            assert!(
                m[last_export..].contains(name),
                "trailing export block should mention `{name}`"
            );
        }
    }

    #[test]
    fn http_module_contains_create_http_factory() {
        let m = http_module();
        assert!(
            m.contains("function createHttp("),
            "expected http module to contain `function createHttp(`"
        );
        assert!(
            m.contains("class HttpError"),
            "expected http module to contain `class HttpError`"
        );
    }

    #[test]
    fn http_module_ends_with_aggregate_export_block() {
        let m = http_module();
        let last_export = m.rfind("export {").expect("expected an `export {` block");
        for name in ZERO_HTTP_EXPORTS {
            assert!(
                m[last_export..].contains(name),
                "trailing http export block should mention `{name}`"
            );
        }
    }

    #[test]
    fn zero_http_types_body_declares_every_public_export() {
        for name in ZERO_HTTP_EXPORTS {
            assert!(
                ZERO_HTTP_TYPES_BODY.contains(name),
                "zero-http.d.ts missing declaration for http export `{name}`"
            );
        }
    }

    #[test]
    fn http_body_has_no_top_level_imports() {
        let re = regex::Regex::new(r"(?m)^\s*import\s").unwrap();
        assert!(
            !re.is_match(ZERO_HTTP_BODY),
            "ZERO_HTTP_BODY should not contain a top-level `import` statement"
        );
    }
}
