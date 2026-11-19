use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

pub struct Reply {
    pub ticket: u64,
    pub created: bool,
    pub result: Result<(PathBuf, String), String>,
}

pub struct Files {
    tx: Sender<Reply>,
    pub replies: Receiver<Reply>,
}

impl Files {
    pub fn new() -> Self {
        let (tx, replies) = mpsc::channel();
        Self { tx, replies }
    }

    pub fn open(&self, ticket: u64, path: PathBuf, fallback: Option<PathBuf>) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let read = |path: PathBuf| fs::read_to_string(&path).map(|text| (path, text));
            let result = read(path)
                .or_else(|error| match fallback {
                    Some(path) => read(path),
                    None => Err(error),
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(Reply {
                ticket,
                created: false,
                result,
            });
        });
    }

    pub fn create(&self, ticket: u64, root: PathBuf, stem: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = (|| {
                fs::create_dir_all(&root)?;
                for i in 1.. {
                    let name = if i == 1 {
                        format!("{stem}.md")
                    } else {
                        format!("{stem} {i}.md")
                    };
                    let path = root.join(name);
                    match OpenOptions::new().write(true).create_new(true).open(&path) {
                        Ok(file) => {
                            file.sync_all()?;
                            return Ok((path, String::new()));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                        Err(e) => return Err(e),
                    }
                }
                unreachable!()
            })()
            .map_err(|e: std::io::Error| e.to_string());
            let _ = tx.send(Reply {
                ticket,
                created: true,
                result,
            });
        });
    }
}
