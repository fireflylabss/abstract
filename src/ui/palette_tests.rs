use super::super::{commands, palette};
use super::*;

fn note_runtime(fixture: &Fixture) -> Runtime<State> {
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "cat cat").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    rt
}

#[test]
fn spelling_shortcut_preserves_rename_draft() {
    let fixture = Fixture::new();
    let mut rt = note_runtime(&fixture);
    rt.update(|s, tx| {
        s.settings.set_spellcheck(true);
        s.settings.kv.set("spell_lang", "en");
        s.document_mut().unwrap().edit = TextEdit::new("speling");
        s.edited(tx);
    });
    until(&mut rt, |s| s.spelling_available());
    rt.update(|s, tx| {
        s.rename_file(s.active.clone().unwrap(), false, false, tx);
        if let file_ops::Mode::Rename { edit, .. } = &mut s.file_ops.mode {
            **edit = TextEdit::new("Uncommitted draft");
        }
    });
    press(&mut rt, RuneKey::Function(7), Mods::default());
    assert!(
        matches!(&rt.state.file_ops.mode, file_ops::Mode::Rename { edit, .. } if edit.text == "Uncommitted draft")
    );
}

#[test]
fn follow_shortcut_in_settings_does_not_open_hidden_note_link() {
    let fixture = Fixture::new();
    let target = fixture.0.join("Target.md");
    std::fs::write(&target, "target").unwrap();
    let mut rt = note_runtime(&fixture);
    let original = rt.state.active.clone();
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit = TextEdit::new("[[Target]]");
        s.document_mut().unwrap().edit.set_caret(3);
        s.edited(tx);
        s.toggle_settings(tx);
    });
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            alt: true,
            ..Mods::default()
        },
    );
    until(&mut rt, |s| !s.navigation.busy);
    assert_eq!(rt.state.active, original);
    assert_eq!(rt.state.tabs.paths().len(), 1);
}

#[test]
fn pointer_can_scroll_and_open_notes_beyond_the_first_page() {
    let fixture = Fixture::new();
    let target = fixture.0.join("Note 20.md");
    std::fs::write(&target, "Note 20").unwrap();
    let mut rt = note_runtime(&fixture);
    until(&mut rt, |s| !s.scanning);
    rt.update(|s, tx| {
        s.tree.clear();
        for i in 0..80 {
            let name = format!("Note {i:02}");
            let path = fixture.0.join(format!("{name}.md"));
            s.tree.push(vault::Node {
                path,
                name,
                kind: vault::NodeKind::Note,
                modified: std::time::SystemTime::UNIX_EPOCH,
                children: vec![],
            });
        }
        s.open_palette(false, tx);
        tx.instant();
    });
    rt.frame(1. / 60.);
    let first = rt.presented("palette-row-0").unwrap().0;
    rt.set_wheel_glide(false);
    rt.wheel(first[0] + 8., first[1] + 8., -600.);
    for _ in 0..30 {
        rt.frame(1. / 60.);
    }
    let later = rt
        .presented("palette-row-20")
        .expect("wheel reveals later notes")
        .0;
    assert!(later[1] >= first[1] && later[1] + later[3] <= 760.);
    let (x, y) = (later[0] + later[2] / 2., later[1] + later[3] / 2.);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 60.);
    until(&mut rt, |s| s.active.as_ref() == Some(&target));
    assert!(rt.state.palette.open.is_none());
    assert_eq!(rt.state.active, Some(target));
}

#[test]
fn palette_keyboard_keeps_selection_within_small_viewport() {
    let fixture = Fixture::new();
    let mut rt = note_runtime(&fixture);
    rt.viewport = (700., 240.);
    rt.frame(1. / 60.);
    rt.update(|s, tx| {
        s.open_palette(true, tx);
        tx.instant();
    });
    rt.frame(1. / 60.);
    let count = palette::entries(&rt.state).len();
    press(&mut rt, RuneKey::Up, Mods::default());
    for _ in 0..5 {
        rt.frame(1. / 60.);
    }
    let last = rt
        .presented(&format!("palette-row-{}", count - 1))
        .unwrap()
        .0;
    let viewport = rt.presented("palette-results").unwrap().0;
    assert!(viewport[1] + viewport[3] <= 240.);
    assert!(last[1] >= viewport[1] && last[1] + last[3] <= viewport[1] + viewport[3] + 0.1);
    press(&mut rt, RuneKey::Down, Mods::default());
    assert_eq!(rt.state.palette.open.as_ref().unwrap().selected, 0);
    assert!(rt.presented("palette-row-0").is_some());
}

