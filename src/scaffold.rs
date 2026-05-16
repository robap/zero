//! Embedded scaffold templates and the write functions that materialize
//! them into a target directory.

use std::fs;
use std::path::{Path, PathBuf};

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
const TPL_TOKENS_SCSS: &str = include_str!("scaffold/.zero/styles/_tokens.scss");
const TPL_BASE_SCSS: &str = include_str!("scaffold/.zero/styles/_base.scss");
const TPL_LAYOUT_SCSS: &str = include_str!("scaffold/.zero/styles/_layout.scss");
const TPL_UTILITIES_SCSS: &str = include_str!("scaffold/.zero/styles/_utilities.scss");
const TPL_ALIGNMENT_SCSS: &str = include_str!("scaffold/.zero/styles/_alignment.scss");
const TPL_ZERO_SCSS: &str = include_str!("scaffold/.zero/styles/zero.scss");
const TPL_AGENTS_MD: &str = include_str!("scaffold/AGENTS.md");
// Inlined rather than `include_str!`'d from `src/scaffold/.gitignore` because
// that path's `.zero/` rule would cause this repo's git to ignore
// `src/scaffold/.zero/`, preventing the framework SCSS partials from being
// tracked.
const TPL_GITIGNORE: &str = ".zero/\ndist/\n";

/// An operation `zero update` (or `zero init`) intends to apply to a path
/// under the project root. Paths are always relative to the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// File does not exist on disk; will be created.
    Add(PathBuf),
    /// File exists but its contents differ from the manifest; will be overwritten.
    Update(PathBuf),
    /// File exists on disk but is not in the manifest; will be deleted.
    Remove(PathBuf),
}

/// Returns the canonical list of framework template files written into
/// `.zero/`. Each tuple is `(relative_path, content)`. Both `zero init`
/// (initial write) and `zero update` (diff + refresh) consult this single
/// source of truth.
///
/// # Returns
/// A vector of `(relative path, file contents)` pairs.
pub fn framework_manifest() -> Vec<(&'static str, &'static str)> {
    vec![
        (".zero/zero.d.ts", crate::runtime::ZERO_TYPES_BODY),
        (".zero/zero-test.d.ts", crate::runtime::ZERO_TEST_TYPES_BODY),
        (".zero/styles/_tokens.scss", TPL_TOKENS_SCSS),
        (".zero/styles/_base.scss", TPL_BASE_SCSS),
        (".zero/styles/_layout.scss", TPL_LAYOUT_SCSS),
        (".zero/styles/_utilities.scss", TPL_UTILITIES_SCSS),
        (".zero/styles/_alignment.scss", TPL_ALIGNMENT_SCSS),
        (".zero/styles/zero.scss", TPL_ZERO_SCSS),
    ]
}

/// Write the user-owned, one-shot scaffold files into `root_dir`. Includes
/// `.gitignore`.
///
/// # Parameters
/// - `root_dir`: directory to write into. Must already exist.
/// - `ctx`: substitution context for templates that need it.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
fn write_user_files(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()> {
    fs::create_dir_all(root_dir.join("src").join("routes"))?;
    fs::create_dir_all(root_dir.join("styles"))?;

    let index_html = TPL_INDEX_HTML.replace("{{title}}", &ctx.title);
    fs::write(root_dir.join("index.html"), index_html)?;
    fs::write(root_dir.join("tsconfig.json"), TPL_TSCONFIG_JSON)?;
    fs::write(root_dir.join("src").join("app.ts"), TPL_APP_TS)?;
    fs::write(
        root_dir.join("src").join("routes").join("home.ts"),
        TPL_HOME_TS,
    )?;
    fs::write(
        root_dir.join("src").join("routes").join("home.test.ts"),
        TPL_HOME_TEST_TS,
    )?;
    fs::write(root_dir.join("styles").join("app.scss"), TPL_APP_SCSS)?;
    fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;
    fs::write(root_dir.join(".gitignore"), TPL_GITIGNORE)?;
    Ok(())
}

/// Write every file listed in `framework_manifest()` into `root_dir`,
/// overwriting any existing content. Creates `.zero/` (and nested
/// directories) as needed.
///
/// # Parameters
/// - `root_dir`: the project root.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub fn write_framework_files(root_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root_dir.join(".zero").join("styles"))?;
    for (rel, content) in framework_manifest() {
        let abs = root_dir.join(rel);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&abs, content)?;
    }
    Ok(())
}

