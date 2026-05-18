use std::path::PathBuf;
use zero_lint::lint_source;

fn fixture(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("layout")
        .join(name);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    (path, src)
}

fn l11_count(diags: &[zero_lint::Diagnostic]) -> usize {
    diags.iter().filter(|d| d.rule == "L11").count()
}

#[test]
fn pass_cluster_uses_class() {
    let (path, src) = fixture("pass_cluster.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(l11_count(&diags), 0, "diags = {diags:#?}");
}

#[test]
fn fail_cluster_fires() {
    let (path, src) = fixture("fail_cluster.scss");
    let diags = lint_source(&path, &src);
    let l11: Vec<_> = diags.iter().filter(|d| d.rule == "L11").collect();
    assert_eq!(l11.len(), 1, "expected 1 L11; diags = {diags:#?}");
    assert!(
        l11[0].message.contains("cluster"),
        "msg = {}",
        l11[0].message
    );
}

#[test]
fn fail_cluster_no_gap_fires() {
    let (path, src) = fixture("fail_cluster_no_gap.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("cluster")),
        "expected cluster diagnostic without gap; diags = {diags:#?}"
    );
}

#[test]
fn fail_stack_no_gap_fires() {
    let (path, src) = fixture("fail_stack_no_gap.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("stack")),
        "expected stack diagnostic without gap; diags = {diags:#?}"
    );
}

#[test]
fn fail_inline_flex_cluster_fires() {
    let (path, src) = fixture("fail_inline_flex_cluster.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("cluster")),
        "inline-flex+flex-wrap should still match cluster; diags = {diags:#?}"
    );
}

#[test]
fn fail_stack_fires() {
    let (path, src) = fixture("fail_stack.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("stack"))
    );
}

#[test]
fn fail_split_fires() {
    let (path, src) = fixture("fail_split.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("split"))
    );
}

#[test]
fn fail_flank_fires() {
    let (path, src) = fixture("fail_flank.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("flank")),
        "expected flank diagnostic; diags = {diags:#?}"
    );
}

#[test]
fn fail_grid_fires() {
    let (path, src) = fixture("fail_grid.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("grid"))
    );
}

#[test]
fn fail_frame_fires() {
    let (path, src) = fixture("fail_frame.scss");
    let diags = lint_source(&path, &src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "L11" && d.message.contains("frame"))
    );
}

#[test]
fn flank_misses_when_no_child_rule() {
    let (path, src) = fixture("pass_flex_only.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(l11_count(&diags), 0, "diags = {diags:#?}");
}

#[test]
fn selector_with_primitive_class_is_skipped() {
    let (path, src) = fixture("pass_cluster_override.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(l11_count(&diags), 0, "diags = {diags:#?}");
}
