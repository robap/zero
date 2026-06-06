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
const TPL_PALETTE_SCSS: &str = include_str!("scaffold/.zero/styles/_palette.scss");
const TPL_TOKENS_SCSS: &str = include_str!("scaffold/.zero/styles/_tokens.scss");
const TPL_THEMES_SCSS: &str = include_str!("scaffold/.zero/styles/_themes.scss");
const TPL_THEME_LIGHT_SCSS: &str = include_str!("scaffold/.zero/styles/themes/_light.scss");
const TPL_THEME_DARK_SCSS: &str = include_str!("scaffold/.zero/styles/themes/_dark.scss");
const TPL_BASE_SCSS: &str = include_str!("scaffold/.zero/styles/_base.scss");
const TPL_LAYOUT_SCSS: &str = include_str!("scaffold/.zero/styles/_layout.scss");
const TPL_UTILITIES_SCSS: &str = include_str!("scaffold/.zero/styles/_utilities.scss");
const TPL_ALIGNMENT_SCSS: &str = include_str!("scaffold/.zero/styles/_alignment.scss");
const TPL_TYPOGRAPHY_SCSS: &str = include_str!("scaffold/.zero/styles/_typography.scss");
const TPL_ZERO_SCSS: &str = include_str!("scaffold/.zero/styles/zero.scss");
const TPL_AGENTS_MD: &str = include_str!("scaffold/AGENTS.md");
const TPL_COMPONENTS_INDEX_TS: &str = include_str!("scaffold/.zero/components/index.ts");
const TPL_COMPONENTS_INTERNAL_TS: &str = include_str!("scaffold/.zero/components/_internal.ts");
const TPL_COMPONENTS_INTERNAL_TEST_TS: &str =
    include_str!("scaffold/.zero/components/_internal.test.ts");
const TPL_FORM_TS: &str = include_str!("scaffold/.zero/components/form.ts");
const TPL_FORM_TEST_TS: &str = include_str!("scaffold/.zero/components/form.test.ts");
const TPL_COMPONENTS_DTS: &str = include_str!("scaffold/.zero/components.d.ts");
const TPL_COMPONENTS_AGGREGATE_SCSS: &str = include_str!("scaffold/.zero/styles/_components.scss");

