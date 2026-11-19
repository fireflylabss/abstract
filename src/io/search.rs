use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::search::{self, Hit};

struct Request {
    generation: u64,
    root: PathBuf,
    query: String,
    buffers: HashMap<PathBuf, String>,
}

pub struct Searches {
    generation: u64,
    tx: Sender<Request>,
    replies: Receiver<(u64, Vec<Hit>)>,
    pub busy: bool,
}

impl Searches {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let (done, replies) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = rx.recv() {
                for newer in rx.try_iter() {
                    request = newer;
                }
                let hits = search::search_with(&request.root, &request.query, &request.buffers);
                if done.send((request.generation, hits)).is_err() {
                    break;
                }
            }
        });
        Self {
            generation: 0,
            tx,
            replies,
            busy: false,
        }
    }

    pub fn start(&mut self, root: PathBuf, query: String, buffers: HashMap<PathBuf, String>) {
        self.generation += 1;
        self.busy = self
            .tx
            .send(Request {
                generation: self.generation,
                root,
                query,
                buffers,
            })
            .is_ok();
    }

    pub fn poll(&mut self) -> Option<Vec<Hit>> {
        let mut latest = None;
        for (generation, hits) in self.replies.try_iter() {
            if generation == self.generation {
                self.busy = false;
                latest = Some(hits);
            }
        }
        latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_replies_cannot_replace_current_results() {
        let (done, replies) = mpsc::channel();
        let (tx, _) = mpsc::channel();
        let mut searches = Searches {
            generation: 2,
            tx,
            replies,
            busy: true,
        };
        done.send((1, vec![])).unwrap();
        assert!(searches.poll().is_none());
        assert!(searches.busy);
        done.send((2, vec![])).unwrap();
        assert!(searches.poll().is_some());
        assert!(!searches.busy);
        assert!(searches.poll().is_none());
    }
}
