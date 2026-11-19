use super::*;

fn size_at(spans: &[Span], byte: usize) -> Option<f32> {
    spans
        .iter()
        .find(|s| s.start as usize <= byte && byte < s.end as usize)
        .and_then(|s| s.size)
}

#[test]
fn heading_and_inline_markers_reveal_only_on_touched_lines() {
    let source = "# Heading\nbody **bold**\nend";
    let a = md::Analyzer::new().analyze(source);
    let p = crate::ui::theme::LIGHT;
    let off = decorate(source, source.len(), source.len(), &a, p);
    assert_eq!(size_at(&off, 0), Some(0.1));
    assert_eq!(size_at(&off, 15), Some(0.1));
    let on = decorate(source, 3, 3, &a, p);
    assert_eq!(size_at(&on, 0), Some(32.));
    let selected = decorate(source, 0, 21, &a, p);
    assert_ne!(size_at(&selected, 15), Some(0.1));
}

#[test]
fn list_task_quote_markers_and_raw_tables_keep_stable_geometry() {
    let source = "- item\n- [ ] task\n> quote\n\n| a | b |\n| - | - |\n| x | y |\nend";
    let a = md::Analyzer::new().analyze(source);
    let spans = decorate(
        source,
        source.len(),
        source.len(),
        &a,
        crate::ui::theme::LIGHT,
    );
    for byte in [0, 1, 7, 8, 9, 10, 11, 12, 17, 18] {
        assert_ne!(size_at(&spans, byte), Some(0.1), "byte {byte}");
    }
}

#[test]
fn unicode_empty_and_stale_analysis_are_safe() {
    let mut analyzer = md::Analyzer::new();
    let source = "# Olá 👋\n**ação**\nend";
    let a = analyzer.analyze(source);
    for span in decorate(
        source,
        source.len(),
        source.len(),
        &a,
        crate::ui::theme::LIGHT,
    ) {
        assert!(source.is_char_boundary(span.start as usize));
        assert!(source.is_char_boundary(span.end as usize));
    }
    assert!(decorate("", 0, 0, &analyzer.analyze(""), crate::ui::theme::LIGHT).is_empty());
    assert!(decorate("x", 0, 0, &a, crate::ui::theme::LIGHT).is_empty());
}
