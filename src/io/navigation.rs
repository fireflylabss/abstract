use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::{links::follow::Target, search, vault};

pub struct Loaded {
    pub path: PathBuf,
    pub source: String,
    pub heading: String,
}

pub struct Reply {
    pub ticket: u64,
    pub result: Result<Loaded, String>,
}

struct Request {
    ticket: u64,
    root: PathBuf,
    current: PathBuf,
    target: Target,
    buffers: HashMap<PathBuf, String>,
}

pub struct Navigation {
    tx: Sender<Request>,
    replies: Receiver<Reply>,
    pub busy: bool,
    ticket: u64,
}

impl Navigation {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let (done, replies) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                let result = load(&request).map_err(|e| e.to_string());
                if done
                    .send(Reply {
                        ticket: request.ticket,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            tx,
            replies,
            busy: false,
            ticket: 0,
        }
    }

    pub fn start(
        &mut self,
        ticket: u64,
        root: PathBuf,
        current: PathBuf,
        target: Target,
        buffers: HashMap<PathBuf, String>,
    ) {
        self.ticket = ticket;
        self.busy = self
            .tx
            .send(Request {
                ticket,
                root,
                current,
                target,
                buffers,
            })
            .is_ok();
    }

    pub fn poll(&mut self) -> Option<Reply> {
        let mut latest = None;
        for reply in self.replies.try_iter() {
            if reply.ticket == self.ticket {
                self.busy = false;
                latest = Some(reply);
            }
        }
        latest
    }
}

fn load(request: &Request) -> io::Result<Loaded> {
    let read = |path: &Path| {
        request
            .buffers
            .get(path)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| std::fs::read_to_string(path))
    };
    let (path, heading, title) = match &request.target {
        Target::Wiki { name, heading } => {
            if name.is_empty() {
                (request.current.clone(), heading, String::new())
            } else {
                let mut files = Vec::new();
                search::collect(&request.root, &mut files);
                for path in request.buffers.keys() {
                    if path.starts_with(&request.root) && !files.iter().any(|(p, _)| p == path) {
                        files.push((path.clone(), std::time::SystemTime::UNIX_EPOCH));
                    }
                }
                let same = |a: &str| {
                    crate::core::find::matches(a, name, false)
                        .first()
                        .is_some_and(|r| r.start == 0 && r.end == a.len())
                };
                let path = files
                    .iter()
                    .find(|(p, _)| p.file_stem().is_some_and(|s| same(&s.to_string_lossy())))
                    .map(|(p, _)| p.clone())
                    .or_else(|| {
                        files.iter().find_map(|(p, _)| {
                            read(p)
                                .ok()
                                .filter(|s| same(&search::file_title(p, s)))
                                .map(|_| p.clone())
                        })
                    })
                    .unwrap_or_else(|| {
                        vault::unique_path(&request.root, &vault::stem_for_title(name), None)
                    });
                (path, heading, name.clone())
            }
        }
        Target::Local { path, heading } => {
            let title = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            (path.clone(), heading, title)
        }
        Target::External(_) => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a note"));
        }
    };
    let path = request
        .buffers
        .keys()
        .find(|open| same_file(open, &path))
        .cloned()
        .unwrap_or(path);
    let source = match read(&path) {
        Ok(source) => source,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
            if !parent
                .canonicalize()?
                .starts_with(request.root.canonicalize()?)
            {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let source = format!("# {title}\n\n");
                    file.write_all(source.as_bytes())?;
                    file.sync_all()?;
                    source
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => read(&path)?,
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    };
    Ok(Loaded {
        path,
        source,
        heading: heading.clone(),
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
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

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
