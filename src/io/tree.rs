use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::vault::{self, Node};

pub struct Reply {
    pub generation: u64,
    pub result: Result<Vec<Node>, String>,
}

#[derive(Default)]
pub struct Generation(u64);

impl Generation {
    pub fn advance(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }

    pub fn accepts(&self, reply: &Reply) -> bool {
        reply.generation == self.0
    }
}

pub struct Scans {
    pub generation: Generation,
    tx: Sender<Reply>,
    pub replies: Receiver<Reply>,
}

impl Scans {
    pub fn new() -> Self {
        let (tx, replies) = mpsc::channel();
        Self {
            generation: Generation::default(),
            tx,
            replies,
        }
    }

    pub fn start(&mut self, path: PathBuf) {
        let generation = self.generation.advance();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = vault::scan(&path).map_err(|e| e.to_string());
            let _ = tx.send(Reply { generation, result });
        });
    }
}
