use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    atomic,
    save::{self, Effect, Event, State},
    tree::{Generation, Reply},
    writer::{Snapshot, Writer},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "abstract-io-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn b3_ordered_revisioned_writes_do_not_overwrite_newer_text() {
    let dir = Dir::new();
    let path = dir.0.join("note.md");
    let writer = Writer::new();
    for revision in [1, 2, 3, 2, 4] {
        writer
            .submit(Snapshot {
                path: path.clone(),
                revision,
                text: format!("revision {revision}"),
            })
            .unwrap();
    }
    writer.flush().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "revision 4");
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    let revisions: Vec<_> = writer.completions.try_iter().map(|c| c.revision).collect();
    assert_eq!(revisions, [1, 2, 3, 4]);
}

#[test]
fn atomic_writes_support_250_byte_filenames() {
    let dir = Dir::new();
    for name in [
        format!("{}.md", "a".repeat(247)),
        format!("{}a.md", "é".repeat(123)),
    ] {
        assert_eq!(name.len(), 250);
        let path = dir.0.join(name);
        std::fs::write(&path, "original").unwrap();
        atomic::write(&path, b"updated").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "updated");
    }
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 2);
}

#[test]
fn atomic_concurrent_writes_have_unique_temps_and_complete_snapshots() {
    let dir = Dir::new();
    let path = dir.0.join("note.md");
    let threads: Vec<_> = (0..12)
        .map(|i| {
            let path = path.clone();
            std::thread::spawn(move || {
                atomic::write(&path, format!("{i}").repeat(4096).as_bytes()).unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let text = std::fs::read_to_string(path).unwrap();
    assert!((0..12).any(|i| text == format!("{i}").repeat(4096)));
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn b4_old_completion_cannot_acknowledge_new_edit() {
    let mut state = State::Clean { revision: 0 };
    assert_eq!(
        save::step(
            &mut state,
            Event::Edited {
                revision: 1,
                now: 0.
            }
        ),
        [Effect::WakeAt(0.6)]
    );
    assert_eq!(
        save::step(&mut state, Event::Timer(0.6)),
        [Effect::Write(1)]
    );
    save::step(
        &mut state,
        Event::Edited {
            revision: 2,
            now: 1.,
        },
    );
    save::step(
        &mut state,
        Event::Completed {
            revision: 1,
            result: Ok(()),
        },
    );
    assert_eq!(
        state,
        State::Pending {
            revision: 2,
            due: 1.6
        }
    );
    save::step(
        &mut state,
        Event::Completed {
            revision: 1,
            result: Err("old failure".into()),
        },
    );
    assert!(matches!(state, State::Pending { revision: 2, .. }));
    assert_eq!(save::step(&mut state, Event::Flush), [Effect::Write(2)]);
}

#[test]
fn b5_quit_flushes_pending_and_failed_and_reports_errors() {
    let dir = Dir::new();
    let writer = Writer::new();
    let blocked = dir.0.join("blocked");
    std::fs::write(&blocked, "not a directory").unwrap();
    let path = blocked.join("note.md");
    let mut failed = State::Failed {
        revision: 7,
        error: "previous failure".into(),
    };
    let mut pending = State::Pending {
        revision: 8,
        due: 999.,
    };
    for (state, target, text) in [
        (&mut failed, path.clone(), "failed buffer"),
        (&mut pending, dir.0.join("pending.md"), "pending buffer"),
    ] {
        for effect in save::step(state, Event::Flush) {
            if let Effect::Write(revision) = effect {
                writer
                    .submit(Snapshot {
                        path: target.clone(),
                        revision,
                        text: text.into(),
                    })
                    .unwrap();
            }
        }
    }
    assert!(writer.flush().unwrap_err().contains("note.md"));
    assert_eq!(
        std::fs::read_to_string(dir.0.join("pending.md")).unwrap(),
        "pending buffer"
    );
    for c in writer.completions.try_iter() {
        if c.path == path {
            save::step(
                &mut failed,
                Event::Completed {
                    revision: c.revision,
                    result: c.result,
                },
            );
        }
    }
    assert!(matches!(failed, State::Failed { revision: 7, .. }));
    std::fs::remove_file(&blocked).unwrap();
    for effect in save::step(&mut failed, Event::Flush) {
        if let Effect::Write(revision) = effect {
            writer
                .submit(Snapshot {
                    path: path.clone(),
                    revision,
                    text: "failed buffer".into(),
                })
                .unwrap();
        }
    }
    writer.flush().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "failed buffer");
}

#[test]
fn b7_previous_space_scan_is_rejected_even_when_it_finishes_last() {
    let mut generation = Generation::default();
    let old = generation.advance();
    let new = generation.advance();
    assert!(generation.accepts(&Reply {
        generation: new,
        result: Ok(vec![])
    }));
    assert!(!generation.accepts(&Reply {
        generation: old,
        result: Ok(vec![])
    }));
    assert!(!generation.accepts(&Reply {
        generation: old,
        result: Err("old space unreadable".into())
    }));
}

#[test]
fn empty_text_saves_and_clean_quit_has_no_effects() {
    let dir = Dir::new();
    let writer = Writer::new();
    let path = dir.0.join("empty.md");
    writer
        .submit(Snapshot {
            path: path.clone(),
            revision: 1,
            text: String::new(),
        })
        .unwrap();
    writer.flush().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"");
    let mut state = State::Clean { revision: 1 };
    assert!(save::step(&mut state, Event::Flush).is_empty());
    assert!(save::step(&mut state, Event::Timer(1.)).is_empty());
}
