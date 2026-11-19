use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

pub fn hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hash);
    hash.finish()
}

pub struct Snapshot {
    pub path: PathBuf,
    pub revision: u64,
    pub text: String,
}

pub struct Completion {
    pub path: PathBuf,
    pub revision: u64,
    pub result: Result<(), String>,
    pub external: Option<Option<String>>,
    pub disk_hash: u64,
}

enum Command {
    Write(Snapshot, Option<u64>),
    Accept(PathBuf),
    Forget(PathBuf),
    Barrier(Sender<Result<(), String>>),
    Operate(super::operations::Operation, Vec<PathBuf>),
    Stop,
}

pub struct Writer {
    tx: Sender<Command>,
    pub completions: Receiver<Completion>,
    pub operations: Receiver<Result<super::operations::Outcome, String>>,
    thread: Option<JoinHandle<()>>,
    acknowledgements: Arc<Mutex<HashMap<PathBuf, u64>>>,
}

impl Writer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (done, completions) = mpsc::channel();
        let (operated, operations) = mpsc::channel();
        let acknowledgements = Arc::new(Mutex::new(HashMap::new()));
        let acks = acknowledgements.clone();
        let thread = thread::spawn(move || {
            let mut revisions = HashMap::<PathBuf, u64>::new();
            let mut failures = HashMap::<PathBuf, String>::new();
            let mut baselines = HashMap::<PathBuf, Option<u64>>::new();
            let mut retry_missing = HashSet::new();
            while let Ok(command) = rx.recv() {
                match command {
                    Command::Forget(path) => {
                        retry_missing.remove(&path);
                        failures.remove(&path);
                        baselines.remove(&path);
                        revisions.remove(&path);
                        acks.lock().unwrap().remove(&path);
                    }
                    Command::Accept(path) => {
                        retry_missing.remove(&path);
                        failures.remove(&path);
                        acks.lock().unwrap().remove(&path);
                        let result = path.parent().map(std::fs::create_dir_all).transpose();
                        if let Err(error) = result {
                            failures.insert(path.clone(), error.to_string());
                        }
                        baselines.insert(
                            path.clone(),
                            super::operations::read_regular(&path)
                                .ok()
                                .map(|text| hash(&text)),
                        );
                    }
                    Command::Write(snapshot, expected) => {
                        if revisions
                            .get(&snapshot.path)
                            .is_some_and(|r| *r > snapshot.revision)
                        {
                            continue;
                        }
                        revisions.insert(snapshot.path.clone(), snapshot.revision);
                        let mut external = None;
                        let disk_hash = hash(&snapshot.text);
                        let result = if let Some(expected) = expected {
                            let actual = super::operations::read_regular(&snapshot.path);
                            let expected = baselines
                                .get(&snapshot.path)
                                .copied()
                                .unwrap_or(Some(expected));
                            let allowed = match actual {
                                Ok(text) if text == snapshot.text => Ok(false),
                                Ok(text) if Some(hash(&text)) == expected => Ok(true),
                                Ok(text) => {
                                    external = Some(Some(text));
                                    Err("File changed outside Abstract".into())
                                }
                                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                                    retry_missing.insert(snapshot.path.clone());
                                    Err(error.to_string())
                                }
                                Err(_)
                                    if expected.is_none()
                                        || retry_missing.contains(&snapshot.path) =>
                                {
                                    Ok(true)
                                }
                                Err(_) => {
                                    external = Some(None);
                                    Err("File changed outside Abstract".into())
                                }
                            };
                            allowed.and_then(|write| {
                                if !write {
                                    return Ok(());
                                }
                                super::atomic::write_existing(
                                    &snapshot.path,
                                    snapshot.text.as_bytes(),
                                )
                                .map_err(|e| e.to_string())
                            })
                        } else {
                            super::atomic::write(&snapshot.path, snapshot.text.as_bytes())
                                .map_err(|e| e.to_string())
                        };
                        match &result {
                            Ok(()) => {
                                retry_missing.remove(&snapshot.path);
                                baselines.insert(snapshot.path.clone(), Some(disk_hash));
                                failures.remove(&snapshot.path);
                                acks.lock()
                                    .unwrap()
                                    .insert(snapshot.path.clone(), disk_hash);
                            }
                            Err(e) => {
                                failures.insert(snapshot.path.clone(), e.clone());
                            }
                        }
                        let _ = done.send(Completion {
                            path: snapshot.path,
                            revision: snapshot.revision,
                            result,
                            external,
                            disk_hash,
                        });
                    }
                    Command::Barrier(reply) => {
                        let result = if failures.is_empty() {
                            Ok(())
                        } else {
                            Err(failures
                                .iter()
                                .map(|(p, e)| format!("{}: {e}", p.display()))
                                .collect::<Vec<_>>()
                                .join("\n"))
                        };
                        let _ = reply.send(result);
                    }
                    Command::Operate(operation, paths) => {
                        let result = if failures.is_empty()
                            || matches!(
                                &operation,
                                super::operations::Operation::Read(_)
                                    | super::operations::Operation::Discard(_)
                                    | super::operations::Operation::Recover { .. }
                            ) {
                            let mut paths = paths;
                            paths.extend(revisions.keys().cloned());
                            paths.extend(baselines.keys().cloned());
                            paths.extend(acks.lock().unwrap().keys().cloned());
                            let relocations =
                                if let super::operations::Operation::Rename { from, to } =
                                    &operation
                                {
                                    paths
                                        .iter()
                                        .filter_map(|path| {
                                            super::identity::relative(path, from).map(|rel| {
                                                (
                                                    path.clone(),
                                                    if rel.as_os_str().is_empty() {
                                                        to.clone()
                                                    } else {
                                                        to.join(rel)
                                                    },
                                                )
                                            })
                                        })
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                            let removed = if let super::operations::Operation::Trash(root) =
                                &operation
                            {
                                paths
                                    .into_iter()
                                    .filter(|path| super::identity::relative(path, root).is_some())
                                    .collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            };
                            let mut result = super::operations::execute(operation);
                            if let Ok(super::operations::Outcome::Renamed { paths, .. }) =
                                &mut result
                            {
                                *paths = relocations;
                            }
                            if matches!(&result, Ok(super::operations::Outcome::Trashed(_))) {
                                for path in removed {
                                    revisions.remove(&path);
                                    failures.remove(&path);
                                    baselines.remove(&path);
                                    acks.lock().unwrap().remove(&path);
                                }
                            }
                            result
                        } else {
                            Err("Save failed; retry saving before changing files".into())
                        };
                        if let Ok(super::operations::Outcome::Read(path, text)) = &result {
                            retry_missing.remove(path);
                            failures.remove(path);
                            revisions.remove(path);
                            acks.lock().unwrap().remove(path);
                            baselines.insert(path.clone(), Some(hash(text)));
                        }
                        if let Ok(super::operations::Outcome::Discarded(path)) = &result {
                            retry_missing.remove(path);
                            failures.remove(path);
                            revisions.remove(path);
                            baselines.remove(path);
                            acks.lock().unwrap().remove(path);
                        }
                        if let Ok(super::operations::Outcome::Renamed { from, to, paths }) = &result
                        {
                            acks.lock()
                                .unwrap()
                                .retain(|path, _| !path.starts_with(from) && !path.starts_with(to));
                            baselines
                                .retain(|path, _| !path.starts_with(to) || path.starts_with(from));
                            let old = std::mem::take(&mut revisions);
                            for (path, revision) in old {
                                if path.starts_with(to) && !path.starts_with(from) {
                                    continue;
                                }
                                let path = paths
                                    .iter()
                                    .find(|(old, _)| *old == path)
                                    .map(|(_, new)| new.clone())
                                    .unwrap_or_else(|| {
                                        crate::core::vault::relocated(&path, from, to)
                                    });
                                revisions.insert(path, revision);
                            }
                            for (old, new) in paths {
                                if let Some(baseline) = baselines.remove(old) {
                                    baselines.insert(new.clone(), baseline);
                                }
                                let mut acks = acks.lock().unwrap();
                                acks.remove(old);
                                acks.remove(new);
                            }
                        }
                        let _ = operated.send(result);
                    }
                    Command::Stop => break,
                }
            }
        });
        Self {
            tx,
            completions,
            operations,
            thread: Some(thread),
            acknowledgements,
        }
    }

    pub fn submit(&self, snapshot: Snapshot) -> Result<(), String> {
        self.tx
            .send(Command::Write(snapshot, None))
            .map_err(|e| e.to_string())
    }

    pub fn operate(&self, operation: super::operations::Operation) -> Result<(), String> {
        self.operate_tracked(operation, Vec::new())
    }

    pub fn submit_document(&self, snapshot: Snapshot, expected: u64) -> Result<(), String> {
        self.tx
            .send(Command::Write(snapshot, Some(expected)))
            .map_err(|e| e.to_string())
    }

    pub fn accept_disk(&self, path: PathBuf) -> Result<(), String> {
        self.tx
            .send(Command::Accept(path))
            .map_err(|e| e.to_string())
    }

    pub fn forget(&self, path: PathBuf) -> Result<(), String> {
        self.tx
            .send(Command::Forget(path))
            .map_err(|e| e.to_string())
    }

    pub fn operate_tracked(
        &self,
        operation: super::operations::Operation,
        paths: Vec<PathBuf>,
    ) -> Result<(), String> {
        self.tx
            .send(Command::Operate(operation, paths))
            .map_err(|e| e.to_string())
    }

    pub fn acknowledged(&self, path: &std::path::Path, text: &str) -> bool {
        self.acknowledgements.lock().unwrap().get(path) == Some(&hash(text))
    }

    pub fn forget_ack(&self, path: &std::path::Path) {
        self.acknowledgements.lock().unwrap().remove(path);
    }

    pub fn flush(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Command::Barrier(tx))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
