use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::core::md::{Analysis, Analyzer};

pub const ASYNC_THRESHOLD: usize = 64 * 1024;

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
            State::Running { .. } if source.len() >= ASYNC_THRESHOLD => return,
            State::Ready { revision: old, .. } if *old == revision => return,
            _ => {}
        }
        if source.len() < ASYNC_THRESHOLD {
            self.state = State::Ready {
                revision,
                analysis: Box::new(self.analyzer.analyze(source)),
            };
        } else {
            let source = source.to_owned();
            let (tx, reply) = mpsc::channel();
            std::thread::spawn(move || {
                let result = Analyzer::new().analyze(&source);
                let _ = tx.send(result);
            });
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
                self.state = State::Ready {
                    revision,
                    analysis: Box::default(),
                };
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
