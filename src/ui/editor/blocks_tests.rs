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
            .any(|d| matches!(d, Decoration::Marker(r, "☑") if &source[r.clone()] == "- [x]"))
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
