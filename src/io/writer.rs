use std::collections::HashMap;
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
}

enum Command {
    Write(Snapshot),
    Barrier(Sender<Result<(), String>>),
    Operate(super::operations::Operation),
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
            while let Ok(command) = rx.recv() {
                match command {
                    Command::Write(snapshot) => {
                        if revisions
                            .get(&snapshot.path)
                            .is_some_and(|r| *r > snapshot.revision)
                        {
                            continue;
                        }
                        revisions.insert(snapshot.path.clone(), snapshot.revision);
                        let result = super::atomic::write(&snapshot.path, snapshot.text.as_bytes())
                            .map_err(|e| e.to_string());
                        match &result {
                            Ok(()) => {
                                failures.remove(&snapshot.path);
                                acks.lock()
                                    .unwrap()
                                    .insert(snapshot.path.clone(), hash(&snapshot.text));
                            }
                            Err(e) => {
                                failures.insert(snapshot.path.clone(), e.clone());
                            }
                        }
                        let _ = done.send(Completion {
                            path: snapshot.path,
                            revision: snapshot.revision,
                            result,
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
                    Command::Operate(operation) => {
                        let result = if failures.is_empty()
                            || matches!(
                                &operation,
                                super::operations::Operation::Read(_)
                                    | super::operations::Operation::Recover { .. }
                            ) {
                            super::operations::execute(operation)
                        } else {
                            Err("Save failed; retry saving before changing files".into())
                        };
                        if let Ok(super::operations::Outcome::Renamed { from, to }) = &result {
                            let old = std::mem::take(&mut revisions);
                            for (path, revision) in old {
                                let path = crate::core::vault::relocated(&path, from, to);
                                revisions.insert(path, revision);
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
            .send(Command::Write(snapshot))
            .map_err(|e| e.to_string())
    }

    pub fn operate(&self, operation: super::operations::Operation) -> Result<(), String> {
        self.tx
            .send(Command::Operate(operation))
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
