use super::*;
use crate::core::{
    crash,
    i18n::tf,
    update::{self, Install, Release},
};
use crate::io::update::{Job, Reply, Updates};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) enum Phase {
    Dormant,
    Checking(u64),
    Quiet,
    Available(Release, Install),
    Preparing(Release, Install),
    Installing(Release, Install),
    Failed(Release, Install, String),
    Waiting(PathBuf),
    Relaunching,
    RestartFailed(PathBuf, String),
}
pub(super) struct Services {
    pub phase: Phase,
    pub worker: Option<Updates>,
    pub crash: Option<crash::Pending>,
    startup_pending: bool,
    live: bool,
    recovery: Option<(PathBuf, u64, u64)>,
}
impl Services {
    pub fn new(live: bool) -> Self {
        Self {
            phase: Phase::Dormant,
            worker: None,
            crash: None,
            startup_pending: false,
            live,
            recovery: None,
        }
    }
    fn send(&mut self, job: Job) -> Result<(), String> {
        self.worker.get_or_insert_with(Updates::new).submit(job)
    }
    pub fn hold(&mut self, path: &std::path::Path, revision: u64, text: &str) {
        if self.live {
            self.recovery = Some((path.to_owned(), revision, crash::unsaved(text)));
        }
    }
    pub fn saved(&mut self, path: &std::path::Path, revision: u64) {
        if self
            .recovery
            .as_ref()
            .is_some_and(|(p, r, _)| p == path && *r == revision)
            && let Some((_, _, id)) = self.recovery.take()
        {
            crash::saved(id);
        }
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl State {
    pub fn services_ready(&mut self, tx: &mut Tx) {
        if !matches!(self.services.phase, Phase::Dormant) {
            return;
        }
        self.services.phase = Phase::Quiet;
        if self.services.live && self.services.worker.is_none() {
            match self.services.send(Job::Startup(crash::dir())) {
                Ok(()) => self.services.startup_pending = true,
                Err(error) => self.error = Some(error),
            }
        }
        let stamp = now();
        if update::due(
            self.settings.updates(),
            self.settings.update_checked(),
            stamp,
        ) {
            match self.services.send(Job::Check(stamp)) {
                Ok(()) => self.services.phase = Phase::Checking(stamp),
                Err(error) => self.error = Some(error),
            }
        }
        tx.wake_after(Duration::from_millis(50));
    }
    pub fn update_restarting(&self) -> bool {
        matches!(self.services.phase, Phase::Waiting(_) | Phase::Relaunching)
    }
    pub fn update_settings_changed(&mut self, tx: &mut Tx) {
        if !self.settings.updates()
            && !matches!(
                self.services.phase,
                Phase::Preparing(..)
                    | Phase::Installing(..)
                    | Phase::Waiting(_)
                    | Phase::Relaunching
            )
        {
            self.services.phase = Phase::Quiet;
        } else if self.settings.updates() && matches!(self.services.phase, Phase::Quiet) {
            self.services.phase = Phase::Dormant;
            self.services_ready(tx);
        }
    }
    pub fn install_update(&mut self, tx: &mut Tx) {
        let (release, install) = match &self.services.phase {
            Phase::Available(r, i) | Phase::Failed(r, i, _) => (r.clone(), i.clone()),
            Phase::RestartFailed(path, _) => {
                let path = path.clone();
                self.save_now(tx);
                self.services.phase = Phase::Waiting(path);
                tx.wake_after(Duration::from_millis(30));
                return;
            }
            _ => return,
        };
        self.save_now(tx);
        self.services.phase = Phase::Preparing(release, install);
        tx.wake_after(Duration::from_millis(30));
    }
    pub fn poll_services(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Some(reply) = self
            .services
            .worker
            .as_ref()
            .and_then(|w| w.replies.try_recv().ok())
        {
            match reply {
                Reply::Crash(pending) => {
                    self.services.startup_pending = false;
                    self.services.crash = pending;
                }
                Reply::Checked(id, result) => {
                    if !matches!(self.services.phase, Phase::Checking(stamp) if stamp == id) {
                        continue;
                    }
                    self.settings.set_update_checked(id);
                    self.save_settings(tx);
                    self.services.phase = match result {
                        Ok((r, i))
                            if self.settings.updates()
                                && update::is_newer(env!("CARGO_PKG_VERSION"), &r.version) =>
                        {
                            Phase::Available(r, i)
                        }
                        Err(error) => {
                            eprintln!("abstract: update check failed: {error}");
                            Phase::Quiet
                        }
                        _ => Phase::Quiet,
                    };
                }
                Reply::Installed(0, result) => {
                    let previous = std::mem::replace(&mut self.services.phase, Phase::Quiet);
                    if let Phase::Installing(r, i) = previous {
                        self.services.phase = match result {
                            Ok(path) => {
                                self.save_now(tx);
                                Phase::Waiting(path)
                            }
                            Err(error) => Phase::Failed(r, i, error),
                        };
                    } else {
                        self.services.phase = previous;
                    }
                }
                Reply::Relaunched(0, result)
                    if matches!(self.services.phase, Phase::Relaunching) =>
                {
                    match result {
                        Ok(()) => tx.quit(),
                        Err(error) => {
                            self.error = Some(error);
                            self.services.phase = Phase::Quiet;
                        }
                    }
                }
                _ => continue,
            }
            changed = true;
        }
        if let Phase::Preparing(release, install) = &self.services.phase {
            let (release, install) = (release.clone(), install.clone());
            if !self.file_ops.conflicts.is_empty()
                || self
                    .documents
                    .values()
                    .any(|d| matches!(d.save, SaveState::Failed { .. }))
            {
                self.services.phase = Phase::Failed(release, install, t(Key::SaveFailed).into());
                changed = true;
            } else {
                if !self.files_busy() {
                    for (path, doc) in &mut self.documents {
                        if matches!(doc.save, SaveState::Pending { .. }) {
                            Self::effects(&self.writer, path, doc, Event::Flush, tx);
                        }
                    }
                }
                if self
                    .documents
                    .values()
                    .all(|d| matches!(d.save, SaveState::Clean { .. }))
                    && self.settings_pending == 0
                    && self.files_pending == 0
                    && !self.files_busy()
                {
                    self.services.phase =
                        match self
                            .services
                            .send(Job::Install(0, release.clone(), install.clone()))
                        {
                            Ok(()) => Phase::Installing(release, install),
                            Err(error) => Phase::Failed(release, install, error),
                        };
                    changed = true;
                }
            }
        }
        if let Phase::Waiting(path) = &self.services.phase {
            let path = path.clone();
            if !self.file_ops.conflicts.is_empty()
                || self
                    .documents
                    .values()
                    .any(|d| matches!(d.save, SaveState::Failed { .. }))
            {
                self.services.phase = Phase::RestartFailed(path, t(Key::SaveFailed).into());
                changed = true;
            } else if self
                .documents
                .values()
                .all(|d| matches!(d.save, SaveState::Clean { .. }))
                && self.settings_pending == 0
                && self.files_pending == 0
                && !self.files_busy()
            {
                match self.services.send(Job::Relaunch(0, path.clone())) {
                    Ok(()) => self.services.phase = Phase::Relaunching,
                    Err(error) => self.services.phase = Phase::RestartFailed(path, error),
                }
                changed = true;
            }
        }
        if self.services.startup_pending
            || matches!(
                self.services.phase,
                Phase::Checking(_)
                    | Phase::Preparing(..)
                    | Phase::Installing(..)
                    | Phase::Waiting(_)
                    | Phase::Relaunching
            )
        {
            tx.wake_after(Duration::from_millis(50));
        }
        changed
    }
}
fn button(id: &'static str, label: impl Into<String>, p: theme::Palette) -> El {
    text(label.into())
        .key(id)
        .role("button")
        .focusable(true)
        .padding(8.)
        .font(TextStyle::new(12., 400.))
        .color(rgb(p.body))
        .hover(rgb(p.hover))
}
pub(super) fn notice(s: &State, p: theme::Palette) -> El {
    let (message, offer, manual) = match &s.services.phase {
        Phase::Available(r, i) => (
            tf(Key::UpdateAvailable, &[("v", &r.version)]),
            i.automatic(),
            !i.automatic(),
        ),
        Phase::Failed(_, i, e) => (tf(Key::UpdateFailed, &[("err", e)]), i.automatic(), false),
        Phase::Preparing(..) | Phase::Installing(..) | Phase::Waiting(_) | Phase::Relaunching => {
            (t(Key::UpdateInstalling).into(), false, false)
        }
        Phase::RestartFailed(_, e) => (tf(Key::UpdateFailed, &[("err", e)]), true, false),
        _ => return col(),
    };
    let mut notice = row()
        .key("update-notice")
        .w_full()
        .padding(8.)
        .gap(8.)
        .items_center()
        .fill(rgb(p.panel))
        .child(
            text(message)
                .font(TextStyle::new(12., 500.))
                .color(rgb(p.body)),
        );
    if manual {
        notice = notice.child(
            text(t(Key::UpdateManual))
                .font(TextStyle::new(11., 400.))
                .color(rgb(p.dim)),
        );
    }
    if offer {
        notice = notice.child(
            button("update-install", t(Key::UpdateRestart), p).on_press_tx(State::install_update),
        );
    }
    if let Phase::Available(r, _) | Phase::Failed(r, _, _) = &s.services.phase {
        let page = r.page();
        notice = notice.child(button("update-notes", t(Key::ReleaseNotes), p).on_press_tx(
            move |s: &mut State, tx| {
                if let Err(error) = tx.open_url(&page) {
                    s.error = Some(error.to_string());
                }
            },
        ));
    }
    if !matches!(
        s.services.phase,
        Phase::Preparing(..) | Phase::Installing(..) | Phase::Waiting(_) | Phase::Relaunching
    ) {
        notice = notice.child(
            button("update-dismiss", t(Key::Later), p)
                .on_press(|s: &mut State| s.services.phase = Phase::Quiet),
        );
    }
    notice
}
pub(super) fn crash_notice(s: &State, p: theme::Palette) -> El {
    let Some(pending) = &s.services.crash else {
        return col();
    };
    let mut notice = col()
        .key("crash-notice")
        .w_full()
        .padding(8.)
        .gap(4.)
        .fill(rgb(p.panel))
        .child(text(t(Key::CrashTitle)).color(rgb(p.fg)))
        .child(
            text(t(Key::CrashBody))
                .font(TextStyle::new(12., 400.))
                .color(rgb(p.body)),
        )
        .child(
            text(pending.path.display().to_string())
                .font(TextStyle::new(11., 400.))
                .color(rgb(p.dim)),
        );
    if pending.recovered {
        notice = notice.child(text(t(Key::CrashRecovered)).color(rgb(p.body)));
    }
    let report = pending.text.clone();
    notice.child(
        row()
            .gap(6.)
            .child(
                button("crash-copy", t(Key::CopyReport), p)
                    .on_press_tx(move |_: &mut State, tx| tx.copy(report.clone())),
            )
            .child(
                button("crash-dismiss", t(Key::Dismiss), p)
                    .on_press(|s: &mut State| s.services.crash = None),
            ),
    )
}
