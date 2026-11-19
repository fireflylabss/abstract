use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn rollback_keeps_foreign_entries_in_a_partly_restored_directory() {
    let dir = Dir::new();
    let root = dir.0.join("restored");
    fs::create_dir(&root).unwrap();
    let owned = root.join("owned.md");
    let foreign = root.join("foreign.md");
    fs::write(&owned, "ours").unwrap();
    fs::write(&foreign, "theirs").unwrap();
    rollback(&[(root.clone(), true), (owned.clone(), false)]);
    assert!(!owned.exists());
    assert_eq!(fs::read_to_string(foreign).unwrap(), "theirs");
}

#[test]
fn unchanged_rename_preserves_the_latest_save_baseline() {
    use crate::io::writer::{Snapshot, Writer, hash};
    let dir = Dir::new();
    let path = dir.0.join("Note.md");
    fs::write(&path, "original").unwrap();
    let writer = Writer::new();
    writer
        .submit_document(
            Snapshot {
                path: path.clone(),
                revision: 1,
                text: "saved".into(),
            },
            hash("original"),
        )
        .unwrap();
    writer.flush().unwrap();
    writer
        .operate(Operation::Rename {
            from: path.clone(),
            to: path.clone(),
        })
        .unwrap();
    assert!(writer.operations.recv().unwrap().is_ok());
    writer
        .submit_document(
            Snapshot {
                path: path.clone(),
                revision: 2,
                text: "later".into(),
            },
            hash("original"),
        )
        .unwrap();
    assert!(writer.flush().is_ok());
    assert_eq!(fs::read_to_string(path).unwrap(), "later");
}

#[test]
fn reload_clears_a_discarded_writer_failure() {
    use crate::io::writer::{Snapshot, Writer};
    let dir = Dir::new();
    let path = dir.0.join("Note.md");
    fs::create_dir(&path).unwrap();
    let writer = Writer::new();
    writer
        .submit(Snapshot {
            path: path.clone(),
            revision: 1,
            text: "failed".into(),
        })
        .unwrap();
    assert!(writer.flush().is_err());
    fs::remove_dir(&path).unwrap();
    fs::write(&path, "external").unwrap();
    writer.operate(Operation::Read(path)).unwrap();
    assert!(matches!(
        writer.operations.recv().unwrap(),
        Ok(Outcome::Read(..))
    ));
    writer
        .operate(Operation::Folder(dir.0.join("New")))
        .unwrap();
    assert!(matches!(
        writer.operations.recv().unwrap(),
        Ok(Outcome::Created(_))
    ));
}

#[test]
fn rename_forgets_the_old_write_acknowledgement() {
    use crate::io::writer::{Snapshot, Writer};
    let dir = Dir::new();
    let path = dir.0.join("Note.md");
    let writer = Writer::new();
    writer
        .submit(Snapshot {
            path: path.clone(),
            revision: 1,
            text: "old".into(),
        })
        .unwrap();
    writer.flush().unwrap();
    writer
        .operate(Operation::Rename {
            from: path.clone(),
            to: dir.0.join("Moved.md"),
        })
        .unwrap();
    assert!(writer.operations.recv().unwrap().is_ok());
    assert!(!writer.acknowledged(&path, "old"));
}

#[test]
fn trash_forgets_hashes_baselines_and_revisions_before_path_reuse() {
    use crate::io::writer::{Snapshot, Writer, hash};
    let dir = Dir::new();
    let path = dir.0.join("Note.md");
    let writer = Writer::new();
    writer
        .submit(Snapshot {
            path: path.clone(),
            revision: 7,
            text: "old".into(),
        })
        .unwrap();
    writer.flush().unwrap();
    writer.operate(Operation::Trash(path.clone())).unwrap();
    assert!(writer.operations.recv().unwrap().is_ok());
    fs::write(&path, "new").unwrap();
    assert!(!writer.acknowledged(&path, "old"));
    writer
        .submit_document(
            Snapshot {
                path: path.clone(),
                revision: 1,
                text: "new edited".into(),
            },
            hash("new"),
        )
        .unwrap();
    writer.flush().unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "new edited");
}

#[test]
fn failed_restore_removes_its_partial_destination() {
    let dir = Dir::new();
    let path = dir.0.join("restore");
    let backup = Backup {
        path: path.clone(),
        directory: true,
        entries: vec![
            (PathBuf::new(), None),
            ("first.md".into(), Some(b"written".to_vec())),
            ("missing/second.md".into(), Some(b"fails".to_vec())),
        ],
    };
    assert!(restore(backup).is_err());
    assert!(!path.exists());
}

#[test]
fn recovery_survives_a_missing_parent() {
    let dir = Dir::new();
    let outcome = execute(Operation::Recover {
        path: dir.0.join("gone/Note.md"),
        text: "local".into(),
        fallback: dir.0.join("recovery"),
    })
    .unwrap();
    let Outcome::Recovered(path) = outcome else {
        panic!("Expected recovery");
    };
    assert!(path.starts_with(dir.0.join("recovery")));
    assert_eq!(fs::read_to_string(path).unwrap(), "local");
}

#[test]
fn folder_events_recheck_every_tracked_open_descendant() {
    let dir = Dir::new();
    let folder = dir.0.join("Old");
    fs::create_dir(&folder).unwrap();
    let note = folder.join("Note.md");
    fs::write(&note, "before").unwrap();
    fs::rename(&folder, dir.0.join("Moved")).unwrap();
    let changes =
        crate::io::watch::changes([folder].into_iter().collect(), std::slice::from_ref(&note));
    assert!(
        changes
            .iter()
            .any(|change| change.path == note && change.text.is_none())
    );
}

