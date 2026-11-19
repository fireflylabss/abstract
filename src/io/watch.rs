use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

pub struct Change {
    pub path: PathBuf,
    pub text: Option<String>,
}

pub struct Batch {
    pub generation: u64,
    pub changes: Vec<Change>,
}

pub struct Watch {
    pub generation: u64,
    pub replies: Receiver<Result<Batch, String>>,
    stop: Sender<()>,
}

impl Watch {
    pub fn start(root: PathBuf, generation: u64) -> Self {
        let (done, replies) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        std::thread::spawn(move || {
            let canonical = match std::fs::canonicalize(&root) {
                Ok(path) => path,
                Err(error) => {
                    let _ = done.send(Err(error.to_string()));
                    return;
                }
            };
            let (tx, rx) = mpsc::channel();
            let watch = notify::recommended_watcher(move |event| {
                let _ = tx.send(event);
            });
            let mut watch = match watch {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = done.send(Err(error.to_string()));
                    return;
                }
            };
            if let Err(error) = watch.watch(&canonical, RecursiveMode::Recursive) {
                let _ = done.send(Err(error.to_string()));
                return;
            }
            let _ = done.send(Ok(Batch {
                generation,
                changes: vec![],
            }));
            let mut paths = HashSet::new();
            loop {
                if stopped.try_recv().is_ok() {
                    break;
                }
                match rx.recv_timeout(Duration::from_millis(150)) {
                    Ok(Ok(event)) => {
                        if matches!(event.kind, notify::EventKind::Access(_)) {
                            continue;
                        }
                        paths.extend(event.paths.into_iter().filter_map(|path| {
                            let rel = path.strip_prefix(&canonical).ok()?;
                            if rel
                                .components()
                                .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
                            {
                                return None;
                            }
                            Some(root.join(rel))
                        }));
                    }
                    Ok(Err(error)) => {
                        let _ = done.send(Err(error.to_string()));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if !paths.is_empty() {
                            let changes = paths
                                .drain()
                                .map(|path| {
                                    let text = std::fs::read_to_string(&path).ok();
                                    Change { path, text }
                                })
                                .collect();
                            if done
                                .send(Ok(Batch {
                                    generation,
                                    changes,
                                }))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            }
        });
        Self {
            generation,
            replies,
            stop,
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}