// Per-component templates. Each component contributes three template
// constants (source, test, SCSS partial) and three manifest entries.
const TPL_AVATAR_TS: &str = include_str!("scaffold/.zero/components/Avatar.ts");
const TPL_AVATAR_TEST_TS: &str = include_str!("scaffold/.zero/components/Avatar.test.ts");
const TPL_AVATAR_SCSS: &str = include_str!("scaffold/.zero/styles/components/_avatar.scss");
const TPL_BADGE_TS: &str = include_str!("scaffold/.zero/components/Badge.ts");
const TPL_BADGE_TEST_TS: &str = include_str!("scaffold/.zero/components/Badge.test.ts");
const TPL_BADGE_SCSS: &str = include_str!("scaffold/.zero/styles/components/_badge.scss");
const TPL_BUTTON_TS: &str = include_str!("scaffold/.zero/components/Button.ts");
const TPL_BUTTON_TEST_TS: &str = include_str!("scaffold/.zero/components/Button.test.ts");
const TPL_BUTTON_SCSS: &str = include_str!("scaffold/.zero/styles/components/_button.scss");
const TPL_CARD_TS: &str = include_str!("scaffold/.zero/components/Card.ts");
const TPL_CARD_TEST_TS: &str = include_str!("scaffold/.zero/components/Card.test.ts");
const TPL_CARD_SCSS: &str = include_str!("scaffold/.zero/styles/components/_card.scss");
const TPL_CHECKBOX_TS: &str = include_str!("scaffold/.zero/components/Checkbox.ts");
const TPL_CHECKBOX_TEST_TS: &str = include_str!("scaffold/.zero/components/Checkbox.test.ts");
const TPL_CHECKBOX_SCSS: &str = include_str!("scaffold/.zero/styles/components/_checkbox.scss");
const TPL_COMBOBOX_TS: &str = include_str!("scaffold/.zero/components/Combobox.ts");
const TPL_COMBOBOX_TEST_TS: &str = include_str!("scaffold/.zero/components/Combobox.test.ts");
const TPL_COMBOBOX_SCSS: &str = include_str!("scaffold/.zero/styles/components/_combobox.scss");
const TPL_DIALOG_TS: &str = include_str!("scaffold/.zero/components/Dialog.ts");
const TPL_DIALOG_TEST_TS: &str = include_str!("scaffold/.zero/components/Dialog.test.ts");
const TPL_DIALOG_SCSS: &str = include_str!("scaffold/.zero/styles/components/_dialog.scss");
const TPL_DRAWER_TS: &str = include_str!("scaffold/.zero/components/Drawer.ts");
const TPL_DRAWER_TEST_TS: &str = include_str!("scaffold/.zero/components/Drawer.test.ts");
const TPL_DRAWER_SCSS: &str = include_str!("scaffold/.zero/styles/components/_drawer.scss");
const TPL_INPUT_TS: &str = include_str!("scaffold/.zero/components/Input.ts");
const TPL_INPUT_TEST_TS: &str = include_str!("scaffold/.zero/components/Input.test.ts");
const TPL_INPUT_SCSS: &str = include_str!("scaffold/.zero/styles/components/_input.scss");
const TPL_PAGINATION_TS: &str = include_str!("scaffold/.zero/components/Pagination.ts");
const TPL_PAGINATION_TEST_TS: &str = include_str!("scaffold/.zero/components/Pagination.test.ts");
const TPL_PAGINATION_SCSS: &str = include_str!("scaffold/.zero/styles/components/_pagination.scss");
const TPL_RADIO_TS: &str = include_str!("scaffold/.zero/components/Radio.ts");
const TPL_RADIO_TEST_TS: &str = include_str!("scaffold/.zero/components/Radio.test.ts");
const TPL_RADIO_SCSS: &str = include_str!("scaffold/.zero/styles/components/_radio.scss");
const TPL_SELECT_TS: &str = include_str!("scaffold/.zero/components/Select.ts");
const TPL_SELECT_TEST_TS: &str = include_str!("scaffold/.zero/components/Select.test.ts");
const TPL_SELECT_SCSS: &str = include_str!("scaffold/.zero/styles/components/_select.scss");
const TPL_SPINNER_TS: &str = include_str!("scaffold/.zero/components/Spinner.ts");
const TPL_SPINNER_TEST_TS: &str = include_str!("scaffold/.zero/components/Spinner.test.ts");
const TPL_SPINNER_SCSS: &str = include_str!("scaffold/.zero/styles/components/_spinner.scss");
const TPL_TABS_TS: &str = include_str!("scaffold/.zero/components/Tabs.ts");
const TPL_TABS_TEST_TS: &str = include_str!("scaffold/.zero/components/Tabs.test.ts");
const TPL_TABS_SCSS: &str = include_str!("scaffold/.zero/styles/components/_tabs.scss");
const TPL_TABLE_TS: &str = include_str!("scaffold/.zero/components/Table.ts");
const TPL_TABLE_TEST_TS: &str = include_str!("scaffold/.zero/components/Table.test.ts");
const TPL_TABLE_SCSS: &str = include_str!("scaffold/.zero/styles/components/_table.scss");
const TPL_TEXTAREA_TS: &str = include_str!("scaffold/.zero/components/TextArea.ts");
const TPL_TEXTAREA_TEST_TS: &str = include_str!("scaffold/.zero/components/TextArea.test.ts");
const TPL_TEXTAREA_SCSS: &str = include_str!("scaffold/.zero/styles/components/_textarea.scss");
const TPL_TOAST_TS: &str = include_str!("scaffold/.zero/components/Toast.ts");
const TPL_TOAST_TEST_TS: &str = include_str!("scaffold/.zero/components/Toast.test.ts");
const TPL_TOAST_SCSS: &str = include_str!("scaffold/.zero/styles/components/_toast.scss");
const TPL_TOGGLE_TS: &str = include_str!("scaffold/.zero/components/Toggle.ts");
const TPL_TOGGLE_TEST_TS: &str = include_str!("scaffold/.zero/components/Toggle.test.ts");
const TPL_TOGGLE_SCSS: &str = include_str!("scaffold/.zero/styles/components/_toggle.scss");
// Inlined rather than `include_str!`'d from `src/scaffold/.gitignore` because
// that path's `.zero/` rule would cause this repo's git to ignore
// `src/scaffold/.zero/`, preventing the framework SCSS partials from being
// tracked.
const TPL_GITIGNORE: &str = ".zero/\ndist/\ncoverage/\nmutation/\n";

// Geist + Geist Mono variable-axis woff2 fonts and the SIL Open Font License
// text, embedded so `zero init` and `zero update` materialize them on disk.
const FONT_GEIST: &[u8] = include_bytes!("scaffold/.zero/fonts/Geist-VariableFont_wght.woff2");
const FONT_GEIST_ITALIC: &[u8] =
    include_bytes!("scaffold/.zero/fonts/Geist-Italic-VariableFont_wght.woff2");
const FONT_GEIST_MONO: &[u8] =
    include_bytes!("scaffold/.zero/fonts/GeistMono-VariableFont_wght.woff2");
const FONT_GEIST_MONO_ITALIC: &[u8] =
    include_bytes!("scaffold/.zero/fonts/GeistMono-Italic-VariableFont_wght.woff2");
