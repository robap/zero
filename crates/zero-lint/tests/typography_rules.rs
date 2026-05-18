//! Integration tests for L01–L04. Each fixture is loaded from disk and
//! piped through `lint_source`; assertions check rule id, line/col, and
//! suggested token text.

use std::path::PathBuf;
use zero_lint::lint_source;

fn fixture(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("typography")
        .join(name);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    (path, src)
}

#[test]
fn pass_weight_silent() {
    let (path, src) = fixture("pass_weight.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_weight_numeric_suggests_semi() {
    let (path, src) = fixture("fail_weight_numeric.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1, "diags = {diags:#?}");
    let d = &diags[0];
    assert_eq!(d.rule, "L01");
    assert_eq!(d.line, 2);
    assert_eq!(d.column, 3);
    assert!(
        d.message.contains("--weight-semi"),
        "message = {}",
        d.message
    );
}

#[test]
fn fail_weight_keyword_suggests_bold() {
    let (path, src) = fixture("fail_weight_keyword.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0].message.contains("--weight-bold"),
        "message = {}",
        diags[0].message
    );
}

#[test]
fn pass_size_silent() {
    let (path, src) = fixture("pass_size.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_size_px_suggests_text_utility() {
    let (path, src) = fixture("fail_size_px.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.rule, "L02");
    assert!(d.message.contains("--font-size-sm"));
    assert!(d.message.contains("text-* utility"));
}

#[test]
fn fail_size_rem_resolves_against_px_scale() {
    // 0.75rem == 12px; nearest entry in FONT_SIZE is --font-size-sm (14).
    let (path, src) = fixture("fail_size_rem.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("--font-size-sm"));
}

#[test]
fn pass_leading_silent() {
    let (path, src) = fixture("pass_leading.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_leading_unitless_suggests_snug() {
    // 1.4 is closer to --leading-snug (1.35) than --leading-normal (1.5).
    let (path, src) = fixture("fail_leading_unitless.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.rule, "L03");
    assert!(d.message.contains("--leading-snug"), "msg = {}", d.message);
}

#[test]
fn pass_tracking_silent() {
    let (path, src) = fixture("pass_tracking.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_tracking_em_suggests_wide() {
    // 0.05em is closer to --tracking-wide (0.04) than --tracking-caps (0.08).
    let (path, src) = fixture("fail_tracking_em.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.rule, "L04");
    assert!(d.message.contains("--tracking-wide"), "msg = {}", d.message);
}
