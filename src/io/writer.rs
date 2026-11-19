use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

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
    Stop,
}

pub struct Writer {
    tx: Sender<Command>,
    pub completions: Receiver<Completion>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (done, completions) = mpsc::channel();
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
                    Command::Stop => break,
                }
            }
        });
        Self {
            tx,
            completions,
            thread: Some(thread),
        }
    }

    pub fn submit(&self, snapshot: Snapshot) -> Result<(), String> {
        self.tx
            .send(Command::Write(snapshot))
            .map_err(|e| e.to_string())
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
