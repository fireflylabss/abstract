use std::path::{Path, PathBuf};

pub fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize())
        && a == b
    {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (a.metadata(), b.metadata()) {
            return (a.dev(), a.ino()) == (b.dev(), b.ino());
        }
    }
    false
}

#[derive(Default)]
pub struct Entries {
    directories: std::collections::HashMap<PathBuf, Option<Vec<PathBuf>>>,
}

impl Entries {
    pub fn relative(&mut self, path: &Path, root: &Path) -> Option<PathBuf> {
        if let Ok(rel) = path.strip_prefix(root) {
            return Some(rel.to_owned());
        }
        let root = self.entry_path(root)?;
        path.ancestors()
            .find(|ancestor| self.entry_path(ancestor).as_ref() == Some(&root))
            .and_then(|ancestor| path.strip_prefix(ancestor).ok())
            .map(Path::to_owned)
    }

    fn entry_path(&mut self, path: &Path) -> Option<PathBuf> {
        std::fs::symlink_metadata(path).ok()?;
        let parent = path.parent()?.canonicalize().ok()?;
        let name = path.file_name()?;
        let entries = self
            .directories
            .entry(parent.clone())
            .or_insert_with(|| read_entries(&parent))
            .as_ref()?;
        entries
            .iter()
            .find(|entry| entry.file_name() == Some(name))
            .or_else(|| {
                entries.iter().find(|entry| {
                    entry
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        == name.to_string_lossy().to_lowercase()
                })
            })
            .cloned()
    }
}

#[cfg(test)]
pub fn relative(path: &Path, root: &Path) -> Option<PathBuf> {
    Entries::default().relative(path, root)
}

fn read_entries(parent: &Path) -> Option<Vec<PathBuf>> {
    #[cfg(test)]
    DIRECTORY_READS.with(|reads| *reads.borrow_mut().entry(parent.to_owned()).or_default() += 1);
    Some(
        std::fs::read_dir(parent)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect(),
    )
}

#[cfg(test)]
thread_local! {
    static DIRECTORY_READS: std::cell::RefCell<std::collections::HashMap<PathBuf, usize>> = std::cell::RefCell::default();
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
