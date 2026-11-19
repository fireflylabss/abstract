use super::*;

#[test]
fn customization_keeps_markers_visible_and_scales_headings_and_blank_lines() {
    let source = "# Heading\n\nbody **bold**\nend";
    let a = md::Analyzer::new().analyze(source);
    let mut settings = crate::core::store::Settings::default();
    for (k, v) in [
        ("conceal", "off"),
        ("editor_size", "20"),
        ("heading_scale", "1.5"),
        ("paragraph_spacing", "10"),
    ] {
        settings.kv.set(k, v);
    }
    let spans = decorate_with(
        source,
        source.len(),
        source.len(),
        &a,
        crate::ui::theme::LIGHT,
        &settings,
    );
    assert_eq!(size_at(&spans, 0), Some(32. * 20. / 17. * 1.5));
    assert_eq!(size_at(&spans, 10), Some(30.));
    assert_eq!(size_at(&spans, source.find("**").unwrap()), Some(20.));
    assert!(spans.iter().all(|s| s.color.is_none_or(|c| c.a > 0.)));
}

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

#[test]
fn wiki_aliases_and_markdown_labels_receive_link_styling() {
    let source = "[[Note|alias]] and [web](https://example.com)\nend";
    let a = md::Analyzer::new().analyze(source);
    let spans = decorate(
        source,
        source.len(),
        source.len(),
        &a,
        crate::ui::theme::LIGHT,
    );
    for label in ["alias", "web"] {
        let byte = source.find(label).unwrap();
        let span = spans
            .iter()
            .find(|s| s.start as usize <= byte && byte < s.end as usize)
            .unwrap();
        assert_eq!(span.color, Some(rgb(crate::ui::theme::LIGHT.callout[0])));
        assert!(span.underline.is_some());
    }
}

#[test]
fn concealed_fences_shrink_including_newlines_but_reveal_at_cursor() {
    let source = "```rust\nfn main() {}\n```\nend";
    let a = md::Analyzer::new().analyze(source);
    let p = crate::ui::theme::LIGHT;
    let spans = decorate(source, source.len(), source.len(), &a, p);
    for start in [0, source.find("\n```").unwrap() + 1] {
        let newline = source[start..].find('\n').unwrap() + start;
        let span = spans
            .iter()
            .rev()
            .find(|s| s.start as usize <= start && newline < s.end as usize)
            .unwrap();
        assert_eq!(span.size, Some(4.));
        assert_eq!(span.color, Some(Color::TRANSPARENT));
    }
    let revealed = decorate(source, 1, 1, &a, p);
    assert!(!revealed.iter().any(|s| s.start == 0 && s.size == Some(4.)));
}
