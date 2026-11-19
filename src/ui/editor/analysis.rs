use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::core::md::{Analysis, Analyzer};

pub const ASYNC_THRESHOLD: usize = 64 * 1024;

type Job = (String, mpsc::Sender<Analysis>);
static WORKER: Mutex<Option<mpsc::Sender<Job>>> = Mutex::new(None);
#[cfg(test)]
static WORKER_STARTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn worker() -> mpsc::Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    #[cfg(test)]
    WORKER_STARTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::thread::spawn(move || {
        let mut analyzer = Analyzer::new();
        for (source, reply) in rx {
            let _ = reply.send(analyzer.analyze(&source));
        }
    });
    tx
}

fn background(source: String) -> Receiver<Analysis> {
    let (tx, reply) = mpsc::channel();
    let mut shared = WORKER.lock().unwrap();
    let worker = shared.get_or_insert_with(worker);
    if let Err(error) = worker.send((source, tx)) {
        *worker = self::worker();
        let _ = worker.send(error.0);
    }
    reply
}

enum State {
    Empty,
    Ready {
        revision: u64,
        analysis: Box<Analysis>,
    },
    Running {
        revision: u64,
        reply: Receiver<Analysis>,
    },
}

pub struct Cache {
    state: State,
    analyzer: Analyzer,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            state: State::Empty,
            analyzer: Analyzer::new(),
        }
    }

    pub fn update(&mut self, source: &str, revision: u64) {
        match &self.state {
            State::Running { .. } => return,
            State::Ready { revision: old, .. } if *old == revision => return,
            _ => {}
        }
        if source.len() < ASYNC_THRESHOLD {
            self.state = State::Ready {
                revision,
                analysis: Box::new(self.analyzer.analyze(source)),
            };
        } else {
            let reply = background(source.to_owned());
            self.state = State::Running { revision, reply };
        }
    }

    pub fn poll(&mut self, source: &str, revision: u64) -> bool {
        let State::Running {
            revision: attempt,
            reply,
        } = &self.state
        else {
            return false;
        };
        match reply.try_recv() {
            Ok(analysis) => {
                self.state = if *attempt == revision {
                    State::Ready {
                        revision,
                        analysis: Box::new(analysis),
                    }
                } else {
                    State::Empty
                };
                self.update(source, revision);
                true
            }
            Err(TryRecvError::Disconnected) => {
                self.state = State::Empty;
                self.update(source, revision);
                true
            }
            Err(TryRecvError::Empty) => false,
        }
    }

    pub fn get(&self, revision: u64) -> Option<&Analysis> {
        match &self.state {
            State::Ready {
                revision: ready,
                analysis,
            } if *ready == revision => Some(analysis),
            _ => None,
        }
    }

    pub fn running(&self) -> bool {
        matches!(self.state, State::Running { .. })
    }
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
