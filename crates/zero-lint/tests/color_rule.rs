use std::path::PathBuf;
use zero_lint::lint_source;

fn fixture(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("color")
        .join(name);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    (path, src)
}

#[test]
fn passes_var_color_reference() {
    let (path, src) = fixture("pass_var.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn passes_currentcolor_and_transparent() {
    let (path, src) = fixture("pass_sentinels.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn flags_hex_background_to_color_primary() {
    let (path, src) = fixture("fail_hex_background.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1, "diags = {diags:#?}");
    let d = &diags[0];
    assert_eq!(d.rule, "L05");
    assert_eq!(d.line, 2);
    assert!(
        d.message.contains("--color-primary"),
        "message = {}",
        d.message
    );
}

#[test]
fn flags_named_color_to_color_danger() {
    let (path, src) = fixture("fail_named_color.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0].message.contains("--color-danger"),
        "message = {}",
        diags[0].message
    );
}

#[test]
fn flags_rgb_function() {
    let (path, src) = fixture("fail_rgb_function.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0].message.contains("--color-danger"),
        "message = {}",
        diags[0].message
    );
}
