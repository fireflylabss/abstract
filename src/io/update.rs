use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::{
    crash,
    update::{self, Install, Release},
};

pub(crate) trait Backend: Send + 'static {
    fn latest(&mut self) -> Result<Release, String>;
    fn detect(&mut self) -> Install;
    fn install(&mut self, release: &Release, install: &Install) -> Result<PathBuf, String>;
    fn relaunch(&mut self, target: &std::path::Path) -> Result<(), String>;
}
struct System;
impl Backend for System {
    fn latest(&mut self) -> Result<Release, String> {
        update::latest()
    }
    fn detect(&mut self) -> Install {
        Install::detect()
    }
    fn install(&mut self, release: &Release, install: &Install) -> Result<PathBuf, String> {
        update::install(release, install)
    }
    fn relaunch(&mut self, target: &std::path::Path) -> Result<(), String> {
        update::relaunch(target).map_err(|e| e.to_string())
    }
}
pub(crate) enum Job {
    Startup(PathBuf),
    Check(u64),
    Install(u64, Release, Install),
    Relaunch(u64, PathBuf),
}
pub(crate) enum Reply {
    Crash(Option<crash::Pending>),
    Checked(u64, Result<(Release, Install), String>),
    Installed(u64, Result<PathBuf, String>),
    Relaunched(u64, Result<(), String>),
}
pub(crate) struct Updates {
    jobs: Sender<Job>,
    pub replies: Receiver<Reply>,
}
impl Updates {
    pub fn new() -> Self {
        Self::with_backend(System)
    }
    pub fn with_backend(mut backend: impl Backend) -> Self {
        let (jobs, requests) = mpsc::channel();
        let (done, replies) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(job) = requests.recv() {
                let reply = match job {
                    Job::Startup(path) => {
                        update::clean_up();
                        Reply::Crash(crash::take_pending(&path))
                    }
                    Job::Check(id) => {
                        Reply::Checked(id, backend.latest().map(|r| (r, backend.detect())))
                    }
                    Job::Install(id, r, i) => Reply::Installed(id, backend.install(&r, &i)),
                    Job::Relaunch(id, path) => Reply::Relaunched(id, backend.relaunch(&path)),
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

#[cfg(test)]
#[path = "update_tests.rs"]
pub(crate) mod tests;
