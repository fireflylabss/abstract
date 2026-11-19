use super::*;

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..4 {
        rt.frame(1. / 60.);
    }
}

#[test]
fn markdown_decorations_keep_editable_source_and_track_cursor() {
    let fixture = Fixture::new();
    let source = "- item\n- [x] done\n\n> quote\n\n```rust\nfn main() {}\n```\n";
    std::fs::write(fixture.0.join("Note.md"), source).unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    settle(&mut rt);
    assert_eq!(rt.state.document().unwrap().edit.text, source);
    let elements = rt.elements();
    let markers: Vec<_> = elements
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|key| key.starts_with("md-marker-"))
        })
        .collect();
    assert_eq!(markers.len(), 2);
    assert!(markers.iter().all(|e| e.rect[2] == 14. && e.rect[3] == 14.));
    assert!(markers.iter().any(|e| e.fill == rgb(theme::LIGHT.fg)));
    let editor = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("editor"))
        .unwrap();
    assert!(
        elements
            .iter()
            .any(|e| e.fill == rgb(theme::LIGHT.code_bg) && e.rect[2] > editor.rect[2])
    );
    assert!(
        elements
            .iter()
            .any(|e| e.fill == rgb(theme::LIGHT.line) && e.rect[2] == 2.)
    );
    rt.update(|s, _| s.document_mut().unwrap().edit.set_caret(1));
    settle(&mut rt);
    assert_eq!(
        rt.elements()
            .iter()
            .filter(|e| e
                .key
                .as_deref()
                .is_some_and(|key| key.starts_with("md-marker-")))
            .count(),
        1
    );
    assert_eq!(rt.state.document().unwrap().edit.text, source);
}

#[test]
fn toolbar_and_single_tab_default_preserve_actions_and_empty_state() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| !s.scanning);
    let elements = rt.elements();
    let trash = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("note-trash"))
        .unwrap();
    let (x, y) = (trash.rect[0] + 10., trash.rect[1] + 10.);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 60.);
    assert!(matches!(rt.state.file_ops.mode, file_ops::Mode::Idle));
    std::fs::write(fixture.0.join("Note.md"), "words stay editable").unwrap();
    rt.update(|s, tx| s.open(fixture.0.join("Note.md"), tx));
    until(&mut rt, |s| s.document().is_some());
    settle(&mut rt);
    let elements = rt.elements();
    assert!(elements.iter().all(|e| {
        e.label
            .as_ref()
            .is_none_or(|label| !label.contains("{MOD}") && !label.contains("{name}"))
    }));
    assert!(
        !elements
            .iter()
            .any(|e| matches!(e.key.as_deref(), Some("tab-strip" | "settings-open")))
    );
    for key in [
        "new-note",
        "new-folder",
        "sidebar-toggle",
        "search-open",
        "theme-toggle",
    ] {
        assert!(
            elements.iter().any(|e| e.key.as_deref() == Some(key)),
            "{key}"
        );
    }
    let (x, y) = rt.find("note-trash").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert!(matches!(rt.state.file_ops.mode, file_ops::Mode::Confirm(_)));
}
