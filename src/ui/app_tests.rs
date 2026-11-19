use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use rune::{Env, Key as RuneKey, KeyEvent, Mods, Runtime, text::TextSystem};

use super::*;

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
            documents: HashMap::new(),
            writer: Writer::new(),
            scans,
            scanning: true,
            files: Files::new(),
            file_ticket: 0,
            files_pending: 0,
            session: Session::default(),
            session_path: self.0.join("session"),
        };
        let mut rt = Runtime::with_text(
            state,
            view,
            (1100., 760.),
            Env::new(false, 1.),
            TextSystem::new(FONTS, ("Noto Sans", "Noto Sans Mono")),
        );
        rt.poll = Some(State::poll);
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
