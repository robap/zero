//! Embedded scaffold templates and the `write_to` function that materializes
//! them into a target directory.

use std::fs;
use std::path::Path;

/// Substitution context for scaffold templates.
pub struct ScaffoldContext {
    /// Value substituted in for the HTML `<title>` placeholder.
    pub title: String,
}

const TPL_INDEX_HTML: &str = include_str!("scaffold/index.html");
const TPL_APP_TS: &str = include_str!("scaffold/src/app.ts");
const TPL_HOME_TS: &str = include_str!("scaffold/src/routes/home.ts");
const TPL_HOME_TEST_TS: &str = include_str!("scaffold/src/routes/home.test.ts");
const TPL_TSCONFIG_JSON: &str = include_str!("scaffold/tsconfig.json");
const TPL_APP_SCSS: &str = include_str!("scaffold/styles/app.scss");
const TPL_TOKENS_SCSS: &str = include_str!("scaffold/styles/_tokens.scss");
const TPL_BASE_SCSS: &str = include_str!("scaffold/styles/_base.scss");
const TPL_LAYOUT_SCSS: &str = include_str!("scaffold/styles/_layout.scss");
const TPL_UTILITIES_SCSS: &str = include_str!("scaffold/styles/_utilities.scss");
const TPL_AGENTS_MD: &str = include_str!("scaffold/AGENTS.md");

