use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use rune::{Env, Key as RuneKey, KeyEvent, Mods, Runtime, text::TextSystem};

use super::*;

#[path = "tabs_tests.rs"]
mod tabs;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "abstract-ui-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn runtime(&self) -> Runtime<State> {
        let mut scans = Scans::new();
        scans.start(self.0.clone());
        let state = State {
            settings: Settings::default(),
            root: self.0.clone(),
            tree: vec![],
            expanded: HashSet::new(),
            active: None,
            error: None,
            sidebar_open: true,
            focus_mode: false,
            documents: HashMap::new(),
            writer: Writer::new(),
            scans,
            scanning: true,
            files: Files::new(),
            file_ticket: 0,
            files_pending: 0,
            session: Session::default(),
            session_path: self.0.join("session"),
            settings_path: self.0.join("settings"),
            settings_pending: 0,
            tabs: Tabs::default(),
            hovered_tab: None,
            restore_started: false,
            edit_revision: 0,
        };
        let mut rt = Runtime::with_text(
            state,
            view,
            (1100., 760.),
            Env::new(false, 1.),
            TextSystem::new(FONTS, ("Noto Sans", "Noto Sans Mono")),
        );
        rt.poll = Some(State::poll);
        rt.on_key = Some(keymap::handle);
        rt
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn until(rt: &mut Runtime<State>, condition: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        rt.frame(1. / 60.);
        if condition(&rt.state) {
            return;
        }
        assert!(Instant::now() < deadline, "background work did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn press(rt: &mut Runtime<State>, key: RuneKey, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1. / 60.);
}

#[test]
fn focused_editor_routes_markdown_keys_before_native_editing_and_undoes_once() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "- first").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert_eq!(rt.state.document().unwrap().edit.text, "- first\n- ");
    assert_eq!(rt.state.document().unwrap().revision, 1);
    press(
        &mut rt,
        RuneKey::Command('z'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.document().unwrap().edit.text, "- first");
    assert_eq!(rt.state.document().unwrap().edit.focus, 7);
    press(&mut rt, RuneKey::Tab, Mods::default());
    assert_eq!(rt.state.document().unwrap().edit.text, "  - first");
    press(
        &mut rt,
        RuneKey::Tab,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.document().unwrap().edit.text, "- first");
}

#[test]
fn formatting_and_task_commands_accept_command_and_control_forms() {
    for control in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.0.join("Note.md");
        std::fs::write(&path, "- [ ] café").unwrap();
        let mut rt = fixture.runtime();
        rt.update(|s, tx| s.open(path.clone(), tx));
        until(&mut rt, |s| s.document().is_some());
        let mods = Mods {
            cmd: !control,
            ctrl: control,
            ..Mods::default()
        };
        press(&mut rt, RuneKey::Enter, mods);
        assert_eq!(rt.state.document().unwrap().edit.text, "- [x] café");
        for (character, shift, expected) in [
            ('b', false, "**café**"),
            ('i', false, "*café*"),
            ('e', false, "`café`"),
            ('x', true, "~~café~~"),
            ('k', false, "[café]()"),
            ('h', true, "==café=="),
            ('/', false, "%%café%%"),
        ] {
            rt.update(|s, _| s.document_mut().unwrap().edit.set_selection(6, 11));
            let key = if control {
                RuneKey::Text(character.to_string())
            } else {
                RuneKey::Command(character)
            };
            press(&mut rt, key, Mods { shift, ..mods });
            assert_eq!(
                rt.state.document().unwrap().edit.text,
                format!("- [x] {expected}")
            );
            let undo = if control {
                RuneKey::Text("z".into())
            } else {
                RuneKey::Command('z')
            };
            press(&mut rt, undo, mods);
            assert_eq!(rt.state.document().unwrap().edit.text, "- [x] café");
        }
    }
}

#[test]
fn inapplicable_commands_are_handled_without_dirtying_or_undo_history() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "prose").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    press(
        &mut rt,
        RuneKey::Tab,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.document().unwrap().edit.text, "prose");
    assert_eq!(rt.state.document().unwrap().revision, 0);
}

#[test]
fn app_shortcuts_zoom_theme_sidebar_focus_and_persist_preferences() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    let primary = Mods {
        cmd: true,
        ..Mods::default()
    };
    press(&mut rt, RuneKey::Command('='), primary);
    assert!((rt.state.settings.zoom() - 1.1).abs() < 0.001);
    press(&mut rt, RuneKey::Command('-'), primary);
    assert!((rt.state.settings.zoom() - 1.).abs() < 0.001);
    for _ in 0..20 {
        press(&mut rt, RuneKey::Command('+'), primary);
    }
    assert_eq!(rt.state.settings.zoom(), 2.);
    press(&mut rt, RuneKey::Command('0'), primary);
    assert_eq!(rt.state.settings.zoom(), 1.);
    press(
        &mut rt,
        RuneKey::Command('l'),
        Mods {
            shift: true,
            ..primary
        },
    );
    assert_eq!(
        rt.state.settings.theme(),
        crate::core::prefs::ThemePref::Light
    );
    press(&mut rt, RuneKey::Command('\\'), primary);
    assert!(!rt.state.sidebar_open);
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            shift: true,
            ..primary
        },
    );
    assert!(rt.state.focus_mode);
    press(&mut rt, RuneKey::Escape, Mods::default());
    assert!(!rt.state.focus_mode);
    assert!(!rt.state.sidebar_open);
    rt.state.shutdown();
    let settings = std::fs::read_to_string(fixture.0.join("settings")).unwrap();
    assert!(settings.contains("zoom = 1.00"), "{settings}");
    assert!(settings.contains("theme = light"), "{settings}");
    let session = std::fs::read_to_string(fixture.0.join("session")).unwrap();
    assert!(session.contains("sidebar = closed"), "{session}");
}

