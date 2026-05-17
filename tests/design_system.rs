//! Integration tests for the built-in CSS design system.

fn write_scss_project(tmp: &std::path::Path) {
    std::fs::write(
        tmp.join("zero.toml"),
        "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
    )
    .unwrap();

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(tmp)
        .assert()
        .success();
}

fn find_asset(dir: &std::path::Path, prefix: &str, ext: &str) -> Option<String> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.starts_with(prefix) && name.ends_with(ext) {
                Some(name)
            } else {
                None
            }
        })
}

#[test]
fn build_emits_design_system_css() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets_dir = tmp.path().join("dist/assets");
    let css_file = find_asset(&assets_dir, "app.", ".css").expect("no hashed CSS found");
    let css = std::fs::read_to_string(assets_dir.join(&css_file)).unwrap();

    assert!(
        css.contains("--color-primary:"),
        "compiled CSS missing --color-primary token: {css}"
    );
    assert!(
        css.contains("--space-md:"),
        "compiled CSS missing --space-md token: {css}"
    );
    assert!(
        css.contains("--border-thin:"),
        "compiled CSS missing --border-thin token: {css}"
    );
    assert!(
        css.contains(".cluster {") || css.contains(".cluster{"),
        "compiled CSS missing .cluster: {css}"
    );
    assert!(
        css.contains(".stack {") || css.contains(".stack{"),
        "compiled CSS missing .stack: {css}"
    );
    assert!(
        css.contains(".gap-md {") || css.contains(".gap-md{"),
        "compiled CSS missing .gap-md: {css}"
    );
    assert!(
        css.contains(".border {") || css.contains(".border{"),
        "compiled CSS missing .border: {css}"
    );
    assert!(
        css.contains(".border-t {") || css.contains(".border-t{"),
        "compiled CSS missing .border-t: {css}"
    );
    assert!(
        css.contains(".align-start {") || css.contains(".align-start{"),
        "compiled CSS missing .align-start: {css}"
    );
    assert!(
        css.contains(".justify-between {") || css.contains(".justify-between{"),
        "compiled CSS missing .justify-between: {css}"
    );
    assert!(
        css.contains(".align-self-stretch {") || css.contains(".align-self-stretch{"),
        "compiled CSS missing .align-self-stretch: {css}"
    );
    assert!(
        css.contains(".justify-self-center {") || css.contains(".justify-self-center{"),
        "compiled CSS missing .justify-self-center: {css}"
    );
    assert!(
        css.contains(".text-center {") || css.contains(".text-center{"),
        "compiled CSS missing .text-center: {css}"
    );
    assert!(
        css.contains(".flex-col-reverse {") || css.contains(".flex-col-reverse{"),
        "compiled CSS missing .flex-col-reverse: {css}"
    );
    assert!(
        css.contains("prefers-color-scheme: dark") || css.contains("prefers-color-scheme:dark"),
        "compiled CSS missing dark mode media query: {css}"
    );
    assert!(
        css.contains("[data-theme=\"dark\"]") || css.contains("[data-theme=dark]"),
        "compiled CSS missing [data-theme=dark]: {css}"
    );
    assert!(
        css.contains("[data-theme=\"light\"]") || css.contains("[data-theme=light]"),
        "compiled CSS missing [data-theme=light]: {css}"
    );
    assert!(
        !css.contains("$color-primary"),
        "SCSS variable leaked into compiled CSS: {css}"
    );

    for needle in [
        "--button-danger-bg",
        "--button-danger-fg",
        "--badge-success-bg",
        "--badge-warning-bg",
        "--badge-danger-bg",
        "--toast-success-bg",
        "--toast-warning-bg",
        "--toast-danger-bg",
    ] {
        assert!(
            !css.contains(needle),
            "compiled CSS contains removed component-private token {needle}: {css}"
        );
    }

    for needle in [
        "--gray-50:",
        "--gray-950:",
        "--blue-600:",
        "--red-700:",
        "--green-700:",
        "--amber-500:",
    ] {
        assert!(
            css.contains(needle),
            "compiled CSS missing palette token {needle}: {css}"
        );
    }

    for needle in [
        "--color-success:",
        "--color-success-fg:",
        "--color-warning:",
        "--color-warning-fg:",
        "--color-danger:",
        "--color-danger-fg:",
        "--font-sans:",
        "--font-mono:",
        "--font-size-md:",
    ] {
        assert!(
            css.contains(needle),
            "compiled CSS missing semantic token {needle}: {css}"
        );
    }

    for needle in ["--font-sm:", "--font-md:", "--font-lg:", "--font-xl:"] {
        assert!(
            !css.contains(needle),
            "compiled CSS still declares old size-token {needle}: {css}"
        );
    }

    assert!(
        css.contains("color-scheme: light") || css.contains("color-scheme:light"),
        "compiled CSS missing color-scheme: light declaration"
    );
    assert!(
        css.contains("color-scheme: dark") || css.contains("color-scheme:dark"),
        "compiled CSS missing color-scheme: dark declaration"
    );
}

#[test]
fn build_emits_font_family_token() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets_dir = tmp.path().join("dist/assets");
    let css_file = find_asset(&assets_dir, "app.", ".css").expect("no hashed CSS found");
    let css = std::fs::read_to_string(assets_dir.join(&css_file)).unwrap();
    assert!(
        css.contains("font-family: var(--font-sans)")
            || css.contains("font-family:var(--font-sans)"),
        "body font-family does not consume --font-sans: {css}"
    );
}

#[test]
fn build_design_system_passes_contrast_smoke() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success();

    let assets_dir = tmp.path().join("dist/assets");
    let css_file = find_asset(&assets_dir, "app.", ".css").expect("no hashed CSS found");
    let css = std::fs::read_to_string(assets_dir.join(&css_file)).unwrap();

    assert!(
        css.contains("color: var(--color-text)"),
        "body color token not wired: {css}"
    );
    assert!(
        css.contains("background: var(--color-bg)"),
        "body background token not wired: {css}"
    );
}

#[test]
fn scaffold_home_uses_design_system_classes() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path());

    let home_ts = std::fs::read_to_string(tmp.path().join("web/src/routes/home.ts")).unwrap();

    assert!(
        home_ts.contains("class=\"stack pad-xl align-center\""),
        "home.ts missing stack pad-xl align-center: {home_ts}"
    );
    assert!(
        home_ts.contains("class=\"cluster gap-md\""),
        "home.ts missing cluster gap-md: {home_ts}"
    );
    assert!(
        home_ts.contains("class=\"pad-sm border\""),
        "home.ts missing pad-sm border: {home_ts}"
    );
}