/// Write the embedded scaffold into `root_dir`, performing `{{title}}` substitution.
///
/// # Parameters
/// - `root_dir`: directory to write into. Created if missing.
/// - `ctx`: substitution context.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub fn write_to(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()> {
    fs::create_dir_all(root_dir)?;
    fs::create_dir_all(root_dir.join("src").join("routes"))?;
    fs::create_dir_all(root_dir.join("styles"))?;

    let index_html = TPL_INDEX_HTML.replace("{{title}}", &ctx.title);
    fs::write(root_dir.join("index.html"), index_html)?;
    fs::write(root_dir.join("src").join("app.ts"), TPL_APP_TS)?;
    fs::write(
        root_dir.join("src").join("routes").join("home.ts"),
        TPL_HOME_TS,
    )?;
    fs::write(
        root_dir.join("src").join("routes").join("home.test.ts"),
        TPL_HOME_TEST_TS,
    )?;
    fs::write(root_dir.join("tsconfig.json"), TPL_TSCONFIG_JSON)?;
    fs::write(root_dir.join("zero.d.ts"), crate::runtime::ZERO_TYPES_BODY)?;
    fs::write(
        root_dir.join("zero-test.d.ts"),
        crate::runtime::ZERO_TEST_TYPES_BODY,
    )?;
    fs::write(
        root_dir.join("styles").join("_tokens.scss"),
        TPL_TOKENS_SCSS,
    )?;
    fs::write(root_dir.join("styles").join("_base.scss"), TPL_BASE_SCSS)?;
    fs::write(
        root_dir.join("styles").join("_layout.scss"),
        TPL_LAYOUT_SCSS,
    )?;
    fs::write(
        root_dir.join("styles").join("_utilities.scss"),
        TPL_UTILITIES_SCSS,
    )?;
    fs::write(root_dir.join("styles").join("app.scss"), TPL_APP_SCSS)?;
    fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_to_emits_all_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "My zero app".to_string(),
        };
        write_to(&root, &ctx).unwrap();

        let index = fs::read_to_string(root.join("index.html")).unwrap();
        assert!(index.contains("<title>My zero app</title>"));

        let app_ts = fs::read_to_string(root.join("src/app.ts")).unwrap();
        assert!(app_ts.contains("new App()"));

        let home_ts = fs::read_to_string(root.join("src/routes/home.ts")).unwrap();
        assert!(home_ts.contains("Hello from zero"));
        assert!(
            home_ts.contains("class=\"stack pad-xl\""),
            "home.ts missing design-system classes: {home_ts}"
        );

        let home_test_ts = fs::read_to_string(root.join("src/routes/home.test.ts")).unwrap();
        assert!(!home_test_ts.is_empty());

        let tsconfig = fs::read_to_string(root.join("tsconfig.json")).unwrap();
        assert!(tsconfig.contains("\"strict\": true"));
        assert!(tsconfig.contains("\"allowImportingTsExtensions\": true"));

        let zero_dts = fs::read_to_string(root.join("zero.d.ts")).unwrap();
        assert!(zero_dts.contains("declare module \"zero\""));

        let zero_test_dts = fs::read_to_string(root.join("zero-test.d.ts")).unwrap();
        assert!(zero_test_dts.contains("declare module \"zero/test\""));

        let app_scss = fs::read_to_string(root.join("styles/app.scss")).unwrap();
        assert!(!app_scss.is_empty());
        assert!(
            app_scss.contains("@use 'tokens'"),
            "app.scss missing @use 'tokens'"
        );
        assert!(
            app_scss.contains("@use 'base'"),
            "app.scss missing @use 'base'"
        );
        assert!(
            app_scss.contains("@use 'layout'"),
            "app.scss missing @use 'layout'"
        );
        assert!(
            app_scss.contains("@use 'utilities'"),
            "app.scss missing @use 'utilities'"
        );

        let tokens_scss = fs::read_to_string(root.join("styles/_tokens.scss")).unwrap();
        assert!(!tokens_scss.is_empty());

        let base_scss = fs::read_to_string(root.join("styles/_base.scss")).unwrap();
        assert!(!base_scss.is_empty());

        let layout_scss = fs::read_to_string(root.join("styles/_layout.scss")).unwrap();
        assert!(!layout_scss.is_empty());

        let utilities_scss = fs::read_to_string(root.join("styles/_utilities.scss")).unwrap();
        assert!(!utilities_scss.is_empty());

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(!agents.is_empty());
    }

    #[test]
    fn write_to_app_ts_imports_zero() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "T".to_string(),
        };
        write_to(&root, &ctx).unwrap();
        let app_ts = fs::read_to_string(root.join("src/app.ts")).unwrap();
        assert!(app_ts.contains("import { App, signal } from \"zero\""));
    }

    #[test]
    fn write_to_agents_md_has_section_sentinels() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "Sentinel app".to_string(),
        };
        write_to(&root, &ctx).unwrap();

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        for sentinel in [
            "# Zero — Agent & Developer Reference",
            "## Quick start",
            "## Imports",
            "## Components",
            "## Reactivity",
            "## App configuration",
            "## Routes",
            "## Styles",
            "## Navigation",
            "## App-level state",
            "## Testing",
            "## JSDoc conventions",
            "## Common pitfalls",
        ] {
            assert!(
                agents.contains(sentinel),
                "AGENTS.md is missing section sentinel: {sentinel}"
            );
        }
    }

    #[test]
    fn write_to_emits_home_test_ts() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "Test app".to_string(),
        };
        write_to(&root, &ctx).unwrap();

        let test_ts = fs::read_to_string(root.join("src/routes/home.test.ts")).unwrap();
        assert!(test_ts.contains(r#"import { describe, it, expect"#));
        assert!(test_ts.contains(r#"from "zero/test""#));
    }

    #[test]
    fn write_to_index_html_links_to_scss() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "SCSS app".to_string(),
        };
        write_to(&root, &ctx).unwrap();
        let index = fs::read_to_string(root.join("index.html")).unwrap();
        assert!(
            index.contains(r#"<link rel="stylesheet" href="/styles/app.scss">"#),
            "index.html doesn't link to app.scss: {index}"
        );
    }

    #[test]
    fn tokens_scss_declares_tokens_directly() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "Tokens app".to_string(),
        };
        write_to(&root, &ctx).unwrap();
        let tokens = fs::read_to_string(root.join("styles/_tokens.scss")).unwrap();
        assert!(
            tokens.contains("--color-primary:"),
            "_tokens.scss missing --color-primary: {tokens}"
        );
        assert!(
            !tokens.contains("$color-primary"),
            "_tokens.scss must not contain SCSS variable $color-primary: {tokens}"
        );
        assert!(
            tokens.contains("@media (prefers-color-scheme: dark)"),
            "_tokens.scss missing system-preference dark block: {tokens}"
        );
        assert!(
            tokens.contains("[data-theme=\"dark\"]"),
            "_tokens.scss missing [data-theme=\"dark\"] override: {tokens}"
        );
        assert!(
            tokens.contains("[data-theme=\"light\"]"),
            "_tokens.scss missing [data-theme=\"light\"] override: {tokens}"
        );
    }
}
