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
    path.ancestors()
        .find(|ancestor| same_file(ancestor, root))
        .and_then(|ancestor| path.strip_prefix(ancestor).ok())
        .map(Path::to_owned)
}
