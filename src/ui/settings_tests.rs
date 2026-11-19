use super::*;

#[test]
fn legacy_font_controls_step_from_effective_font() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| {
        s.settings.kv.set("font", "Georgia");
        for key in ["ui_font", "editor_font"] {
            assert_eq!(s.settings.preference(key), "Georgia");
            s.change_preference(key, true, tx);
            assert_eq!(s.settings.preference(key), "Helvetica");
        }
    });
    until(&mut rt, |s| s.settings_pending == 0);
    let saved = std::fs::read_to_string(fixture.0.join("settings")).unwrap();
    assert!(saved.contains("ui_font = Helvetica"));
    assert!(saved.contains("editor_font = Helvetica"));
}

#[test]
fn smart_dash_uses_current_line_not_later_table_rows() {
    for (source, offset, expected) in [
        ("prose-\n| table |", 6, "prose—\n| table |"),
        ("prose- | table |", 6, "prose-- | table |"),
        ("prose-", 6, "prose—"),
        ("", 0, "-"),
    ] {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("Note.md"), source).unwrap();
        let mut rt = fixture.runtime();
        until(&mut rt, |s| s.document().is_some());
        rt.update(|s, tx| {
            s.settings.kv.set("smart_quotes", "on");
            s.document_mut().unwrap().edit.set_selection(offset, offset);
            tx.focus("editor", false);
        });
        press(&mut rt, RuneKey::Text("-".into()), Mods::default());
        assert_eq!(rt.state.document().unwrap().edit.text, expected);
    }
}

#[test]
fn settings_shortcuts_live_changes_persist_and_restore_editor_without_editing() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "# Heading\nbody").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    for mods in [
        Mods {
            cmd: true,
            ..Mods::default()
        },
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    ] {
        press(&mut rt, RuneKey::Text(",".into()), mods);
        assert!(rt.state.settings_open());
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some("settings"))
        );
        press(&mut rt, RuneKey::Escape, Mods::default());
        assert!(!rt.state.settings_open());
    }
    let original = rt.state.document().unwrap().edit.text.clone();
    rt.update(|s, tx| {
        tx.instant();
        s.settings.kv.set("future", "retained");
        s.toggle_settings(tx);
        for key in [
            "theme",
            "accent",
            "ui_font",
            "editor_font",
            "editor_size",
            "line_height",
            "conceal",
            "caret_blink",
            "autosave_delay",
            "single_tab",
        ] {
            s.change_preference(key, true, tx);
        }
    });
    until(&mut rt, |s| s.settings_pending == 0);
    assert_eq!(rt.state.document().unwrap().edit.text, original);
    assert_eq!(rt.state.editor_style().size, 18.);
    assert_eq!(
        rt.state.editor_style().family,
        rune::Family::Named("Noto Sans Mono")
    );
    assert!(!rt.state.settings.enabled("conceal"));
    assert!(
        !rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("tab-strip"))
    );
    let saved = std::fs::read_to_string(fixture.0.join("settings")).unwrap();
    for line in [
        "future = retained",
        "editor_size = 18",
        "conceal = off",
        "autosave_delay = 0.7",
    ] {
        assert!(saved.contains(line), "{saved}");
    }
    let restored = Settings {
        kv: crate::core::store::KeyVals::parse(&saved),
    };
    assert_eq!(restored.number("editor_size"), 18.);
    rt.update(|s, tx| {
        tx.instant();
        s.toggle_settings(tx);
    });
    rt.frame(1. / 60.);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("custom-caret"))
    );
}

#[test]
fn empty_space_settings_and_failed_persistence_are_recoverable() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| {
        s.settings_path = fixture.0.clone();
        s.toggle_settings(tx);
        s.change_preference("editor_size", true, tx);
    });
    until(&mut rt, |s| s.error.is_some());
    assert!(rt.state.settings_open());
    assert_eq!(rt.state.settings.number("editor_size"), 18.);
    assert!(rt.state.document().is_none());
    rt.update(|s, tx| {
        s.settings_path = fixture.0.join("settings");
        s.change_preference("editor_size", true, tx);
    });
    until(&mut rt, |s| s.settings_pending == 0);
    assert!(
        std::fs::read_to_string(fixture.0.join("settings"))
            .unwrap()
            .contains("editor_size = 19")
    );
}

#[test]
fn smart_pairing_and_quotes_are_keyboard_only_and_undoable() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.change_preference("smart_pairs", true, tx);
        tx.focus("editor", false);
    });
    press(&mut rt, RuneKey::Text("(".into()), Mods::default());
    assert_eq!(rt.state.document().unwrap().edit.text, "()");
    assert_eq!(rt.state.document().unwrap().edit.focus, 1);
    press(&mut rt, RuneKey::Text(")".into()), Mods::default());
    assert_eq!(rt.state.document().unwrap().edit.text, "()");
    assert_eq!(rt.state.document().unwrap().edit.focus, 2);
    press(
        &mut rt,
        RuneKey::Text("z".into()),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    assert!(rt.state.document().unwrap().edit.text.is_empty());
    rt.paste("[paste]");
    rt.frame(1. / 60.);
    assert_eq!(rt.state.document().unwrap().edit.text, "[paste]");
    rt.update(|s, tx| {
        s.change_preference("smart_pairs", true, tx);
    });
    press(&mut rt, RuneKey::Text("\"".into()), Mods::default());
    assert_eq!(rt.state.document().unwrap().edit.text, "[paste]”");
}

#[test]
fn autosave_uses_selected_delay_and_zoom_scales_the_whole_runtime() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "text").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.settings.kv.set("autosave_delay", "2");
        s.settings.set_zoom(1.5);
        s.document_mut().unwrap().edit.insert(" edited");
        s.edited(tx);
        let SaveState::Pending { due, .. } = s.document().unwrap().save else {
            panic!("pending save");
        };
        assert!((due - tx.clock() - 2.).abs() < 0.001);
    });
    rt.frame(1. / 60.);
    assert_eq!(rt.zoom(), 1.5);
    rt.update(|s, tx| {
        s.change_preference("autosave_delay", false, tx);
    });
    rt.update(|s, tx| {
        let SaveState::Pending { due, .. } = s.document().unwrap().save else {
            panic!("pending save");
        };
        assert!((due - tx.clock() - 1.9).abs() < 0.001);
    });
}

#[test]
fn keyboard_groups_and_nonblinking_caret_track_zoomed_editor_geometry() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "# Heading\nbody").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    press(
        &mut rt,
        RuneKey::Command(','),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    press(&mut rt, RuneKey::Tab, Mods::default());
    press(&mut rt, RuneKey::Text(" ".into()), Mods::default());
    assert_eq!(rt.state.customization.group, 1);
    press(&mut rt, RuneKey::Escape, Mods::default());
    rt.update(|s, tx| {
        tx.instant();
        s.settings.set_zoom(1.5);
        s.change_preference("caret_blink", true, tx);
        tx.focus("editor", false);
    });
    rt.frame(1. / 60.);
    let elements = rt.elements();
    let editor = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("editor") && !e.exiting)
        .unwrap();
    let caret = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("custom-caret") && !e.exiting)
        .unwrap();
    let expected = editor.caret.unwrap();
    for (actual, expected) in caret.rect.iter().zip(expected) {
        assert!((actual - expected).abs() < 2., "{actual} vs {expected}");
    }
}