#[test]
fn existing_document_atomic_write_never_recreates_parents() {
    let dir = Dir::new();
    let parent = dir.0.join("missing");
    assert!(crate::io::atomic::write_existing(&parent.join("Note.md"), b"local").is_err());
    assert!(!parent.exists());
}

#[cfg(unix)]
#[test]
fn rename_refuses_dangling_and_source_pointing_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = Dir::new();
    let from = dir.0.join("source.md");
    fs::write(&from, "source").unwrap();
    for target in [dir.0.join("missing"), from.clone()] {
        let to = dir.0.join("link.md");
        symlink(target, &to).unwrap();
        assert!(
            execute(Operation::Rename {
                from: from.clone(),
                to: to.clone()
            })
            .is_err()
        );
        assert!(fs::symlink_metadata(&to).unwrap().file_type().is_symlink());
        fs::remove_file(to).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn snapshot_rejects_a_fifo_without_opening_it() {
    let dir = Dir::new();
    let pipe = dir.0.join("pipe.md");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(snapshot(&pipe).is_err() && read_regular(&pipe).is_err());
    });
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(1)).ok(),
        Some(true)
    );
}
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "abstract-ops-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn undo_never_overwrites_a_recreated_note_or_folder() {
    let dir = Dir::new();
    let path = dir.0.join("Note.md");
    fs::write(&path, "original").unwrap();
    let backup = snapshot(&path).unwrap();
    fs::write(&path, "replacement").unwrap();
    let restored = restore(backup).unwrap();
    assert_eq!(restored, dir.0.join("Note 2.md"));
    assert_eq!(fs::read_to_string(path).unwrap(), "replacement");
    assert_eq!(fs::read_to_string(restored).unwrap(), "original");
    let folder = dir.0.join("Folder");
    fs::create_dir_all(folder.join("Empty")).unwrap();
    fs::write(folder.join("child.md"), "child").unwrap();
    let backup = snapshot(&folder).unwrap();
    let restored = restore(backup).unwrap();
    assert_eq!(restored, dir.0.join("Folder 2"));
    assert!(restored.join("Empty").is_dir());
    assert_eq!(
        fs::read_to_string(restored.join("child.md")).unwrap(),
        "child"
    );
}

#[test]
fn undo_collision_with_long_unicode_filename_stays_within_filename_limit() {
    let dir = Dir::new();
    let path = dir.0.join(format!("{}.md", "é".repeat(123)));
    fs::write(&path, "original").unwrap();
    let backup = snapshot(&path).unwrap();
    fs::write(&path, "replacement").unwrap();
    let restored = restore(backup).unwrap();
    assert!(restored.file_name().unwrap().len() <= 255);
    assert_eq!(fs::read_to_string(path).unwrap(), "replacement");
    assert_eq!(fs::read_to_string(restored).unwrap(), "original");
}

#[test]
fn ordered_rename_waits_for_writes_and_rebases_revisions() {
    use crate::io::writer::{Snapshot, Writer};
    let dir = Dir::new();
    let from = dir.0.join("old.md");
    let to = dir.0.join("new.md");
    let writer = Writer::new();
    writer
        .submit(Snapshot {
            path: from.clone(),
            revision: 7,
            text: "pending edits".into(),
        })
        .unwrap();
    writer
        .operate(Operation::Rename {
            from: from.clone(),
            to: to.clone(),
        })
        .unwrap();
    assert!(matches!(
        writer.operations.recv().unwrap(),
        Ok(Outcome::Renamed { .. })
    ));
    assert!(!from.exists());
    assert_eq!(fs::read_to_string(&to).unwrap(), "pending edits");
    writer
        .submit(Snapshot {
            path: to.clone(),
            revision: 8,
            text: "later edits".into(),
        })
        .unwrap();
    writer.flush().unwrap();
    assert!(!from.exists());
    assert_eq!(fs::read_to_string(to).unwrap(), "later edits");
}

#[test]
fn operation_errors_do_not_remove_sources_or_replace_destinations() {
    let dir = Dir::new();
    let from = dir.0.join("old.md");
    let to = dir.0.join("new.md");
    fs::write(&from, "old").unwrap();
    fs::write(&to, "new").unwrap();
    assert!(
        execute(Operation::Rename {
            from: from.clone(),
            to: to.clone()
        })
        .is_err()
    );
    assert_eq!(fs::read_to_string(from).unwrap(), "old");
    assert_eq!(fs::read_to_string(to).unwrap(), "new");
    assert!(execute(Operation::Trash(dir.0.join("missing"))).is_err());
}

#[test]
fn watcher_debounces_changes_and_ignores_atomic_temporary_files() {
    use crate::io::watch::Watch;
    use std::time::Duration;
    let dir = Dir::new();
    let watch = Watch::start(dir.0.clone(), 42);
    watch
        .replies
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    fs::write(dir.0.join(".abstract-tmp"), "hidden").unwrap();
    fs::write(dir.0.join("Note.md"), "first").unwrap();
    fs::write(dir.0.join("Note.md"), "latest").unwrap();
    let batch = watch
        .replies
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(batch.generation, 42);
    assert_eq!(batch.changes.len(), 1);
    assert_eq!(batch.changes[0].text.as_deref(), Some("latest"));
}
