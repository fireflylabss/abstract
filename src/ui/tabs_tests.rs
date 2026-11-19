use super::*;

#[test]
fn folder_toggles_do_not_discard_tabs_waiting_for_restoration() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    rt.state.restore_started = true;
    rt.state.session.set_tabs(&[SessionNote {
        space: fixture.0.clone(),
        rel: "Note.md".into(),
        cursor: 0,
        scroll: 0.,
    }]);
    rt.state
        .toggle_folder(fixture.0.join("Folder"), &mut Tx::default());
    assert_eq!(rt.state.session.tabs().len(), 1);
    assert_eq!(
        rt.state.session.expanded(&fixture.0),
        [PathBuf::from("Folder")]
    );
    rt.state.writer.flush().unwrap();
}

#[test]
fn expanded_folders_restore_after_shutdown_and_prune_missing_directories() {
    let fixture = Fixture::new();
    let folder = fixture.0.join("Folder");
    let nested = folder.join("Nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(fixture.0.join("Removed")).unwrap();
    std::fs::write(fixture.0.join("Note.md"), "text").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.restore_started && !s.scanning);
    rt.update(|s, tx| {
        s.toggle_folder(folder.clone(), tx);
        s.toggle_folder(nested.clone(), tx);
        s.toggle_folder(fixture.0.join("Removed"), tx);
    });
    until(&mut rt, |s| s.settings_pending == 0);
    let saved = std::fs::read_to_string(fixture.0.join("session")).unwrap();
    assert!(saved.contains("\tFolder/Nested"));
    assert!(!saved.contains(nested.to_str().unwrap()));
    rt.state.shutdown();
    std::fs::remove_dir(fixture.0.join("Removed")).unwrap();
    let mut restored = fixture.runtime();
    restored.state.session = rt.state.session.clone();
    restored.state.session.set_expanded(
        &fixture.0,
        &[
            "Folder".into(),
            "Folder/Nested".into(),
            "Removed".into(),
            "Note.md".into(),
        ],
    );
    until(&mut restored, |s| s.restore_started && !s.scanning);
    assert_eq!(
        restored.state.expanded,
        HashSet::from([folder.clone(), nested.clone()])
    );
    restored.update(|s, tx| s.toggle_folder(folder.clone(), tx));
    restored.state.shutdown();
    assert_eq!(
        restored.state.session.expanded(&fixture.0),
        [PathBuf::from("Folder/Nested")]
    );
    let mut again = fixture.runtime();
    again.state.session = restored.state.session.clone();
    until(&mut again, |s| s.restore_started && !s.scanning);
    assert_eq!(again.state.expanded, HashSet::from([nested]));
}

#[test]
fn reopened_saved_tabs_write_new_edits_via_autosave_and_quit() {
    for quit in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.0.join("Reopened.md");
        std::fs::write(&path, "original").unwrap();
        let mut rt = fixture.runtime();
        rt.update(|s, tx| s.open(path.clone(), tx));
        until(&mut rt, |s| s.document().is_some());
        rt.update(|s, tx| {
            s.document_mut().unwrap().edit.insert(" first");
            s.edited(tx);
            s.document_mut().unwrap().edit.insert(" second");
            s.edited(tx);
            s.save_now(tx);
        });
        until(&mut rt, |s| {
            matches!(s.document().unwrap().save, SaveState::Clean { revision: 2 })
        });
        rt.update(|s, tx| s.close_tab(&path, tx));
        until(&mut rt, |s| !s.documents.contains_key(&path));
        rt.update(|s, tx| s.open(path.clone(), tx));
        until(&mut rt, |s| s.document().is_some());
        rt.update(|s, tx| {
            s.document_mut().unwrap().edit.insert(" reopened");
            s.edited(tx);
        });
        if quit {
            rt.state.shutdown();
        } else {
            for _ in 0..60 {
                rt.frame(1. / 60.);
            }
            rt.state.writer.flush().unwrap();
            rt.state.completions();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "original first second reopened",
            "quit={quit}"
        );
        assert!(matches!(
            rt.state.document().unwrap().save,
            SaveState::Clean { .. }
        ));
    }
}

