use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::{html, pdf, vault};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Html,
    Pdf,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
        }
    }
}

pub struct Snapshot {
    pub path: PathBuf,
    pub text: Option<String>,
    pub root: PathBuf,
    pub tree: Vec<vault::Node>,
}

pub struct Job {
    pub source: Snapshot,
    pub destination: PathBuf,
    pub format: Format,
}

pub struct Exports {
    jobs: Sender<Job>,
    pub replies: Receiver<Result<PathBuf, String>>,
}

impl Exports {
    pub fn new() -> Self {
        let (jobs, requests) = mpsc::channel::<Job>();
        let (completed, replies) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(job) = requests.recv() {
                let result = render(job).map_err(|e| e.to_string());
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

fn render(job: Job) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let Job {
        source,
        destination,
        format,
    } = job;
    if destination == source.path || crate::io::identity::same_file(&destination, &source.path) {
        return Err("Export destination is the source note".into());
    }
    let text = match source.text {
        Some(text) => text,
        None => {
            if !source.path.metadata()?.is_file() {
                return Err("Not a regular note".into());
            }
            std::fs::read_to_string(&source.path)?
        }
    };
    let title = text
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or_else(|| {
            source
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Note")
        });
    let ctx = html::Context {
        note_dir: source.path.parent(),
        root: Some(&source.root),
        tree: Some(&source.tree),
    };
    let bytes = match format {
        Format::Html => html::export(&text, title, &ctx).into_bytes(),
        Format::Pdf => pdf::export(&text, title, &ctx)?,
    };
    super::atomic::write(&destination, &bytes)?;
    Ok(destination)
}
