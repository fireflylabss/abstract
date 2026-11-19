use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
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