const FONT_OFL_TXT: &[u8] = include_bytes!("scaffold/.zero/fonts/OFL.txt");

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
        ("AGENTS.md", TPL_AGENTS_MD),
        (".zero/zero.d.ts", zero_runtime::ZERO_TYPES_BODY),
        (".zero/zero-test.d.ts", zero_runtime::ZERO_TEST_TYPES_BODY),
        (".zero/zero-http.d.ts", zero_runtime::ZERO_HTTP_TYPES_BODY),
        (".zero/styles/_palette.scss", TPL_PALETTE_SCSS),
        (".zero/styles/_tokens.scss", TPL_TOKENS_SCSS),
        (".zero/styles/_themes.scss", TPL_THEMES_SCSS),
        (".zero/styles/themes/_light.scss", TPL_THEME_LIGHT_SCSS),
        (".zero/styles/themes/_dark.scss", TPL_THEME_DARK_SCSS),
        (".zero/styles/_base.scss", TPL_BASE_SCSS),
        (".zero/styles/_layout.scss", TPL_LAYOUT_SCSS),
        (".zero/styles/_utilities.scss", TPL_UTILITIES_SCSS),
        (".zero/styles/_alignment.scss", TPL_ALIGNMENT_SCSS),
        (".zero/styles/_typography.scss", TPL_TYPOGRAPHY_SCSS),
        (
            ".zero/styles/_components.scss",
            TPL_COMPONENTS_AGGREGATE_SCSS,
        ),
        (".zero/styles/zero.scss", TPL_ZERO_SCSS),
        (".zero/components/index.ts", TPL_COMPONENTS_INDEX_TS),
        (".zero/components/_internal.ts", TPL_COMPONENTS_INTERNAL_TS),
        (
            ".zero/components/_internal.test.ts",
            TPL_COMPONENTS_INTERNAL_TEST_TS,
        ),
        (".zero/components/form.ts", TPL_FORM_TS),
        (".zero/components/form.test.ts", TPL_FORM_TEST_TS),
        (".zero/components.d.ts", TPL_COMPONENTS_DTS),
        (".zero/components/Avatar.ts", TPL_AVATAR_TS),
        (".zero/components/Avatar.test.ts", TPL_AVATAR_TEST_TS),
        (".zero/styles/components/_avatar.scss", TPL_AVATAR_SCSS),
        (".zero/components/Badge.ts", TPL_BADGE_TS),
        (".zero/components/Badge.test.ts", TPL_BADGE_TEST_TS),
        (".zero/styles/components/_badge.scss", TPL_BADGE_SCSS),
        (".zero/components/Button.ts", TPL_BUTTON_TS),
        (".zero/components/Button.test.ts", TPL_BUTTON_TEST_TS),
        (".zero/styles/components/_button.scss", TPL_BUTTON_SCSS),
        (".zero/components/Card.ts", TPL_CARD_TS),
        (".zero/components/Card.test.ts", TPL_CARD_TEST_TS),
        (".zero/styles/components/_card.scss", TPL_CARD_SCSS),
        (".zero/components/Checkbox.ts", TPL_CHECKBOX_TS),
        (".zero/components/Checkbox.test.ts", TPL_CHECKBOX_TEST_TS),
        (".zero/styles/components/_checkbox.scss", TPL_CHECKBOX_SCSS),
        (".zero/components/Combobox.ts", TPL_COMBOBOX_TS),
        (".zero/components/Combobox.test.ts", TPL_COMBOBOX_TEST_TS),
        (".zero/styles/components/_combobox.scss", TPL_COMBOBOX_SCSS),
        (".zero/components/Dialog.ts", TPL_DIALOG_TS),
        (".zero/components/Dialog.test.ts", TPL_DIALOG_TEST_TS),
        (".zero/styles/components/_dialog.scss", TPL_DIALOG_SCSS),
        (".zero/components/Drawer.ts", TPL_DRAWER_TS),
        (".zero/components/Drawer.test.ts", TPL_DRAWER_TEST_TS),
        (".zero/styles/components/_drawer.scss", TPL_DRAWER_SCSS),
        (".zero/components/Input.ts", TPL_INPUT_TS),
        (".zero/components/Input.test.ts", TPL_INPUT_TEST_TS),
        (".zero/styles/components/_input.scss", TPL_INPUT_SCSS),
        (".zero/components/Pagination.ts", TPL_PAGINATION_TS),
        (
            ".zero/components/Pagination.test.ts",
            TPL_PAGINATION_TEST_TS,
        ),
        (
            ".zero/styles/components/_pagination.scss",
            TPL_PAGINATION_SCSS,
        ),
        (".zero/components/Radio.ts", TPL_RADIO_TS),
        (".zero/components/Radio.test.ts", TPL_RADIO_TEST_TS),
        (".zero/styles/components/_radio.scss", TPL_RADIO_SCSS),
        (".zero/components/Select.ts", TPL_SELECT_TS),
        (".zero/components/Select.test.ts", TPL_SELECT_TEST_TS),
        (".zero/styles/components/_select.scss", TPL_SELECT_SCSS),
        (".zero/components/Spinner.ts", TPL_SPINNER_TS),
        (".zero/components/Spinner.test.ts", TPL_SPINNER_TEST_TS),
        (".zero/styles/components/_spinner.scss", TPL_SPINNER_SCSS),
        (".zero/components/Tabs.ts", TPL_TABS_TS),
        (".zero/components/Tabs.test.ts", TPL_TABS_TEST_TS),
        (".zero/styles/components/_tabs.scss", TPL_TABS_SCSS),
        (".zero/components/Table.ts", TPL_TABLE_TS),
        (".zero/components/Table.test.ts", TPL_TABLE_TEST_TS),
        (".zero/styles/components/_table.scss", TPL_TABLE_SCSS),
        (".zero/components/TextArea.ts", TPL_TEXTAREA_TS),
        (".zero/components/TextArea.test.ts", TPL_TEXTAREA_TEST_TS),
        (".zero/styles/components/_textarea.scss", TPL_TEXTAREA_SCSS),
        (".zero/components/Toast.ts", TPL_TOAST_TS),
        (".zero/components/Toast.test.ts", TPL_TOAST_TEST_TS),
        (".zero/styles/components/_toast.scss", TPL_TOAST_SCSS),
        (".zero/components/Toggle.ts", TPL_TOGGLE_TS),
        (".zero/components/Toggle.test.ts", TPL_TOGGLE_TEST_TS),
        (".zero/styles/components/_toggle.scss", TPL_TOGGLE_SCSS),
    ]
}

