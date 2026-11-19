use super::*;
use crate::core::md::Analyzer;

#[test]
fn decorations_preserve_source_and_reveal_selected_lines() {
    let source =
        "- top\n  - nested\n- [ ] todo\n- [x] done\n\n> quote\n\n```rust\nfn main() {}\n```\n";
    let a = Analyzer::new().analyze(source);
    let plan = plan(source, &a, source.len()..source.len(), true);
    assert!(
        plan.iter()
            .any(|d| matches!(d, Decoration::Code(r) if source[r.clone()].contains("fn main")))
    );
    for glyph in ["•", "◦", "☐", "☑"] {
        assert!(
            plan.iter()
                .any(|d| matches!(d, Decoration::Marker(_, g) if *g == glyph))
        );
    }
    assert!(plan.iter().any(|d| matches!(d, Decoration::Quote(_))));
    assert!(
        plan.iter()
            .any(|d| matches!(d, Decoration::Marker(r, "☑") if &source[r.clone()] == "- [x] "))
    );
    assert!(
        !super::plan(source, &a, 0..source.len(), true)
            .iter()
            .any(|d| matches!(d, Decoration::Marker(_, _)))
    );
    assert!(
        !super::plan(source, &a, 0..0, false)
            .iter()
            .any(|d| matches!(d, Decoration::Marker(_, _)))
    );
}

#[test]
fn marker_prefixes_reserve_a_shared_gutter_and_nested_indent() {
    for (source, width) in [
        ("- ", 20.),
        ("- [x] ", 20.),
        ("  - ", 48.),
        ("    - [ ] ", 76.),
    ] {
        let span = marker_span(source, 0..source.len(), 17.);
        assert!((span.size.unwrap() * source.len() as f32 * 0.6 - width).abs() < 0.01);
        assert_eq!(span.color, Some(Color::TRANSPARENT));
    }
}

#[test]
fn checked_task_ranges_exclude_whitespace_and_allow_empty_text() {
    for (source, expected) in [("- [x] \t ação \t\nend", "ação"), ("- [x]  \nend", "")] {
        assert_eq!(&source[task_text(source, &(2..5))], expected);
    }
}

#[test]
fn concealed_code_background_uses_contents_with_equal_padding() {
    let source = "```rust\nfn main() {}\n```\nend";
    let end = source.find("\nend").unwrap();
    let (range, padding) = code_content(source, 0..end, &(source.len()..source.len()), true);
    assert_eq!(&source[range], "fn main() {}");
    assert_eq!(padding, 12.);
    assert_eq!(code_content(source, 0..end, &(0..0), true), (0..end, 0.));
    assert_eq!(code_content(source, 0..end, &(0..0), false), (0..end, 0.));
    let empty = "```\n```\nend";
    assert!(
        code_content(empty, 0..7, &(empty.len()..empty.len()), true)
            .0
            .is_empty()
    );
}

#[test]
fn empty_and_stale_analysis_has_no_decoration() {
    let a = Analyzer::new().analyze("");
    assert!(plan("", &a, 0..0, true).is_empty());
    assert!(plan("different", &a, 0..0, true).is_empty());
}

#[test]
fn fenced_blocks_include_blank_rows_and_ordered_lists_stay_raw() {
    let source = "1. ordered\n\n```\nfirst\n\nlast\n```\nend";
    let a = Analyzer::new().analyze(source);
    let decorations = plan(source, &a, source.len()..source.len(), true);
    assert_eq!(decorations.len(), 1);
    assert!(
        matches!(&decorations[0], Decoration::Code(r) if &source[r.clone()] == "```\nfirst\n\nlast\n```")
    );
}
