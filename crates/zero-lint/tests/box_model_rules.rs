use std::path::PathBuf;
use zero_lint::lint_source;

fn fixture(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("box_model")
        .join(name);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    (path, src)
}

#[test]
fn pass_radius_silent() {
    let (path, src) = fixture("pass_radius.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_radius_999_suggests_3xl() {
    let (path, src) = fixture("fail_radius_999.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.rule, "L06");
    assert!(d.message.contains("--radius-3xl"), "msg = {}", d.message);
}

#[test]
fn fail_radius_50pct_suggests_3xl() {
    let (path, src) = fixture("fail_radius_50pct.scss");
    let diags = lint_source(&path, &src);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule, "L06");
    assert!(
        diags[0].message.contains("--radius-3xl"),
        "msg = {}",
        diags[0].message
    );
}

#[test]
fn pass_border_silent() {
    let (path, src) = fixture("pass_border.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_border_shorthand_names_utility() {
    let (path, src) = fixture("fail_border_shorthand.scss");
    // Two diagnostics fire: L07 on the border width AND L05 on `red`.
    let diags = lint_source(&path, &src);
    let l07 = diags.iter().find(|d| d.rule == "L07").expect("missing L07");
    assert!(l07.message.contains("--border-thin"));
    assert!(l07.message.contains(".border"));
}

#[test]
fn pass_padding_silent() {
    let (path, src) = fixture("pass_padding.scss");
    assert!(lint_source(&path, &src).is_empty());
}

#[test]
fn fail_padding_single_suggests_space_md_and_pad_md() {
    let (path, src) = fixture("fail_padding_single.scss");
    let diags = lint_source(&path, &src);
    let l08 = diags.iter().find(|d| d.rule == "L08").expect("missing L08");
    assert!(l08.message.contains("--space-md"));
    assert!(l08.message.contains("pad-md"));
}

#[test]
fn fail_padding_two_value_names_tokens_only() {
    let (path, src) = fixture("fail_padding_two_value.scss");
    let diags = lint_source(&path, &src);
    let l08 = diags.iter().find(|d| d.rule == "L08").expect("missing L08");
    assert!(
        !l08.message.contains("pad-"),
        "two-value form should not name a utility: {}",
        l08.message
    );
}

#[test]
fn fail_margin_silent_on_utility() {
    let (path, src) = fixture("fail_margin.scss");
    let diags = lint_source(&path, &src);
    let l09 = diags.iter().find(|d| d.rule == "L09").expect("missing L09");
    assert!(l09.message.contains("--space-lg"));
    assert!(
        !l09.message.contains("pad-") && !l09.message.contains("gap-"),
        "margin should not name a utility class: {}",
        l09.message
    );
}

#[test]
fn fail_gap_names_utility() {
    let (path, src) = fixture("fail_gap.scss");
    let diags = lint_source(&path, &src);
    let l10 = diags.iter().find(|d| d.rule == "L10").expect("missing L10");
    assert!(l10.message.contains("--space-sm"));
    assert!(l10.message.contains("gap-sm"));
}

#[test]
fn calc_with_raw_value_fires() {
    let (path, src) = fixture("fail_calc_raw.scss");
    let diags = lint_source(&path, &src);
    let l08 = diags.iter().find(|d| d.rule == "L08").expect("missing L08");
    assert!(l08.message.contains("--space"));
}