/// Returns the canonical list of framework-owned binary assets written into
/// `.zero/`. Each tuple is `(relative_path, bytes)`. Parallel to
/// [`framework_manifest`] for text files; consulted by `zero init` and
/// `zero update` so binary assets follow the same single-source-of-truth
/// rule.
///
/// # Returns
/// A vector of `(relative path, file bytes)` pairs.
pub fn binary_manifest() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (".zero/fonts/Geist-VariableFont_wght.woff2", FONT_GEIST),
        (
            ".zero/fonts/Geist-Italic-VariableFont_wght.woff2",
            FONT_GEIST_ITALIC,
        ),
        (
            ".zero/fonts/GeistMono-VariableFont_wght.woff2",
            FONT_GEIST_MONO,
        ),
        (
            ".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2",
            FONT_GEIST_MONO_ITALIC,
        ),
        (".zero/fonts/OFL.txt", FONT_OFL_TXT),
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
#[doc(hidden)]
pub fn write_user_files(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()> {
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
    for (rel, bytes) in binary_manifest() {
        let abs = root_dir.join(rel);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&abs, bytes)?;
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

    /// Canonical roster of landed components. Each per-component step
    /// appends one entry; the iterating tests below derive their coverage
    /// from this slice so adding a component does not require duplicating
    /// per-name assertions.
    const COMPONENT_NAMES: &[&str] = &[
        "Avatar",
        "Badge",
        "Button",
        "Card",
        "Checkbox",
        "Combobox",
        "Dialog",
        "Drawer",
        "Input",
        "Pagination",
        "Radio",
        "Select",
        "Spinner",
        "Table",
        "Tabs",
        "TextArea",
        "Toast",
        "Toggle",
    ];

    fn fresh_scaffold() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        let ctx = ScaffoldContext {
            title: "My zero app".to_string(),
        };
        write_initial_project(&root, &ctx).unwrap();
        (dir, root)
    }

    /// Return the `{ ... }` body of the first CSS rule whose selector
    /// line begins with `selector`. Newlines preserved. Returns `None`
    /// if the selector is not found or the braces are unbalanced.
    fn extract_rule_block(css: &str, selector: &str) -> Option<String> {
        let head_idx = css.lines().enumerate().find_map(|(i, line)| {
            if line.trim_start().starts_with(selector) {
                Some(i)
            } else {
                None
            }
        })?;
        let mut byte_start = css
            .lines()
            .take(head_idx)
            .map(|l| l.len() + 1)
            .sum::<usize>();
        let after_brace = css[byte_start..].find('{')?;
        byte_start += after_brace + 1;
        let mut depth = 1usize;
        let mut byte_end = byte_start;
        for (i, ch) in css[byte_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        byte_end = byte_start + i;
                        return Some(css[byte_start..byte_end].to_string());
                    }
                }
                _ => {}
            }
        }
        let _ = byte_end;
        None
    }

    #[test]
    fn components_index_re_exports_each_listed() {
        let (_dir, root) = fresh_scaffold();
        let idx = fs::read_to_string(root.join(".zero/components/index.ts")).unwrap();
        for name in COMPONENT_NAMES {
            assert!(
                idx.contains(&format!("export {{ default as {name}")),
                "components/index.ts missing export for {name}: {idx}"
            );
        }
    }

    #[test]
    fn component_source_files_emitted() {
        let (_dir, root) = fresh_scaffold();
        for name in COMPONENT_NAMES {
            let p = root.join(format!(".zero/components/{name}.ts"));
            let src = fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing {}", p.display()));
            assert!(!src.is_empty(), "{name}.ts is empty");
            let cls = name.to_ascii_lowercase();
            // Allow either a fully-static class attribute (`class="<cls>"`)
            // or a computed class string starting with the base class
            // (e.g. `` `<cls> ${...}` `` / `` `<cls>-${...}` ``). The
            // template parser does not preserve a static prefix alongside
            // a `${}` slot inside an attribute value, so components with
            // a dynamic size/variant build the class string outside the
            // template and interpolate it as one slot.
            let has_static = src.contains(&format!("class=\"{cls}"));
            let has_template =
                src.contains(&format!("`{cls} ")) || src.contains(&format!("`{cls}-"));
            assert!(
                has_static || has_template,
                "{name}.ts missing base class \"{cls}\": {src}"
            );
        }
    }

    #[test]
    fn component_test_files_emitted() {
        let (_dir, root) = fresh_scaffold();
        for name in COMPONENT_NAMES {
            let p = root.join(format!(".zero/components/{name}.test.ts"));
            let src = fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing {}", p.display()));
            assert!(
                src.contains("from \"zero/test\""),
                "{name}.test.ts missing `from \"zero/test\"`: {src}"
            );
        }
    }

    #[test]
    fn component_partials_use_layer_components() {
        let (_dir, root) = fresh_scaffold();
        for name in COMPONENT_NAMES {
            let cls = name.to_ascii_lowercase();
            let p = root.join(format!(".zero/styles/components/_{cls}.scss"));
            let src = fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing {}", p.display()));
            assert!(
                src.contains("@layer components"),
                "_{cls}.scss missing @layer components: {src}"
            );
            assert!(
                !src.contains("!important"),
                "_{cls}.scss contains !important: {src}"
            );
        }
    }

    #[test]
    fn components_aggregate_uses_each_partial() {
        let (_dir, root) = fresh_scaffold();
        let agg = fs::read_to_string(root.join(".zero/styles/_components.scss")).unwrap();
        for name in COMPONENT_NAMES {
            let cls = name.to_ascii_lowercase();
            assert!(
                agg.contains(&format!("@use 'components/{cls}'")),
                "_components.scss missing @use 'components/{cls}': {agg}"
            );
        }
    }

    #[test]
    fn form_module_registered() {
        let manifest = framework_manifest();
        let paths: BTreeSet<&str> = manifest.iter().map(|(p, _)| *p).collect();
        assert!(
            paths.contains(".zero/components/form.ts"),
            "manifest missing .zero/components/form.ts"
        );
        assert!(
            paths.contains(".zero/components/form.test.ts"),
            "manifest missing .zero/components/form.test.ts"
        );
        assert!(
            TPL_COMPONENTS_INDEX_TS.contains("export { createForm } from \"./form.ts\";"),
            "components/index.ts must re-export createForm"
        );
        assert!(
            TPL_COMPONENTS_DTS.contains("createForm"),
            "components.d.ts must declare createForm"
        );
    }

    #[test]
    fn components_dts_declares_each_listed() {
        let (_dir, root) = fresh_scaffold();
        let dts = fs::read_to_string(root.join(".zero/components.d.ts")).unwrap();
        for name in COMPONENT_NAMES {
            let plain = format!("{name}(");
            let generic = format!("{name}<");
            assert!(
                dts.contains(&plain) || dts.contains(&generic),
                "components.d.ts missing function declaration for {name}: {dts}"
            );
        }
    }

    #[test]
    fn components_dts_accepts_computed_for_widened_props() {
        let (_dir, root) = fresh_scaffold();
        let dts = fs::read_to_string(root.join(".zero/components.d.ts")).unwrap();
        assert!(
            dts.contains("totalPages: Signal<number> | Computed<number> | number"),
            "components.d.ts: PaginationProps.totalPages must accept Computed: {dts}"
        );
        assert!(
            dts.contains("disabled?: Signal<boolean> | Computed<boolean> | boolean"),
            "components.d.ts: disabled must accept Computed: {dts}"
        );
        assert!(
            dts.contains("import type { Signal, Computed, TemplateResult } from \"zero\""),
            "components.d.ts must import Computed alongside Signal: {dts}"
        );
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
    }

    #[test]
    fn write_initial_project_emits_framework_files() {
        let (_dir, root) = fresh_scaffold();

        let zero_dts = fs::read_to_string(root.join(".zero/zero.d.ts")).unwrap();
        assert!(zero_dts.contains("declare module \"zero\""));

        let zero_test_dts = fs::read_to_string(root.join(".zero/zero-test.d.ts")).unwrap();
        assert!(zero_test_dts.contains("declare module \"zero/test\""));

        let zero_http_dts = fs::read_to_string(root.join(".zero/zero-http.d.ts")).unwrap();
        assert!(zero_http_dts.contains("declare module \"zero/http\""));

        let tokens_scss = fs::read_to_string(root.join(".zero/styles/_tokens.scss")).unwrap();
        assert!(tokens_scss.contains("--space-md:"));

        let light_scss = fs::read_to_string(root.join(".zero/styles/themes/_light.scss")).unwrap();
        assert!(light_scss.contains("--color-primary:"));

        let base_scss = fs::read_to_string(root.join(".zero/styles/_base.scss")).unwrap();
        assert!(!base_scss.is_empty());

        let layout_scss = fs::read_to_string(root.join(".zero/styles/_layout.scss")).unwrap();
        assert!(!layout_scss.is_empty());

        let split_block = extract_rule_block(&layout_scss, ".split")
            .expect("_layout.scss must declare a .split rule");
        assert!(
            split_block.contains("display: flex;"),
            ".split must be a flex container, got:\n{split_block}"
        );
        assert!(
            split_block.contains("justify-content: space-between;"),
            ".split must use justify-content: space-between, got:\n{split_block}"
        );
        assert!(
            !split_block.contains("grid-template-columns"),
            ".split must not retain its previous grid definition, got:\n{split_block}"
        );

        let utilities_scss = fs::read_to_string(root.join(".zero/styles/_utilities.scss")).unwrap();
        assert!(!utilities_scss.is_empty());

        let alignment_scss = fs::read_to_string(root.join(".zero/styles/_alignment.scss")).unwrap();
        assert!(!alignment_scss.is_empty());

        let zero_scss = fs::read_to_string(root.join(".zero/styles/zero.scss")).unwrap();
        assert!(!zero_scss.is_empty());

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(!agents.is_empty(), "AGENTS.md is empty");

        for rel in [
            ".zero/fonts/Geist-VariableFont_wght.woff2",
            ".zero/fonts/Geist-Italic-VariableFont_wght.woff2",
            ".zero/fonts/GeistMono-VariableFont_wght.woff2",
            ".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2",
            ".zero/fonts/OFL.txt",
        ] {
            let p = root.join(rel);
            let bytes = fs::read(&p).unwrap_or_else(|_| panic!("missing {rel}"));
            assert!(!bytes.is_empty(), "{rel} is empty");
        }
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
    fn write_initial_project_emits_gitignore_with_coverage_and_mutation() {
        let (_dir, root) = fresh_scaffold();
        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        let lines: Vec<&str> = gitignore.lines().map(|l| l.trim()).collect();
        assert!(
            lines.contains(&"coverage/"),
            ".gitignore missing `coverage/` line: {gitignore}"
        );
        assert!(
            lines.contains(&"mutation/"),
            ".gitignore missing `mutation/` line: {gitignore}"
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
            "## Component library",
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
            "## Best practices",
            "## Common mistakes",
            "### When to reach for which primitive",
            "### Reach for these first",
            "### When to run what",
        ] {
            assert!(
                agents.contains(sentinel),
                "AGENTS.md is missing section sentinel: {sentinel}"
            );
        }
        assert!(
            agents.contains("framework-owned just like the files under `.zero/`"),
            "AGENTS.md missing framework-ownership note in the .zero/ section: {agents}"
        );
        assert!(
            agents.contains("zero mutate"),
            "AGENTS.md Quick Start must mention `zero mutate`: {agents}"
        );
        assert!(
            agents.contains("--coverage"),
            "AGENTS.md Quick Start must mention `--coverage`: {agents}"
        );
    }

    #[test]
    fn utilities_scss_has_zero_step_for_gap_and_pad() {
        let (_dir, root) = fresh_scaffold();
        let utilities = fs::read_to_string(root.join(".zero/styles/_utilities.scss")).unwrap();
        assert!(
            utilities.contains(".gap-0"),
            "_utilities.scss missing .gap-0: {utilities}"
        );
        assert!(
            utilities.contains(".pad-0"),
            "_utilities.scss missing .pad-0: {utilities}"
        );
    }

    #[test]
    fn agents_md_lists_all_lint_rules() {
        let (_dir, root) = fresh_scaffold();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        for rule in [
            "L01", "L02", "L03", "L04", "L05", "L06", "L07", "L08", "L09", "L10", "L11", "L12",
            "L13",
        ] {
            assert!(
                agents.contains(rule),
                "AGENTS.md missing lint rule sentinel: {rule}"
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
    fn radius_scale_has_seven_steps() {
        let (_dir, root) = fresh_scaffold();
        let tokens = fs::read_to_string(root.join(".zero/styles/_tokens.scss")).unwrap();
        for token in [
            "--radius-xs:",
            "--radius-sm:",
            "--radius-md:",
            "--radius-lg:",
            "--radius-xl:",
            "--radius-2xl:",
            "--radius-3xl:",
        ] {
            assert!(
                tokens.contains(token),
                "_tokens.scss missing {token}: {tokens}"
            );
        }
    }

    #[test]
    fn tokens_and_themes_split_correctly() {
        let (_dir, root) = fresh_scaffold();
        let tokens = fs::read_to_string(root.join(".zero/styles/_tokens.scss")).unwrap();
        assert!(
            tokens.contains("--space-md:"),
            "_tokens.scss missing --space-md: {tokens}"
        );
        assert!(
            tokens.contains("--font-sans:"),
            "_tokens.scss missing --font-sans: {tokens}"
        );
        assert!(
            tokens.contains("--font-size-md:"),
            "_tokens.scss missing --font-size-md: {tokens}"
        );
        assert!(
            !tokens.contains("--color-primary:"),
            "_tokens.scss must not declare --color-primary (moved to themes): {tokens}"
        );
        assert!(
            !tokens.contains("@media"),
            "_tokens.scss must not contain @media block (moved to _themes.scss): {tokens}"
        );
        assert!(
            !tokens.contains("--font-md:"),
            "_tokens.scss must not contain old font-md token name: {tokens}"
        );
        assert!(
            !tokens.contains("$color-primary"),
            "_tokens.scss must not contain SCSS variable $color-primary: {tokens}"
        );
        assert!(
            !tokens.contains("--radius-pill"),
            "_tokens.scss must not contain legacy --radius-pill (renamed to --radius-3xl): {tokens}"
        );

        let light = fs::read_to_string(root.join(".zero/styles/themes/_light.scss")).unwrap();
        assert!(
            light.contains("--color-primary:"),
            "themes/_light.scss missing --color-primary: {light}"
        );

        let dark = fs::read_to_string(root.join(".zero/styles/themes/_dark.scss")).unwrap();
        assert!(
            dark.contains("--color-primary:"),
            "themes/_dark.scss missing --color-primary: {dark}"
        );

        let themes = fs::read_to_string(root.join(".zero/styles/_themes.scss")).unwrap();
        assert!(
            themes.contains("@media (prefers-color-scheme: dark)"),
            "_themes.scss missing prefers-color-scheme: dark block: {themes}"
        );
        assert!(
            themes.contains("[data-theme=\"dark\"]"),
            "_themes.scss missing [data-theme=\"dark\"]: {themes}"
        );
        assert!(
            themes.contains("[data-theme=\"light\"]"),
            "_themes.scss missing [data-theme=\"light\"]: {themes}"
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
            "@use 'palette'",
            "@use 'tokens'",
            "@use 'themes'",
            "@use 'base'",
            "@use 'layout'",
            "@use 'utilities'",
            "@use 'alignment'",
            "@use 'typography'",
            "@use 'components'",
        ] {
            assert!(
                zero_scss.contains(needle),
                "zero.scss missing {needle}: {zero_scss}"
            );
        }
    }

    #[test]
    fn components_aggregate_partial_exists() {
        let (_dir, root) = fresh_scaffold();
        let partial = fs::read_to_string(root.join(".zero/styles/_components.scss")).unwrap();
        assert!(!partial.is_empty(), "_components.scss is empty: {partial}");
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
    fn write_initial_project_emits_components_stubs() {
        let (_dir, root) = fresh_scaffold();

        let index_ts = fs::read_to_string(root.join(".zero/components/index.ts")).unwrap();
        assert!(!index_ts.is_empty(), ".zero/components/index.ts is empty");

        let components_dts = fs::read_to_string(root.join(".zero/components.d.ts")).unwrap();
        assert!(
            components_dts.contains("declare module \"zero/components\""),
            ".zero/components.d.ts missing module declaration: {components_dts}"
        );
    }

    #[test]
    fn tsconfig_include_contains_components_dts() {
        let (_dir, root) = fresh_scaffold();
        let tsconfig = fs::read_to_string(root.join("tsconfig.json")).unwrap();
        assert!(
            tsconfig.contains(".zero/components.d.ts"),
            "tsconfig.json missing .zero/components.d.ts in include: {tsconfig}"
        );
    }

    #[test]
    fn tsconfig_include_contains_zero_http_dts() {
        let (_dir, root) = fresh_scaffold();
        let tsconfig = fs::read_to_string(root.join("tsconfig.json")).unwrap();
        assert!(
            tsconfig.contains(".zero/zero-http.d.ts"),
            "tsconfig.json missing .zero/zero-http.d.ts in include: {tsconfig}"
        );
    }

    #[test]
    fn write_initial_project_emits_typography_partial() {
        let (_dir, root) = fresh_scaffold();
        let typo = fs::read_to_string(root.join(".zero/styles/_typography.scss")).unwrap();
        assert!(
            typo.contains("@layer components"),
            "missing @layer components: {typo}"
        );
        for cls in [
            ".text-display",
            ".text-h1",
            ".text-h2",
            ".text-h3",
            ".text-h4",
            ".text-eyebrow",
            ".text-body",
            ".text-small",
            ".text-muted",
            ".text-code",
            ".text-link",
            ".divider",
        ] {
            assert!(typo.contains(cls), "_typography.scss missing {cls}: {typo}");
        }
        assert!(!typo.contains("!important"));
    }

    #[test]
    fn binary_manifest_matches_expected_paths() {
        let manifest = binary_manifest();
        let actual: BTreeSet<&str> = manifest.iter().map(|(p, _)| *p).collect();
        let expected: BTreeSet<&str> = [
            ".zero/fonts/Geist-VariableFont_wght.woff2",
            ".zero/fonts/Geist-Italic-VariableFont_wght.woff2",
            ".zero/fonts/GeistMono-VariableFont_wght.woff2",
            ".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2",
            ".zero/fonts/OFL.txt",
        ]
        .into_iter()
        .collect();
        assert_eq!(actual, expected, "binary manifest path-set drift");
        assert_eq!(manifest.len(), 5, "binary manifest has duplicate keys");
        for (_, bytes) in manifest {
            assert!(!bytes.is_empty(), "binary manifest entry is empty");
        }
    }

    #[test]
    fn framework_manifest_includes_agents_md_first() {
        let manifest = framework_manifest();
        let first = manifest.first().expect("manifest is non-empty");
        assert_eq!(first.0, "AGENTS.md", "AGENTS.md must be first entry");
        assert!(!first.1.is_empty(), "AGENTS.md template is empty");
    }

    #[test]
    fn framework_manifest_matches_expected_path_set() {
        let manifest = framework_manifest();
        let actual: BTreeSet<&str> = manifest.iter().map(|(p, _)| *p).collect();
        let expected: BTreeSet<&str> = [
            "AGENTS.md",
            // Type declarations + style aggregate.
            ".zero/zero.d.ts",
            ".zero/zero-test.d.ts",
            ".zero/zero-http.d.ts",
            ".zero/components.d.ts",
            ".zero/components/index.ts",
            ".zero/components/_internal.ts",
            ".zero/components/_internal.test.ts",
            ".zero/components/form.ts",
            ".zero/components/form.test.ts",
            // Design-system SCSS partials + aggregate.
            ".zero/styles/_palette.scss",
            ".zero/styles/_tokens.scss",
            ".zero/styles/_themes.scss",
            ".zero/styles/themes/_light.scss",
            ".zero/styles/themes/_dark.scss",
            ".zero/styles/_base.scss",
            ".zero/styles/_layout.scss",
            ".zero/styles/_utilities.scss",
            ".zero/styles/_alignment.scss",
            ".zero/styles/_typography.scss",
            ".zero/styles/_components.scss",
            ".zero/styles/zero.scss",
            // 18 components × (source, test, scss partial) = 54 entries.
            ".zero/components/Avatar.ts",
            ".zero/components/Avatar.test.ts",
            ".zero/styles/components/_avatar.scss",
            ".zero/components/Badge.ts",
            ".zero/components/Badge.test.ts",
            ".zero/styles/components/_badge.scss",
            ".zero/components/Button.ts",
            ".zero/components/Button.test.ts",
            ".zero/styles/components/_button.scss",
            ".zero/components/Card.ts",
            ".zero/components/Card.test.ts",
            ".zero/styles/components/_card.scss",
            ".zero/components/Checkbox.ts",
            ".zero/components/Checkbox.test.ts",
            ".zero/styles/components/_checkbox.scss",
            ".zero/components/Combobox.ts",
            ".zero/components/Combobox.test.ts",
            ".zero/styles/components/_combobox.scss",
            ".zero/components/Dialog.ts",
            ".zero/components/Dialog.test.ts",
            ".zero/styles/components/_dialog.scss",
            ".zero/components/Drawer.ts",
            ".zero/components/Drawer.test.ts",
            ".zero/styles/components/_drawer.scss",
            ".zero/components/Input.ts",
            ".zero/components/Input.test.ts",
            ".zero/styles/components/_input.scss",
            ".zero/components/Pagination.ts",
            ".zero/components/Pagination.test.ts",
            ".zero/styles/components/_pagination.scss",
            ".zero/components/Radio.ts",
            ".zero/components/Radio.test.ts",
            ".zero/styles/components/_radio.scss",
            ".zero/components/Select.ts",
            ".zero/components/Select.test.ts",
            ".zero/styles/components/_select.scss",
            ".zero/components/Spinner.ts",
            ".zero/components/Spinner.test.ts",
            ".zero/styles/components/_spinner.scss",
            ".zero/components/Tabs.ts",
            ".zero/components/Tabs.test.ts",
            ".zero/styles/components/_tabs.scss",
            ".zero/components/Table.ts",
            ".zero/components/Table.test.ts",
            ".zero/styles/components/_table.scss",
            ".zero/components/TextArea.ts",
            ".zero/components/TextArea.test.ts",
            ".zero/styles/components/_textarea.scss",
            ".zero/components/Toast.ts",
            ".zero/components/Toast.test.ts",
            ".zero/styles/components/_toast.scss",
            ".zero/components/Toggle.ts",
            ".zero/components/Toggle.test.ts",
            ".zero/styles/components/_toggle.scss",
        ]
        .into_iter()
        .collect();
        assert_eq!(actual, expected, "manifest path set drift");
        assert_eq!(
            manifest.len(),
            expected.len(),
            "manifest has duplicate keys"
        );
    }

    fn compile_scaffold_css(root: &std::path::Path) -> String {
        let app_scss_path = root.join("styles").join("app.scss");
        let source = fs::read_to_string(&app_scss_path).unwrap();
        let opts = zero_sass::SassOptions {
            filename: "app.scss",
            inline_source_map: false,
            emit_source_map: false,
            load_paths: &[],
        };
        zero_sass::compile_scss(&source, &app_scss_path, &opts)
            .expect("compile app.scss")
            .code
    }

    #[test]
    fn compiled_zero_css_has_typography_and_fonts_and_no_element_selectors() {
        let (_dir, root) = fresh_scaffold();
        let css = compile_scaffold_css(&root);

        // (a) every typography utility class appears.
        for cls in [
            ".text-display",
            ".text-h1",
            ".text-h2",
            ".text-h3",
            ".text-h4",
            ".text-eyebrow",
            ".text-body",
            ".text-small",
            ".text-muted",
            ".text-code",
            ".text-link",
            ".divider",
        ] {
            assert!(css.contains(cls), "compiled CSS missing {cls}");
        }

        // (b) all four @font-face declarations present.
        let face_count = css.matches("@font-face").count();
        assert!(
            face_count >= 4,
            "expected >= 4 @font-face, got {face_count}"
        );
        let geist_faces = css.matches("font-family: \"Geist\"").count()
            + css.matches("font-family:\"Geist\"").count();
        assert_eq!(
            geist_faces, 2,
            "expected 2 Geist faces (normal + italic), got {geist_faces}"
        );
        let geist_mono_faces = css.matches("font-family: \"Geist Mono\"").count()
            + css.matches("font-family:\"Geist Mono\"").count();
        assert_eq!(
            geist_mono_faces, 2,
            "expected 2 Geist Mono faces, got {geist_mono_faces}"
        );

        // (c) no Google Fonts URL.
        assert!(
            !css.contains("fonts.googleapis.com"),
            "compiled CSS still imports Google Fonts"
        );

        // (d) no top-level bare element selectors for typography tags.
        for sel in [
            "\nh1 ", "\nh2 ", "\nh3 ", "\nh4 ", "\nh5 ", "\nh6 ", "\np ", "\nsmall ", "\nhr ",
            "\nh1{", "\nh2{", "\np{", "\nhr{",
        ] {
            assert!(
                !css.contains(sel),
                "compiled CSS contains forbidden element selector {sel:?}"
            );
        }
        assert!(
            !css.contains("\na {") && !css.contains("\na:hover"),
            "compiled CSS still has bare a/a:hover rule"
        );
        assert!(
            !css.contains("code, kbd, samp, pre"),
            "compiled CSS still groups code/kbd/samp/pre"
        );
    }

    #[test]
    fn write_framework_files_writes_only_dot_zero_and_agents_md() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("web");
        fs::create_dir_all(&root).unwrap();
        write_framework_files(&root).unwrap();

        for (rel, _) in framework_manifest() {
            assert!(
                root.join(rel).exists(),
                "framework text file missing after write: {rel}"
            );
        }
        for (rel, _) in binary_manifest() {
            assert!(
                root.join(rel).exists(),
                "framework binary file missing after write: {rel}"
            );
        }

        let mut entries: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec![".zero".to_string(), "AGENTS.md".to_string()],
            "write_framework_files wrote unexpected root-level entries: {entries:?}"
        );
    }
}