#[test]
fn follow_shortcut_in_replacement_does_not_replace_text() {
    let fixture = Fixture::new();
    let mut rt = note_runtime(&fixture);
    rt.update(|s, tx| {
        s.open_find(true, tx);
        s.find.as_mut().unwrap().query = TextEdit::new("cat");
        s.find.as_mut().unwrap().replacement = TextEdit::new("dog");
        s.refresh_find(true, tx);
        tx.focus("find-replacement", false);
    });
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            alt: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.document().unwrap().edit.text, "cat cat");
}

#[test]
fn palette_trash_requests_confirmation_before_touching_disk() {
    let fixture = Fixture::new();
    let mut rt = note_runtime(&fixture);
    let path = rt.state.active.clone().unwrap();
    rt.update(|s, tx| {
        s.open_palette(true, tx);
        s.palette.open.as_mut().unwrap().selected = palette::entries(s)
            .iter()
            .position(|e| e.target == palette::Target::Command("trash"))
            .unwrap();
    });
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert!(matches!(&rt.state.file_ops.mode, file_ops::Mode::Confirm(p) if p == &path));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "cat cat");
}

#[test]
fn palette_blocks_function_shortcuts_behind_modal() {
    let fixture = Fixture::new();
    let mut rt = note_runtime(&fixture);
    rt.update(|s, tx| {
        s.open_find(false, tx);
        s.find.as_mut().unwrap().query = TextEdit::new("cat");
        s.refresh_find(true, tx);
        s.open_palette(true, tx);
    });
    let current = rt.state.find.as_ref().unwrap().current;
    press(&mut rt, RuneKey::Function(3), Mods::default());
    assert_eq!(rt.state.find.as_ref().unwrap().current, current);
    press(&mut rt, RuneKey::Function(2), Mods::default());
    assert!(matches!(rt.state.file_ops.mode, file_ops::Mode::Idle));
}

#[test]
fn palette_dismissal_restores_each_query_focus() {
    for field in ["find-query", "find-replacement", "global-query"] {
        let fixture = Fixture::new();
        let mut rt = note_runtime(&fixture);
        rt.update(|s, tx| {
            if field == "global-query" {
                s.open_search(tx);
            } else {
                s.open_find(true, tx);
            }
            tx.focus(field, false);
        });
        press(
            &mut rt,
            RuneKey::Text("p".into()),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        );
        press(&mut rt, RuneKey::Escape, Mods::default());
        rt.paste("query");
        rt.frame(1. / 60.);
        let query = match field {
            "global-query" => &rt.state.search.as_ref().unwrap().query.text,
            "find-replacement" => &rt.state.find.as_ref().unwrap().replacement.text,
            _ => &rt.state.find.as_ref().unwrap().query.text,
        };
        assert_eq!(query, "query", "{field}");
        assert_eq!(rt.state.document().unwrap().edit.text, "cat cat", "{field}");
    }
}

#[test]
fn unavailable_commands_remain_disabled_and_do_not_dismiss_palette() {
    let fixture = Fixture::new();
    let mut rt = note_runtime(&fixture);
    for id in ["find-next", "find-prev", "spelling"] {
        rt.update(|s, tx| {
            s.open_find(false, tx);
            s.open_palette(true, tx);
            let entries = palette::entries(s);
            let index = entries
                .iter()
                .position(|e| e.target == palette::Target::Command(id))
                .unwrap();
            s.palette.open.as_mut().unwrap().selected = index;
            assert!(!entries[index].enabled, "{id}");
        });
        press(&mut rt, RuneKey::Enter, Mods::default());
        assert!(rt.state.palette.open.is_some(), "{id}");
        press(&mut rt, RuneKey::Escape, Mods::default());
    }
}

#[test]
fn registry_has_unique_ids_bindings_and_localized_labels() {
    let mut ids = HashSet::new();
    for c in commands::ALL {
        assert!(ids.insert(c.id), "{}", c.id);
        for lang in Lang::ALL {
            assert!(!i18n::lookup(lang, c.label).is_empty());
        }
    }
    for cmd in [true, false] {
        for shift in [true, false] {
            for c in ['p', 'n', 'f', 'e', 'z', 'b', 'd', 'g', 's', 'q'] {
                let event = KeyEvent {
                    key: RuneKey::Text(c.to_string()),
                    mods: Mods {
                        cmd,
                        ctrl: !cmd,
                        shift,
                        ..Mods::default()
                    },
                };
                assert!(
                    commands::ALL
                        .iter()
                        .filter(|c| c.bindings.iter().any(|b| b.matches(&event)))
                        .count()
                        <= 1
                );
            }
        }
    }
}

