use super::*;
use crate::html::Context;
use crate::pdf::{Analyzer, Emitter};

fn layout_code(text: &str, width: f32) -> (Vec<(String, f32)>, f32) {
    let analysis = Analyzer::new().analyze(text);
    let context = Context {
        note_dir: None,
        root: None,
        tree: None,
    };
    let mut body = LinearLayout::vertical();
    Emitter::new(
        text,
        &analysis,
        0..text.len(),
        &context,
        Family::Mono,
        Family::Math,
    )
    .render(&mut body);
    let fonts = Fonts::load().unwrap();
    let mut layout = Layout {
        fonts: &fonts,
        pages: vec![vec![]],
        pos: Position { page: 0, y: TOP },
    };
    layout.block(&body.into(), LEFT, width);
    let mut lines: Vec<(String, f32)> = vec![];
    for draw in &layout.pages[0] {
        if let Draw::Text { y, run, .. } = draw {
            if let Some((text, previous_y)) = lines.last_mut()
                && previous_y == y
            {
                text.push_str(&run.s);
            } else {
                lines.push((run.s.clone(), *y));
            }
        }
    }
    (lines, layout.pos.y - TOP)
}

#[test]
fn code_layout_preserves_indentation_tabs_and_blank_rows() {
    for text in [
        "```rust\n  first\n\n\tlast\n```\n",
        "      first\n\n    \tlast\n",
    ] {
        let (rows, _) = layout_code(text, 500.);
        let visible: Vec<_> = rows.iter().filter(|(s, _)| !s.is_empty()).collect();
        assert_eq!(visible[0].0, "  first");
        assert_eq!(visible[1].0, "    last");
        let gap = visible[1].1 - visible[0].1;
        assert!(
            (gap - 2. * super::super::CODE_PT as f32 * 1.35).abs() < 0.01,
            "{gap}"
        );
        assert!(
            super::super::export(
                text,
                "Code",
                &Context {
                    note_dir: None,
                    root: None,
                    tree: None,
                }
            )
            .unwrap()
            .starts_with(b"%PDF")
        );
    }
}

#[test]
fn code_layout_keeps_trailing_and_whitespace_only_rows() {
    let (_, one_row) = layout_code("```\nfirst\n```\n", 500.);
    let (_, three_rows) = layout_code("```\nfirst\n   \n\n```\n", 500.);
    assert!((three_rows - one_row - 2. * super::super::CODE_PT as f32 * 1.35).abs() < 0.01);
    let (rows, _) = layout_code("```\n\n```\n", 500.);
    assert_eq!(rows.len(), 1);
}

#[test]
fn code_layout_wraps_only_overlong_rows_without_dropping_spaces_or_unicode() {
    let code = format!("    {}", "ab cd λ ".repeat(12));
    let (rows, _) = layout_code(&format!("```\n{code}\n```\n"), 50.);
    assert!(rows.len() > 1);
    assert_eq!(
        rows.into_iter().map(|(text, _)| text).collect::<String>(),
        code
    );
}