fn three(fixture: &Fixture) -> (Runtime<State>, Vec<PathBuf>) {
    let paths: Vec<_> = ["One.md", "Two.md", "Three.md"]
        .iter()
        .map(|n| fixture.0.join(n))
        .collect();
    for path in &paths {
        std::fs::write(path, "hello café").unwrap();
    }
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(paths[0].clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    for path in &paths {
        rt.update(|s, tx| s.open(path.clone(), tx));
        until(&mut rt, |s| s.active.as_ref() == Some(path));
    }
    (rt, paths)
}

#[test]
fn switching_preserves_buffers_selection_undo_and_inactive_autosave() {
    let fixture = Fixture::new();
    let (mut rt, paths) = three(&fixture);
    rt.update(|s, tx| {
        s.open(paths[0].clone(), tx);
        let doc = s.document_mut().unwrap();
        doc.edit.set_caret(5);
        doc.edit.insert(" edited");
        s.edited(tx);
        s.open(paths[1].clone(), tx);
    });
    until(&mut rt, |s| {
        matches!(
            s.documents[&paths[0]].save,
            SaveState::Clean { revision: 1 }
        )
    });
    assert_eq!(
        std::fs::read_to_string(&paths[0]).unwrap(),
        "hello edited café"
    );
    rt.update(|s, tx| s.open(paths[0].clone(), tx));
    assert_eq!(rt.state.document().unwrap().edit.focus, 12);
    assert_eq!(rt.state.tabs.paths().len(), 3);
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.undo();
        s.edited(tx);
    });
    assert_eq!(rt.state.document().unwrap().edit.text, "hello café");
    assert_eq!(rt.state.documents[&paths[1]].revision, 0);
    rt.state.shutdown();
    assert_eq!(std::fs::read_to_string(&paths[0]).unwrap(), "hello café");
}

#[test]
fn shortcuts_close_flush_and_session_roundtrip() {
    let fixture = Fixture::new();
    let (mut rt, paths) = three(&fixture);
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
        press(&mut rt, RuneKey::Command('1'), mods);
        assert_eq!(rt.state.active.as_ref(), Some(&paths[0]));
        press(&mut rt, RuneKey::Command('9'), mods);
        assert_eq!(rt.state.active.as_ref(), Some(&paths[2]));
    }
    press(
        &mut rt,
        RuneKey::Tab,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[0]));
    press(
        &mut rt,
        RuneKey::Tab,
        Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[2]));
    press(
        &mut rt,
        RuneKey::PageUp,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[1]));
    press(
        &mut rt,
        RuneKey::PageDown,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[2]));
    press(
        &mut rt,
        RuneKey::Command('['),
        Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[1]));
    press(
        &mut rt,
        RuneKey::Command(']'),
        Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[2]));
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" unsaved");
        s.edited(tx);
    });
    press(
        &mut rt,
        RuneKey::Command('w'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.active.as_ref(), Some(&paths[1]));
    until(&mut rt, |s| !s.documents.contains_key(&paths[2]));
    assert_eq!(
        std::fs::read_to_string(&paths[2]).unwrap(),
        "hello café unsaved"
    );
    rt.state.shutdown();
    let saved = std::fs::read_to_string(fixture.0.join("session")).unwrap();
    let mut restored = fixture.runtime();
    restored.state.session.kv = crate::core::store::KeyVals::parse(&saved);
    until(&mut restored, |s| {
        s.tabs.paths().len() == 2 && s.active.as_ref() == Some(&paths[1])
    });
    assert_eq!(restored.state.tabs.paths(), &paths[..2]);
}

