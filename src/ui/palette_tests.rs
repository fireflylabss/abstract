use super::super::{commands, palette};
use super::*;

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
