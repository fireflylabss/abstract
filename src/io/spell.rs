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
    jobs: Option<Sender<Job>>,
    pub replies: Receiver<Reply>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Spelling {
    pub fn new(path: PathBuf) -> Self {
        let (jobs, requests) = mpsc::channel();
        let (done, replies) = mpsc::channel();
        let thread = std::thread::spawn(move || {
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
        Self {
            jobs: Some(jobs),
            replies,
            thread: Some(thread),
        }
    }
    pub fn submit(&self, job: Job) -> Result<(), String> {
        self.jobs
            .as_ref()
            .ok_or("Spellchecking stopped")?
            .send(job)
            .map_err(|e| e.to_string())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "Spellchecking worker failed".to_string())?;
        }
        for reply in self.replies.try_iter() {
            if let Reply::Changed(Err(error)) = reply {
                return Err(error);
            }
        }
        Ok(())
    }
}

impl Drop for Spelling {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("abstract: {error}");
        }
    }
}