#[test]
fn restore_drops_missing_and_unsafe_paths_and_clamps_unicode_cursor() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Café.md"), "é").unwrap();
    let mut rt = fixture.runtime();
    rt.state.session.set_tabs(
        &["Missing.md", "../outside.md", "Café.md"].map(|rel| SessionNote {
            space: fixture.0.clone(),
            rel: rel.into(),
            cursor: 1,
            scroll: f32::NAN,
        }),
    );
    until(&mut rt, |s| s.document().is_some());
    assert_eq!(rt.state.tabs.paths(), &[fixture.0.join("Café.md")]);
    assert_eq!(rt.state.document().unwrap().edit.focus, 0);
    assert!(rt.state.error.is_none());
    let path = rt.state.active.clone().unwrap();
    rt.update(|s, tx| s.close_tab(&path, tx));
    for _ in 0..3 {
        rt.frame(1. / 60.);
    }
    assert!(rt.state.active.is_none());
    assert!(rt.state.tabs.paths().is_empty());
    rt.state.shutdown();
    let mut restored = fixture.runtime();
    restored.state.session = rt.state.session.clone();
    until(&mut restored, |s| {
        s.restore_started && !s.scanning && s.files_pending == 0
    });
    assert!(restored.state.tabs.paths().is_empty());
    assert!(restored.state.active.is_none());
}

#[test]
fn failed_close_keeps_buffer_recoverable_and_quit_flushes_every_tab() {
    let fixture = Fixture::new();
    let (mut rt, paths) = three(&fixture);
    rt.update(|s, tx| {
        s.open(paths[0].clone(), tx);
        s.document_mut().unwrap().edit.insert(" A");
        s.edited(tx);
        s.open(paths[1].clone(), tx);
        s.document_mut().unwrap().edit.insert(" B");
        s.edited(tx);
    });
    rt.state.shutdown();
    assert_eq!(std::fs::read_to_string(&paths[0]).unwrap(), "hello café A");
    assert_eq!(std::fs::read_to_string(&paths[1]).unwrap(), "hello café B");
    std::fs::remove_file(&paths[1]).unwrap();
    std::fs::create_dir(&paths[1]).unwrap();
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" recover");
        s.edited(tx);
        s.close_tab(&paths[1], tx);
    });
    until(&mut rt, |s| s.error.is_some());
    assert!(rt.state.tabs.paths().contains(&paths[1]));
    assert_eq!(
        rt.state.documents[&paths[1]].edit.text,
        "hello café B recover"
    );
    std::fs::remove_dir(&paths[1]).unwrap();
    rt.state.shutdown();
    assert_eq!(
        std::fs::read_to_string(&paths[1]).unwrap(),
        "hello café B recover"
    );
}

#[test]
fn switching_preserves_viewport_scroll() {
    let fixture = Fixture::new();
    let (mut rt, paths) = three(&fixture);
    rt.update(|s, tx| {
        s.open(paths[0].clone(), tx);
        let doc = s.document_mut().unwrap();
        doc.edit.set_selection(0, doc.edit.text.len());
        doc.edit.insert(&"paragraph\n".repeat(200));
        doc.edit.set_caret(0);
        s.edited(tx);
    });
    rt.frame(1. / 60.);
    rt.update(|_, tx| tx.scroll_to_y("editor-scroll", 500.));
    rt.frame(1. / 60.);
    let before = rt.state.document().unwrap().scroll.get();
    assert!(before > 400., "{before}");
    rt.update(|s, tx| s.open(paths[1].clone(), tx));
    rt.frame(1. / 60.);
    rt.update(|s, tx| s.open(paths[0].clone(), tx));
    rt.frame(1. / 60.);
    assert!((rt.state.document().unwrap().scroll.get() - before).abs() < 1.);
    rt.state.shutdown();
    let mut restored = fixture.runtime();
    restored.state.session = rt.state.session.clone();
    until(&mut restored, |s| s.active.as_ref() == Some(&paths[0]));
    assert_eq!(restored.state.document().unwrap().edit.focus, 0);
    assert!((restored.state.document().unwrap().scroll.get() - before).abs() < 1.);
}

#[test]
fn new_and_close_tab_shortcuts_accept_both_modifiers() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| !s.scanning && s.files_pending == 0);
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
        press(&mut rt, RuneKey::Command('t'), mods);
        until(&mut rt, |s| s.tabs.paths().len() == 1);
        press(&mut rt, RuneKey::Command('w'), mods);
        assert!(rt.state.tabs.paths().is_empty());
        assert!(rt.state.active.is_none());
    }
}
