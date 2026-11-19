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

pub fn relative(path: &Path, root: &Path) -> Option<PathBuf> {
    if let Ok(rel) = path.strip_prefix(root) {
        return Some(rel.to_owned());
    }
    let root = entry_path(root)?;
    path.ancestors()
        .find(|ancestor| entry_path(ancestor).as_ref() == Some(&root))
        .and_then(|ancestor| path.strip_prefix(ancestor).ok())
        .map(Path::to_owned)
}

fn entry_path(path: &Path) -> Option<PathBuf> {
    std::fs::symlink_metadata(path).ok()?;
    let parent = path.parent()?.canonicalize().ok()?;
    let name = path.file_name()?;
    let entries: Vec<_> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .collect();
    entries
        .iter()
        .find(|entry| entry.file_name().as_os_str() == name)
        .or_else(|| {
            entries.iter().find(|entry| {
                entry.file_name().to_string_lossy().to_lowercase()
                    == name.to_string_lossy().to_lowercase()
            })
        })
        .map(|entry| entry.path())
}
