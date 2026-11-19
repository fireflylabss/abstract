use super::*;
use crate::core::spell::{Langs, SpellLang};
use crate::io::spell::{Job, Reply, Spelling};
use rune::Span;
use std::ops::Range;

#[derive(Clone, PartialEq, Eq)]
struct Version {
    path: PathBuf,
    revision: u64,
    langs: Langs,
    epoch: u64,
    disk_hash: u64,
}
struct Menu {
    id: u64,
    version: Version,
    range: Range<usize>,
    word: String,
    suggestions: Option<Vec<String>>,
}
pub(super) struct SpellUi {
    worker: Option<Spelling>,
    path: PathBuf,
    next: u64,
    epoch: u64,
    pending: Option<(u64, Version)>,
    ready: Option<(Version, Vec<Range<usize>>)>,
    menu: Option<Menu>,
    actions: usize,
}
impl SpellUi {
    pub fn finish(&mut self) -> Result<(), String> {
        self.worker
            .take()
            .map_or(Ok(()), |mut worker| worker.finish())
    }
    pub fn new(path: PathBuf) -> Self {
        Self {
            worker: None,
            path,
            next: 0,
            epoch: 0,
            pending: None,
            ready: None,
            menu: None,
            actions: 0,
        }
    }
    fn submit(&mut self, job: Job) -> Result<(), String> {
        self.worker
            .get_or_insert_with(|| Spelling::new(self.path.clone()))
            .submit(job)
    }
}
impl State {
    fn spell_version(&self) -> Option<Version> {
        if !self.settings.spellcheck() {
            return None;
        }
        Some(Version {
            path: self.active.clone()?,
            revision: self.document()?.revision,
            langs: SpellLang::parse(self.settings.spell_lang().as_str()).langs(),
            epoch: self.spelling.epoch,
            disk_hash: self.document()?.disk_hash,
        })
    }
    pub fn poll_spelling(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Some(reply) = self
            .spelling
            .worker
            .as_ref()
            .and_then(|w| w.replies.try_recv().ok())
        {
            match reply {
                Reply::Checked(id, ranges) => {
                    if let Some((pending, version)) = self.spelling.pending.take()
                        && pending == id
                        && self.spell_version().as_ref() == Some(&version)
                    {
                        self.spelling.ready = Some((version, ranges));
                        changed = true;
                    }
                }
                Reply::Suggested(id, suggestions) => {
                    if let Some(menu) = &mut self.spelling.menu
                        && menu.id == id
                    {
                        menu.suggestions = Some(suggestions);
                        tx.focus("spell-choice-0", false);
                        changed = true;
                    }
                }
                Reply::Changed(result) => {
                    self.spelling.actions = self.spelling.actions.saturating_sub(1);
                    self.spelling.epoch += 1;
                    if let Err(error) = result {
                        self.error = Some(error);
                    }
                    changed = true;
                }
            }
        }
        let version = self.spell_version();
        if self
            .spelling
            .menu
            .as_ref()
            .is_some_and(|m| Some(&m.version) != version.as_ref())
        {
            self.spelling.menu = None;
            changed = true;
        }
        if self
            .spelling
            .ready
            .as_ref()
            .is_some_and(|(v, _)| Some(v) != version.as_ref())
        {
            self.spelling.ready = None;
            changed = true;
        }
        if self.spelling.pending.is_none()
            && self.spelling.ready.is_none()
            && let Some(version) = version
        {
            self.spelling.next += 1;
            let id = self.spelling.next;
            let job = Job::Check {
                id,
                text: self.document().unwrap().edit.text.clone(),
                langs: version.langs,
            };
            match self.spelling.submit(job) {
                Ok(()) => self.spelling.pending = Some((id, version)),
                Err(error) => self.error = Some(error),
            }
        }
        if self.spelling.pending.is_some()
            || self.spelling.actions > 0
            || self
                .spelling
                .menu
                .as_ref()
                .is_some_and(|m| m.suggestions.is_none())
        {
            tx.wake_after(Duration::from_millis(30));
        }
        changed
    }
    pub fn spell_spans(&self, p: theme::Palette) -> Vec<Span> {
        self.spelling
            .ready
            .as_ref()
            .filter(|(v, _)| self.spell_version().as_ref() == Some(v))
            .map(|(_, ranges)| {
                ranges
                    .iter()
                    .map(|r| Span::new(r.start, r.end).underline(rgb(p.callout[3])))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn spell_suggestions(&mut self, tx: &mut Tx) {
        if self.spelling.menu.is_some() {
            return;
        }
        let Some(version) = self.spell_version() else {
            return;
        };
        let Some(doc) = self.document() else {
            return;
        };
        let Some((v, ranges)) = &self.spelling.ready else {
            return;
        };
        if *v != version {
            return;
        }
        let Some(range) = ranges
            .iter()
            .find(|r| r.start <= doc.edit.focus && doc.edit.focus <= r.end)
            .or_else(|| ranges.iter().find(|r| r.start >= doc.edit.focus))
            .or_else(|| ranges.first())
            .cloned()
        else {
            return;
        };
        let word = doc.edit.text[range.clone()].to_owned();
        self.spelling.next += 1;
        let id = self.spelling.next;
        if let Err(error) = self.spelling.submit(Job::Suggest {
            id,
            word: word.clone(),
            langs: version.langs,
        }) {
            self.error = Some(error);
            return;
        }
        self.file_ops.mode = file_ops::Mode::Idle;
        if let Some(doc) = self.document_mut() {
            doc.edit.set_selection(range.start, range.end);
        }
        self.spelling.menu = Some(Menu {
            id,
            version,
            range,
            word,
            suggestions: None,
        });
        tx.focus("spell-add", false);
        tx.wake_after(Duration::from_millis(30));
    }
    fn spell_choice(&mut self, replacement: Option<String>, add: bool, tx: &mut Tx) {
        let Some(menu) = self.spelling.menu.take() else {
            return;
        };
        if self.spell_version().as_ref() != Some(&menu.version)
            || self
                .document()
                .and_then(|d| d.edit.text.get(menu.range.clone()))
                != Some(menu.word.as_str())
        {
            return;
        }
        if let Some(replacement) = replacement {
            if let Some(doc) = self.document_mut() {
                doc.edit.set_selection(menu.range.start, menu.range.end);
                doc.edit.undo_group(|edit| {
                    edit.insert(&replacement);
                });
            }
            self.edited(tx);
        } else {
            let job = if add {
                Job::Add(menu.word)
            } else {
                Job::Ignore(menu.word)
            };
            match self.spelling.submit(job) {
                Ok(()) => self.spelling.actions += 1,
                Err(error) => self.error = Some(error),
            }
            tx.wake_after(Duration::from_millis(30));
        }
        tx.focus("editor", false);
    }
    pub fn close_spell(&mut self, tx: &mut Tx) -> bool {
        if self.spelling.menu.take().is_some() {
            tx.focus("editor", false);
            true
        } else {
            false
        }
    }
}
fn button(key: impl Into<String>, label: impl Into<String>, p: theme::Palette) -> El {
    text(label.into())
        .key(key.into())
        .role("menuitem")
        .focusable(true)
        .font(TextStyle::new(12., 400.))
        .color(rgb(p.body))
        .padding(7.)
        .hover(rgb(p.hover))
}
pub(super) fn overlay(s: &State, p: theme::Palette) -> Option<El> {
    let menu = s.spelling.menu.as_ref()?;
    if s.spell_version().as_ref() != Some(&menu.version) {
        return None;
    }
    let mut panel = col()
        .key("spell-menu")
        .role("menu")
        .focus_group()
        .w(240.)
        .padding(6.)
        .gap(2.)
        .absolute(300., 130.)
        .fill(rgb(p.panel))
        .border(1., rgb(p.faint))
        .child(
            text(format!("{} — {}", t(Key::Spellcheck), menu.word))
                .font(TextStyle::new(12., 600.))
                .color(rgb(p.fg))
                .padding(7.),
        );
    if let Some(suggestions) = &menu.suggestions {
        if suggestions.is_empty() {
            panel = panel.child(
                text(t(Key::SpellNoSuggestions))
                    .color(rgb(p.dim))
                    .padding(7.),
            );
        }
        for (ix, word) in suggestions.iter().enumerate() {
            let word = word.clone();
            panel = panel.child(
                button(format!("spell-choice-{ix}"), word.clone(), p).on_press_tx(
                    move |s: &mut State, tx| s.spell_choice(Some(word.clone()), false, tx),
                ),
            );
        }
    } else {
        panel = panel.child(text(t(Key::SpellLoading)).color(rgb(p.dim)).padding(7.));
    }
    Some(
        panel
            .child(
                button("spell-add", t(Key::SpellAddWord), p)
                    .on_press_tx(|s: &mut State, tx| s.spell_choice(None, true, tx)),
            )
            .child(
                button("spell-ignore", t(Key::SpellIgnore), p)
                    .on_press_tx(|s: &mut State, tx| s.spell_choice(None, false, tx)),
            )
            .child(
                button("spell-dismiss", t(Key::Dismiss), p).on_press_tx(|s: &mut State, tx| {
                    s.close_spell(tx);
                }),
            ),
    )
}
pub(super) fn menu(p: theme::Palette) -> El {
    button("spell-open", t(Key::SpellSuggestions), p).on_press_tx(State::spell_suggestions)
}
