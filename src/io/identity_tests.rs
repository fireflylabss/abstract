use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "abstract-identity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn directory_entries_are_listed_at_most_once_per_operation() {
    let dir = Dir::new();
    let root = dir.0.join("Source.md");
    fs::write(&root, "source").unwrap();
    let mut paths = Vec::new();
    for n in 0..10 {
        let other = dir.0.join(format!("Other{n}.md"));
        fs::write(&other, "other").unwrap();
        paths.push(other);
    }
    let mut entries = Entries::default();
    DIRECTORY_READS.with(|reads| reads.borrow_mut().clear());
    for path in &paths {
        assert!(entries.relative(path, &root).is_none());
    }
    DIRECTORY_READS.with(|reads| {
        let reads = reads.borrow();
        assert!(!reads.is_empty());
        assert!(reads.values().all(|count| *count == 1), "{reads:?}");
    });
    assert!(Entries::default().relative(&paths[0], &root).is_none());
    DIRECTORY_READS.with(|reads| assert_eq!(reads.borrow()[&dir.0.canonicalize().unwrap()], 2));
}