#[test]
fn headless_open_type_autosave_changes_disk_and_preserves_dirty_note_on_switch() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    let other = fixture.0.join("Other.md");
    std::fs::write(&path, "# Heading\nbody").unwrap();
    std::fs::write(&other, "other").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.active.as_ref() == Some(&path));
    rt.key(KeyEvent {
        key: RuneKey::Text(" autosaved".into()),
        mods: Mods::default(),
    });
    rt.frame(1. / 60.);
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "# Heading\nbody autosaved"
    );
    assert_eq!(rt.state.document().unwrap().revision, 1);
    rt.update(|s, tx| s.open(other.clone(), tx));
    until(&mut rt, |s| s.active.as_ref() == Some(&other));
    rt.frame(save::DEBOUNCE + 0.1);
    rt.state.writer.flush().unwrap();
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "# Heading\nbody autosaved"
    );
}

#[test]
fn new_notes_are_empty_and_never_overwrite_existing_files() {
    let fixture = Fixture::new();
    let stem = vault::stem_for_title(t(Key::Untitled));
    let existing = fixture.0.join(format!("{stem}.md"));
    std::fs::write(&existing, "keep me").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.new_note(tx));
    until(&mut rt, |s| {
        s.active.as_ref().is_some_and(|p| p != &existing)
    });
    let path = rt.state.active.as_ref().unwrap();
    assert_eq!(path.file_name().unwrap(), format!("{stem} 2.md").as_str());
    assert!(rt.state.document().unwrap().edit.text.is_empty());
    assert_eq!(std::fs::read_to_string(existing).unwrap(), "keep me");
}

#[test]
fn failed_open_keeps_current_buffer_and_exposes_error() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "keep me").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| s.open(fixture.0.join("missing.md"), tx));
    until(&mut rt, |s| s.error.is_some());
    assert_eq!(rt.state.active, Some(path));
    assert_eq!(rt.state.document().unwrap().edit.text, "keep me");
}

#[test]
fn startup_falls_back_when_restored_note_is_missing_or_unreadable() {
    for restored in ["Missing.md", "Invalid.md"] {
        let fixture = Fixture::new();
        let fallback = fixture.0.join("Available.md");
        std::fs::write(&fallback, "fallback content").unwrap();
        if restored == "Invalid.md" {
            std::fs::write(fixture.0.join(restored), [0xff]).unwrap();
        }
        let mut rt = fixture.runtime();
        rt.state
            .session
            .set_active_tab(Some((&fixture.0, std::path::Path::new(restored))));
        until(&mut rt, |s| s.document().is_some() || s.error.is_some());
        assert_eq!(rt.state.active.as_ref(), Some(&fallback), "{restored}");
        assert_eq!(rt.state.document().unwrap().edit.text, "fallback content");
        assert!(rt.state.error.is_none());
    }
}

#[test]
fn startup_preserves_valid_restoration_and_handles_empty_space() {
    let fixture = Fixture::new();
    let restored = fixture.0.join("Restored.md");
    std::fs::write(&restored, "restored content").unwrap();
    std::fs::write(fixture.0.join("Other.md"), "other content").unwrap();
    let mut rt = fixture.runtime();
    rt.state
        .session
        .set_active_tab(Some((&fixture.0, std::path::Path::new("Restored.md"))));
    until(&mut rt, |s| s.document().is_some());
    assert_eq!(rt.state.active.as_ref(), Some(&restored));
    assert_eq!(rt.state.document().unwrap().edit.text, "restored content");

    let empty = Fixture::new();
    let mut rt = empty.runtime();
    rt.state
        .session
        .set_active_tab(Some((&empty.0, std::path::Path::new("Missing.md"))));
    until(&mut rt, |s| {
        s.restore_started && !s.scanning && s.files_pending == 0
    });
    assert!(rt.state.active.is_none());
    assert!(rt.state.error.is_none());
    assert_eq!(rt.state.files_pending, 0);
}

#[test]
fn cancelled_quit_does_not_skip_later_edits_or_failed_save_retries() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "before quit").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    rt.state.shutdown();
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" after cancellation");
        s.edited(tx);
    });
    rt.state.shutdown();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "before quit after cancellation"
    );
    assert!(matches!(
        rt.state.document().unwrap().save,
        SaveState::Clean { revision: 1 }
    ));

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" retry");
        s.edited(tx);
    });
    rt.state.shutdown();
    assert!(matches!(
        rt.state.document().unwrap().save,
        SaveState::Failed { revision: 2, .. }
    ));
    std::fs::remove_dir(&path).unwrap();
    rt.state.shutdown();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "before quit after cancellation retry"
    );
    assert!(matches!(
        rt.state.document().unwrap().save,
        SaveState::Clean { revision: 2 }
    ));
}
