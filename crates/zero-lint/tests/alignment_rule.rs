use std::path::PathBuf;
use zero_lint::lint_source;

fn fixture(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("alignment")
        .join(name);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    (path, src)
}

fn l12_count(diags: &[zero_lint::Diagnostic]) -> usize {
    diags.iter().filter(|d| d.rule == "L12").count()
}

#[test]
fn passes_var_value() {
    let (path, src) = fixture("pass_var.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(l12_count(&diags), 0, "diags = {diags:#?}");
}

#[test]
fn flags_align_items_center_and_justify_content_center() {
    let (path, src) = fixture("fail_align_items_center.scss");
    let diags = lint_source(&path, &src);
    let l12: Vec<_> = diags.iter().filter(|d| d.rule == "L12").collect();
    assert_eq!(
        l12.len(),
        2,
        "expected align-items + justify-content; diags = {diags:#?}"
    );
    assert!(l12.iter().any(|d| d.message.contains("align-center")));
    assert!(l12.iter().any(|d| d.message.contains("justify-center")));
}

#[test]
fn flags_justify_space_between() {
    let (path, src) = fixture("fail_justify_space_between.scss");
    let diags = lint_source(&path, &src);
    let l12: Vec<_> = diags.iter().filter(|d| d.rule == "L12").collect();
    assert_eq!(l12.len(), 1);
    assert!(l12[0].message.contains("justify-between"));
}

#[test]
fn flags_text_align_center() {
    let (path, src) = fixture("fail_text_align.scss");
    let diags = lint_source(&path, &src);
    let l12: Vec<_> = diags.iter().filter(|d| d.rule == "L12").collect();
    assert_eq!(l12.len(), 1);
    assert!(l12[0].message.contains("text-center"));
}

#[test]
fn skips_selectors_with_alignment_utility_class() {
    let (path, src) = fixture("pass_override_selector.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(l12_count(&diags), 0, "diags = {diags:#?}");
}
