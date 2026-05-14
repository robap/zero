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
const TPL_APP_JS: &str = include_str!("scaffold/src/app.js");
const TPL_HOME_JS: &str = include_str!("scaffold/src/routes/home.js");
const TPL_HOME_TEST_JS: &str = include_str!("scaffold/src/routes/home.test.js");
const TPL_APP_CSS: &str = include_str!("scaffold/styles/app.css");
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
    fs::write(root_dir.join("src").join("app.js"), TPL_APP_JS)?;
    fs::write(
        root_dir.join("src").join("routes").join("home.js"),
        TPL_HOME_JS,
    )?;
    fs::write(
        root_dir.join("src").join("routes").join("home.test.js"),
        TPL_HOME_TEST_JS,
    )?;
    fs::write(root_dir.join("styles").join("app.css"), TPL_APP_CSS)?;
    fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_to_emits_all_four_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "My zero app".to_string(),
        };
        write_to(&root, &ctx).unwrap();

        let index = fs::read_to_string(root.join("index.html")).unwrap();
        assert!(index.contains("<title>My zero app</title>"));

        let app_js = fs::read_to_string(root.join("src/app.js")).unwrap();
        assert!(app_js.contains("new App()"));

        let home_js = fs::read_to_string(root.join("src/routes/home.js")).unwrap();
        assert!(home_js.contains("Hello from zero"));

        let css = fs::read_to_string(root.join("styles/app.css")).unwrap();
        assert!(!css.is_empty());

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(
            !agents.is_empty(),
            "AGENTS.md should be written and non-empty"
        );
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
    fn write_to_emits_home_test_js() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "Test app".to_string(),
        };
        write_to(&root, &ctx).unwrap();

        let test_js = fs::read_to_string(root.join("src/routes/home.test.js")).unwrap();
        assert!(
            test_js.contains(r#"import { describe, it, expect"#),
            "home.test.js should import from zero/test"
        );
        assert!(
            test_js.contains(r#"from "zero/test""#),
            "home.test.js should import from zero/test"
        );
    }
}
