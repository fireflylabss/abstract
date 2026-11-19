use std::collections::BTreeSet;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::spell::{Engine, Langs, cache::Cache};

pub enum Job {
    Check { id: u64, text: String, langs: Langs },
    Suggest { id: u64, word: String, langs: Langs },
    Add(String),
    Ignore(String),
}

#[cfg(test)]
#[path = "spell_tests.rs"]
mod tests;
pub enum Reply {
    Checked(u64, Vec<Range<usize>>),
    Suggested(u64, Vec<String>),
    Changed(Result<(), String>),
}
pub struct Spelling {
    jobs: Sender<Job>,
    pub replies: Receiver<Reply>,
}
impl Spelling {
    pub fn new(path: PathBuf) -> Self {
        let (jobs, requests) = mpsc::channel();
        let (done, replies) = mpsc::channel();
        std::thread::spawn(move || {
            let mut engine = None;
            let mut cache = Cache::default();
            let mut ignored = BTreeSet::new();
            while let Ok(job) = requests.recv() {
                let engine = engine.get_or_insert_with(|| Engine::load_from(&path));
                let reply = match job {
                    Job::Check { id, text, langs } => {
                        let mut bad = cache.check(&text, engine, langs);
                        bad.retain(|r| !ignored.contains(&text[r.clone()].to_lowercase()));
                        Reply::Checked(id, bad)
                    }
                    Job::Suggest { id, word, langs } => {
                        Reply::Suggested(id, engine.suggest(&word, langs, 6))
                    }
                    Job::Add(word) => {
                        engine.add_user_word(&word);
                        Reply::Changed(engine.save_user_to(&path).map_err(|e| e.to_string()))
                    }
                    Job::Ignore(word) => {
                        if ignored.len() < 10_000 {
                            ignored.insert(word.to_lowercase());
                        }
                        Reply::Changed(Ok(()))
                    }
                };
                if done.send(reply).is_err() {
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
