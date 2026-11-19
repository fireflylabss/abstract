use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub enum Operation {
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Trash(PathBuf),
    Restore(Backup),
    Folder(PathBuf),
    Read(PathBuf),
    Discard(PathBuf),
    Recover {
        path: PathBuf,
        text: String,
        fallback: PathBuf,
    },
}

pub enum Outcome {
    Renamed {
        from: PathBuf,
        to: PathBuf,
        paths: Vec<(PathBuf, PathBuf)>,
    },
    Trashed(Backup),
    Restored(PathBuf, bool),
    Created(PathBuf),
    Read(PathBuf, String),
    Discarded(PathBuf),
    Recovered(PathBuf),
}

#[derive(Clone)]
pub struct Backup {
    pub path: PathBuf,
    directory: bool,
    entries: Vec<(PathBuf, Option<Vec<u8>>)>,
}

fn snapshot(path: &Path) -> io::Result<Backup> {
    fn visit(
        path: &Path,
        rel: &Path,
        entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>,
        bytes: &mut u64,
    ) -> io::Result<()> {
        let meta = fs::symlink_metadata(path)?;
        if (!meta.is_file() && !meta.is_dir())
            || entries.len() >= 10_000
            || *bytes + meta.len() > 128 * 1024 * 1024
        {
            return Err(io::Error::other(
                "Trash undo snapshot exceeds limits or contains a symbolic link",
            ));
        }
        if meta.is_dir() {
            entries.push((rel.to_owned(), None));
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                visit(&entry.path(), &rel.join(entry.file_name()), entries, bytes)?;
            }
        } else {
            let data = fs::read(path)?;
            *bytes += data.len() as u64;
            entries.push((rel.to_owned(), Some(data)));
        }
        Ok(())
    }
    let mut entries = Vec::new();
    visit(path, Path::new(""), &mut entries, &mut 0)?;
    Ok(Backup {
        path: path.to_owned(),
        directory: fs::metadata(path)?.is_dir(),
        entries,
    })
}

fn restore(backup: Backup) -> io::Result<PathBuf> {
    let parent = backup
        .path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent"))?;
    let stem = backup
        .path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let mut end = stem.len().min(200);
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    let stem = &stem[..end];
    let folder_name = backup
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let mut end = folder_name.len().min(220);
    while !folder_name.is_char_boundary(end) {
        end -= 1;
    }
    let mut ext = backup
        .path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut end_ext = ext.len().min(32);
    while !ext.is_char_boundary(end_ext) {
        end_ext -= 1;
    }
    ext.truncate(end_ext);
    for n in 1.. {
        let path = if n == 1 {
            backup.path.clone()
        } else if backup.directory {
            parent.join(format!("{} {n}", &folder_name[..end]))
        } else {
            parent.join(format!("{stem} {n}{ext}"))
        };
        let reserve = if backup.directory {
            fs::create_dir(&path).map(|()| None)
        } else {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map(Some)
        };
        let file = match reserve {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
            Ok(file) => file,
        };
        let result = (|| -> io::Result<()> {
            if let Some(mut file) = file {
                if let Some((_, Some(content))) = backup.entries.first() {
                    file.write_all(content)?;
                }
                file.sync_all()?;
                return Ok(());
            }
            for (rel, content) in &backup.entries {
                let target = path.join(rel);
                if let Some(content) = content {
                    let mut options = OpenOptions::new();
                    options.write(true);
                    options.create_new(true);
                    let mut file = options.open(if rel.as_os_str().is_empty() {
                        &path
                    } else {
                        &target
                    })?;
                    file.write_all(content)?;
                    file.sync_all()?;
                } else if !rel.as_os_str().is_empty() {
                    fs::create_dir(&target)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            if backup.directory {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
            return Err(error);
        }
        return Ok(path);
    }
    unreachable!()
}

pub fn execute(operation: Operation) -> Result<Outcome, String> {
    let run = || -> io::Result<Outcome> {
        match operation {
            Operation::Rename { from, to } => {
                if from != to {
                    let occupied = match fs::symlink_metadata(&to) {
                        Ok(meta) => {
                            meta.file_type().is_symlink()
                                || fs::canonicalize(&from)? != fs::canonicalize(&to)?
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                        Err(error) => return Err(error),
                    };
                    if occupied {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "Destination exists",
                        ));
                    }
                    if to.starts_with(&from) {
                        return Err(io::Error::other("Cannot move a folder into itself"));
                    }
                    fs::rename(&from, &to)?;
                }
                Ok(Outcome::Renamed {
                    from,
                    to,
                    paths: Vec::new(),
                })
            }
            Operation::Trash(path) => {
                let backup = snapshot(&path)?;
                trash::delete(&path).map_err(io::Error::other)?;
                Ok(Outcome::Trashed(backup))
            }
            Operation::Restore(backup) => {
                let directory = backup.directory;
                restore(backup).map(|path| Outcome::Restored(path, directory))
            }
            Operation::Folder(path) => {
                fs::create_dir(&path)?;
                Ok(Outcome::Created(path))
            }
            Operation::Read(path) => {
                let text = read_regular(&path)?;
                Ok(Outcome::Read(path, text))
            }
            Operation::Discard(path) => Ok(Outcome::Discarded(path)),
            Operation::Recover {
                path,
                text,
                fallback,
            } => {
                let stem = crate::core::vault::stem_for_title(
                    &path.file_stem().unwrap_or_default().to_string_lossy(),
                );
                let backup = Backup {
                    path: path.with_file_name(format!("{stem} (conflict).md")),
                    directory: false,
                    entries: vec![(PathBuf::new(), Some(text.into_bytes()))],
                };
                restore(backup.clone())
                    .or_else(|_| {
                        fs::create_dir_all(&fallback)?;
                        restore(Backup {
                            path: fallback.join(backup.path.file_name().unwrap()),
                            ..backup
                        })
                    })
                    .map(Outcome::Recovered)
            }
        }
    };
    run().map_err(|e| e.to_string())
}

pub fn read_regular(path: &Path) -> io::Result<String> {
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(io::Error::other("Not a regular file"));
    }
    fs::read_to_string(path)
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;
