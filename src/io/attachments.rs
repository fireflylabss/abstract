use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::attach::{self, Incoming};

pub struct Job {
    pub path: PathBuf,
    pub root: PathBuf,
    pub items: Vec<Incoming>,
}

pub struct Imports {
    jobs: Sender<Job>,
    pub replies: Receiver<Result<Vec<String>, String>>,
}

impl Imports {
    pub fn new() -> Self {
        let (jobs, requests) = mpsc::channel::<Job>();
        let (completed, replies) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(job) = requests.recv() {
                let result = (|| {
                    if !job.path.metadata()?.is_file() {
                        return Err(std::io::Error::other("Not a regular note"));
                    }
                    let dir = job
                        .path
                        .parent()
                        .ok_or_else(|| std::io::Error::other("Missing note directory"))?;
                    attach::import(job.items, dir, &job.root, &attach::stamp())
                })()
                .map_err(|e| e.to_string());
                if completed.send(result).is_err() {
                    break;
                }
            }
        });
        Self { jobs, replies }
    }

    pub fn submit(&self, job: Job) -> Result<(), String> {
        self.jobs.send(job).map_err(|e| e.to_string())
    }
}