#[test]
fn both_primary_forms_open_notes_and_commands_and_escape_dismisses() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    for cmd in [true, false] {
        for shift in [true, false] {
            press(
                &mut rt,
                RuneKey::Text("p".into()),
                Mods {
                    cmd,
                    ctrl: !cmd,
                    shift,
                    ..Mods::default()
                },
            );
            assert_eq!(
                rt.state.palette.open.as_ref().unwrap().query.text,
                if shift { ">" } else { "" }
            );
            press(&mut rt, RuneKey::Escape, Mods::default());
            assert!(rt.state.palette.open.is_none());
        }
    }
}

#[test]
fn palette_preserves_native_query_shortcuts_without_running_app_commands() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    for cmd in [true, false] {
        let mods = Mods {
            cmd,
            ctrl: !cmd,
            ..Mods::default()
        };
        press(&mut rt, RuneKey::Text("p".into()), mods);
        press(&mut rt, RuneKey::Text("old query".into()), Mods::default());
        press(
            &mut rt,
            RuneKey::Command('a'),
            Mods {
                cmd: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Mods::default()
            },
        );
        press(&mut rt, RuneKey::Text(">sidebar".into()), Mods::default());
        assert_eq!(
            rt.state.palette.open.as_ref().unwrap().query.text,
            ">sidebar"
        );
        let tabs = rt.state.tabs.paths().to_vec();
        press(&mut rt, RuneKey::Text("n".into()), mods);
        assert_eq!(rt.state.tabs.paths(), tabs);
        assert!(rt.state.palette.open.is_some());
        press(&mut rt, RuneKey::Escape, Mods::default());
    }
}

#[test]
fn commands_share_actions_recent_order_and_disabled_predicates() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| {
        s.tree.clear();
        s.documents.clear();
        s.active = None;
        s.open_palette(true, tx);
        let entries = palette::entries(s);
        assert!(
            !entries
                .iter()
                .find(|e| e.target == palette::Target::Command("bold"))
                .unwrap()
                .enabled
        );
        let before = s.sidebar_open;
        assert!(commands::run(s, "sidebar", tx));
        assert_eq!(s.sidebar_open, !before);
        assert_eq!(
            palette::entries(s)[0].target,
            palette::Target::Command("sidebar")
        );
        commands::run(s, "palette", tx);
        assert_eq!(s.palette.recent, ["sidebar"]);
        assert!(!commands::run(s, "not-a-command", tx));
        s.palette.open.as_mut().unwrap().query.text = ">🦀".into();
        assert!(palette::entries(s).is_empty());
        assert!(s.palette_key(
            &KeyEvent {
                key: RuneKey::Enter,
                mods: Mods::default()
            },
            tx
        ));
        assert!(s.palette.open.is_some());
    });
}

#[test]
fn notes_include_collapsed_descendants_without_reading_disk_and_activate() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Ideas/Hidden.md");
    let mut rt = fixture.runtime();
    rt.update(|s, tx| {
        s.tree = vec![vault::Node {
            path: fixture.0.join("Ideas"),
            name: "Ideas".into(),
            kind: vault::NodeKind::Folder,
            modified: std::time::SystemTime::UNIX_EPOCH,
            children: vec![vault::Node {
                path: path.clone(),
                name: "Hidden".into(),
                kind: vault::NodeKind::Note,
                modified: std::time::SystemTime::UNIX_EPOCH,
                children: vec![],
            }],
        }];
        s.expanded.clear();
        s.load_document(path.clone(), "unsaved content".into());
        s.open_palette(false, tx);
        assert_eq!(
            palette::entries(s)[0].target,
            palette::Target::Note(path.clone())
        );
        s.palette_key(
            &KeyEvent {
                key: RuneKey::Down,
                mods: Mods::default(),
            },
            tx,
        );
        s.palette_key(
            &KeyEvent {
                key: RuneKey::Enter,
                mods: Mods::default(),
            },
            tx,
        );
        assert!(s.palette.open.is_none());
        assert_eq!(s.active.as_ref(), Some(&path));
        assert_eq!(s.document().unwrap().edit.text, "unsaved content");
    });
    assert!(!path.exists());
}