/// Write the full scaffold (user files + framework files) into `root_dir`.
/// Used by `zero init`.
///
/// # Parameters
/// - `root_dir`: directory to write into. Created if missing.
/// - `ctx`: substitution context.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub fn write_initial_project(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()> {
    fs::create_dir_all(root_dir)?;
    write_user_files(root_dir, ctx)?;
    write_framework_files(root_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use tempfile::tempdir;

    fn fresh_scaffold() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "My zero app".to_string(),
        };
        write_initial_project(&root, &ctx).unwrap();
        (dir, root)
    }

    #[test]
    fn write_initial_project_emits_user_files() {
        let (_dir, root) = fresh_scaffold();

        let index = fs::read_to_string(root.join("index.html")).unwrap();
        assert!(index.contains("<title>My zero app</title>"));

        let app_ts = fs::read_to_string(root.join("src/app.ts")).unwrap();
        assert!(app_ts.contains("new App()"));

        let home_ts = fs::read_to_string(root.join("src/routes/home.ts")).unwrap();
        assert!(home_ts.contains("Hello from zero"));
        assert!(
            home_ts.contains("class=\"stack pad-xl align-center\""),
            "home.ts missing design-system classes: {home_ts}"
        );
        assert!(
            home_ts.contains("align-center"),
            "home.ts missing alignment demo class: {home_ts}"
        );

        let home_test_ts = fs::read_to_string(root.join("src/routes/home.test.ts")).unwrap();
        assert!(!home_test_ts.is_empty());

        let tsconfig = fs::read_to_string(root.join("tsconfig.json")).unwrap();
        assert!(tsconfig.contains("\"strict\": true"));
        assert!(tsconfig.contains("\"allowImportingTsExtensions\": true"));

        let app_scss = fs::read_to_string(root.join("styles/app.scss")).unwrap();
        assert!(!app_scss.is_empty());

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(!agents.is_empty());
    }

    #[test]
    fn write_initial_project_emits_framework_files() {
        let (_dir, root) = fresh_scaffold();

        let zero_dts = fs::read_to_string(root.join(".zero/zero.d.ts")).unwrap();
        assert!(zero_dts.contains("declare module \"zero\""));

        let zero_test_dts = fs::read_to_string(root.join(".zero/zero-test.d.ts")).unwrap();
        assert!(zero_test_dts.contains("declare module \"zero/test\""));

        let tokens_scss = fs::read_to_string(root.join(".zero/styles/_tokens.scss")).unwrap();
        assert!(tokens_scss.contains("--color-primary:"));

        let base_scss = fs::read_to_string(root.join(".zero/styles/_base.scss")).unwrap();
        assert!(!base_scss.is_empty());

        let layout_scss = fs::read_to_string(root.join(".zero/styles/_layout.scss")).unwrap();
        assert!(!layout_scss.is_empty());

        let utilities_scss = fs::read_to_string(root.join(".zero/styles/_utilities.scss")).unwrap();
        assert!(!utilities_scss.is_empty());

        let alignment_scss = fs::read_to_string(root.join(".zero/styles/_alignment.scss")).unwrap();
        assert!(!alignment_scss.is_empty());

        let zero_scss = fs::read_to_string(root.join(".zero/styles/zero.scss")).unwrap();
        assert!(!zero_scss.is_empty());
    }

    #[test]
    fn write_initial_project_emits_gitignore_with_zero_dir() {
        let (_dir, root) = fresh_scaffold();
        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(
            gitignore.lines().any(|l| l.trim() == ".zero/"),
            ".gitignore missing `.zero/` line: {gitignore}"
        );
    }

    #[test]
    fn write_initial_project_app_ts_imports_zero() {
        let (_dir, root) = fresh_scaffold();
        let app_ts = fs::read_to_string(root.join("src/app.ts")).unwrap();
        assert!(app_ts.contains("import { App, signal } from \"zero\""));
    }

    #[test]
    fn write_initial_project_agents_md_has_section_sentinels() {
        let (_dir, root) = fresh_scaffold();

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
            "## The .zero/ directory",
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
    fn write_initial_project_emits_home_test_ts() {
        let (_dir, root) = fresh_scaffold();
        let test_ts = fs::read_to_string(root.join("src/routes/home.test.ts")).unwrap();
        assert!(test_ts.contains(r#"import { describe, it, expect"#));
        assert!(test_ts.contains(r#"from "zero/test""#));
    }

    #[test]
    fn write_initial_project_index_html_links_to_scss() {
        let (_dir, root) = fresh_scaffold();
        let index = fs::read_to_string(root.join("index.html")).unwrap();
        assert!(
            index.contains(r#"<link rel="stylesheet" href="/styles/app.scss">"#),
            "index.html doesn't link to app.scss: {index}"
        );
    }

    #[test]
    fn tokens_scss_declares_tokens_directly() {
        let (_dir, root) = fresh_scaffold();
        let tokens = fs::read_to_string(root.join(".zero/styles/_tokens.scss")).unwrap();
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

    #[test]
    fn app_scss_imports_framework_aggregate() {
        let (_dir, root) = fresh_scaffold();
        let app_scss = fs::read_to_string(root.join("styles/app.scss")).unwrap();
        assert!(
            app_scss.contains("@use '../.zero/styles/zero'"),
            "app.scss missing aggregate @use: {app_scss}"
        );
    }

    #[test]
    fn zero_scss_contains_aggregate_uses() {
        let (_dir, root) = fresh_scaffold();
        let zero_scss = fs::read_to_string(root.join(".zero/styles/zero.scss")).unwrap();
        for needle in [
            "@use 'tokens'",
            "@use 'base'",
            "@use 'layout'",
            "@use 'utilities'",
            "@use 'alignment'",
        ] {
            assert!(
                zero_scss.contains(needle),
                "zero.scss missing {needle}: {zero_scss}"
            );
        }
    }

    #[test]
    fn tsconfig_include_points_at_dot_zero() {
        let (_dir, root) = fresh_scaffold();
        let tsconfig = fs::read_to_string(root.join("tsconfig.json")).unwrap();
        assert!(
            tsconfig.contains(".zero/zero.d.ts"),
            "tsconfig.json missing .zero/zero.d.ts in include: {tsconfig}"
        );
        assert!(
            tsconfig.contains(".zero/zero-test.d.ts"),
            "tsconfig.json missing .zero/zero-test.d.ts in include: {tsconfig}"
        );
        let include_line = tsconfig
            .lines()
            .find(|l| l.contains("\"include\""))
            .expect("no include line found");
        assert!(
            !include_line.contains("\"zero.d.ts\""),
            "tsconfig include still references bare zero.d.ts: {include_line}"
        );
        assert!(
            !include_line.contains("\"zero-test.d.ts\""),
            "tsconfig include still references bare zero-test.d.ts: {include_line}"
        );
    }

    #[test]
    fn alignment_scss_contains_each_family() {
        let (_dir, root) = fresh_scaffold();
        let alignment = fs::read_to_string(root.join(".zero/styles/_alignment.scss")).unwrap();
        assert!(!alignment.is_empty(), "_alignment.scss is empty");
        for needle in [
            ".align-start ",
            ".justify-between ",
            ".align-self-stretch ",
            ".justify-self-center ",
            ".text-center ",
            ".flex-col-reverse ",
        ] {
            assert!(
                alignment.contains(needle),
                "_alignment.scss missing {needle}: {alignment}"
            );
        }
        assert!(
            !alignment.contains("!important"),
            "_alignment.scss must not use !important: {alignment}"
        );
    }

    #[test]
    fn framework_manifest_lists_eight_files() {
        let manifest = framework_manifest();
        assert_eq!(manifest.len(), 8, "manifest should have 8 entries");
        let paths: BTreeSet<&str> = manifest.iter().map(|(p, _)| *p).collect();
        for expected in [
            ".zero/zero.d.ts",
            ".zero/zero-test.d.ts",
            ".zero/styles/_tokens.scss",
            ".zero/styles/_base.scss",
            ".zero/styles/_layout.scss",
            ".zero/styles/_utilities.scss",
            ".zero/styles/_alignment.scss",
            ".zero/styles/zero.scss",
        ] {
            assert!(paths.contains(expected), "missing {expected} from manifest");
        }
    }

    #[test]
    fn write_framework_files_writes_only_dot_zero() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        fs::create_dir_all(&root).unwrap();
        write_framework_files(&root).unwrap();

        for (rel, _) in framework_manifest() {
            assert!(
                root.join(rel).exists(),
                "framework file missing after write: {rel}"
            );
        }

        let entries: Vec<_> = fs::read_dir(&root).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "write_framework_files wrote outside .zero/: {entries:?}"
        );
        let only = entries.into_iter().next().unwrap().unwrap();
        assert_eq!(only.file_name(), ".zero");
    }
}
