use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use rune::{App, AppSpec, Cx, El, ScrollSpec, TextEdit, TextStyle, Tx, col, row, text};

use crate::core::{
    i18n::{self, Key, Lang, LangPref, t},
    spaces,
    store::{Session, SessionNote, Settings},
    vault,
};
use crate::io::{
    files::Files,
    save::{self, Effect, Event, State as SaveState},
    tree::Scans,
    writer::{Snapshot, Writer},
};

use super::{
    editor::{self, analysis::Cache},
    keymap, sidebar,
    theme::{self, rgb},
};

const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-Medium.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-Bold.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-Italic.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-SemiBoldItalic.ttf"),
    include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/NotoSansMono-Bold.ttf"),
];

pub struct Document {
    pub edit: TextEdit,
    pub revision: u64,
    pub save: SaveState,
    pub analysis: Cache,
}

impl Document {
    fn new(source: String) -> Self {
        let mut edit = TextEdit::new(source);
        edit.set_caret(edit.text.len());
        edit.focused = true;
        let mut analysis = Cache::new();
        analysis.update(&edit.text, 0);
        Self {
            edit,
            revision: 0,
            save: SaveState::Clean { revision: 0 },
            analysis,
        }
    }
}

pub struct State {
    pub settings: Settings,
    pub root: PathBuf,
    pub tree: Vec<vault::Node>,
    pub expanded: HashSet<PathBuf>,
    pub active: Option<PathBuf>,
    pub error: Option<String>,
    documents: HashMap<PathBuf, Document>,
    writer: Writer,
    scans: Scans,
    scanning: bool,
    files: Files,
    file_ticket: u64,
    files_pending: usize,
    session: Session,
    session_path: PathBuf,
}

impl Default for State {
    fn default() -> Self {
        Self::new(Settings::load())
    }
}

impl State {
    fn new(settings: Settings) -> Self {
        i18n::set(match settings.lang() {
            LangPref::System => i18n::detect(),
            LangPref::En => Lang::En,
            LangPref::PtBr => Lang::PtBr,
        });
        let spaces = spaces::load(
            rune::extra_args()
                .iter()
                .find(|arg| !arg.starts_with('-'))
                .map(PathBuf::from),
        );
        spaces::save(&spaces);
        let root = spaces.current().to_owned();
        let mut scans = Scans::new();
        scans.start(root.clone());
        let session = Session::load();
        let session_path = session.snapshot().0;
        Self {
            settings,
            root,
            tree: vec![],
            expanded: HashSet::new(),
            active: None,
            error: None,
            documents: HashMap::new(),
            writer: Writer::new(),
            scans,
            scanning: true,
            files: Files::new(),
            file_ticket: 0,
            files_pending: 0,
            session,
            session_path,
        }
    }

    pub fn document(&self) -> Option<&Document> {
        self.active.as_ref().and_then(|p| self.documents.get(p))
    }

    pub fn document_mut(&mut self) -> Option<&mut Document> {
        self.active.as_ref().and_then(|p| self.documents.get_mut(p))
    }

    pub fn open(&mut self, path: PathBuf, tx: &mut Tx) {
        self.file_ticket += 1;
        if self.documents.contains_key(&path) {
            self.active = Some(path);
            tx.focus("editor", false);
        } else {
            self.files_pending += 1;
            self.files.open(self.file_ticket, path);
            tx.wake_after(Duration::from_millis(16));
        }
        self.error = None;
    }

    pub fn new_note(&mut self, tx: &mut Tx) {
        self.file_ticket += 1;
        self.files_pending += 1;
        self.files.create(
            self.file_ticket,
            self.root.clone(),
            vault::stem_for_title(t(Key::Untitled)),
        );
        tx.wake_after(Duration::from_millis(16));
    }

    pub fn edited(&mut self, tx: &mut Tx) {
        let Some(doc) = self.document_mut() else {
            return;
        };
        doc.revision += 1;
        let effects = save::step(
            &mut doc.save,
            Event::Edited {
                revision: doc.revision,
                now: tx.clock(),
            },
        );
        doc.analysis.update(&doc.edit.text, doc.revision);
        for effect in effects {
            if let Effect::WakeAt(due) = effect {
                tx.wake_at(due);
            }
        }
        if doc.analysis.running() {
            tx.wake_after(Duration::from_millis(16));
        }
    }

    pub fn save_now(&mut self, tx: &mut Tx) {
        for (path, doc) in &mut self.documents {
            Self::effects(&self.writer, path, doc, Event::Flush, tx);
        }
    }

    fn effects(
        writer: &Writer,
        path: &std::path::Path,
        doc: &mut Document,
        event: Event,
        tx: &mut Tx,
    ) {
        for effect in save::step(&mut doc.save, event) {
            match effect {
                Effect::WakeAt(due) => tx.wake_at(due),
                Effect::Write(revision) => {
                    if let Err(error) = writer.submit(Snapshot {
                        path: path.to_owned(),
                        revision,
                        text: doc.edit.text.clone(),
                    }) {
                        save::step(
                            &mut doc.save,
                            Event::Completed {
                                revision,
                                result: Err(error),
                            },
                        );
                    }
                    tx.wake_after(Duration::from_millis(16));
                }
            }
        }
    }

    fn completions(&mut self) -> bool {
        let mut changed = false;
        for completion in self.writer.completions.try_iter() {
            if let Some(doc) = self.documents.get_mut(&completion.path) {
                save::step(
                    &mut doc.save,
                    Event::Completed {
                        revision: completion.revision,
                        result: completion.result,
                    },
                );
                changed = true;
            }
        }
        changed
    }

    fn poll(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = self.completions();
        while let Ok(reply) = self.scans.replies.try_recv() {
            if !self.scans.generation.accepts(&reply) {
                continue;
            }
            self.scanning = false;
            match reply.result {
                Ok(tree) => {
                    self.tree = tree;
                    if self.active.is_none() && self.files_pending == 0 {
                        let restored = self
                            .session
                            .active_tab()
                            .filter(|(space, _)| space == &self.root)
                            .map(|(_, rel)| self.root.join(rel));
                        if let Some(path) =
                            restored.or_else(|| vault::newest_note(&self.tree, None))
                        {
                            self.open(path, tx);
                        }
                    }
                }
                Err(error) => self.error = Some(error),
            }
            changed = true;
        }
        while let Ok(reply) = self.files.replies.try_recv() {
            self.files_pending = self.files_pending.saturating_sub(1);
            if reply.created {
                self.scans.start(self.root.clone());
                self.scanning = true;
            }
            if reply.ticket != self.file_ticket {
                continue;
            }
            match reply.result {
                Ok((path, source)) => {
                    self.documents
                        .entry(path.clone())
                        .or_insert_with(|| Document::new(source));
                    self.active = Some(path);
                    tx.focus("editor", false);
                }
                Err(error) => self.error = Some(error),
            }
            changed = true;
        }
        let mut working = self.scanning || self.files_pending > 0;
        for (path, doc) in &mut self.documents {
            changed |= doc.analysis.poll(&doc.edit.text, doc.revision);
            let was_pending = matches!(doc.save, SaveState::Pending { .. });
            Self::effects(&self.writer, path, doc, Event::Timer(now), tx);
            changed |= was_pending && matches!(doc.save, SaveState::Saving { .. });
            working |= doc.analysis.running() || matches!(doc.save, SaveState::Saving { .. });
        }
        if working {
            tx.wake_after(Duration::from_millis(16));
        }
        changed
    }

    fn shutdown(&mut self) {
        for (path, doc) in &mut self.documents {
            for effect in save::step(&mut doc.save, Event::Flush) {
                if let Effect::Write(revision) = effect
                    && let Err(error) = self.writer.submit(Snapshot {
                        path: path.clone(),
                        revision,
                        text: doc.edit.text.clone(),
                    })
                {
                    eprintln!(
                        "abstract: {}: {}: {error}",
                        t(Key::SaveFailed),
                        path.display()
                    );
                }
            }
        }
        let notes: Vec<_> = self
            .documents
            .iter()
            .filter_map(|(path, doc)| {
                Some(SessionNote {
                    space: self.root.clone(),
                    rel: path.strip_prefix(&self.root).ok()?.to_owned(),
                    cursor: doc.edit.focus,
                    scroll: 0.,
                })
            })
            .collect();
        self.session.set_tabs(&notes);
        self.session.set_active_tab(
            self.active
                .as_ref()
                .and_then(|p| p.strip_prefix(&self.root).ok())
                .map(|rel| (self.root.as_path(), rel)),
        );
        let (_, text) = self.session.snapshot();
        if let Err(error) = self
            .writer
            .submit(Snapshot {
                path: self.session_path.clone(),
                revision: 0,
                text,
            })
            .and_then(|_| self.writer.flush())
        {
            self.error = Some(format!("{}: {error}", t(Key::SaveFailed)));
            eprintln!("abstract: {}", self.error.as_deref().unwrap_or_default());
        }
        self.completions();
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn view(state: &State, cx: &mut Cx<State>) -> El {
    cx.zoom(state.settings.zoom());
    let (palette, dark) = theme::resolve(&state.settings, cx.env().system_dark);
    let title = state
        .active
        .as_ref()
        .map(|p| {
            p.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| spaces::name_of(&state.root));
    let mut header = row()
        .w_full()
        .items_center()
        .justify_between()
        .padding(14.)
        .child(
            text(title)
                .font(TextStyle::new(13., 500.))
                .color(rgb(palette.fg))
                .window_drag(),
        );
    if let Some(doc) = state.document() {
        let status = match &doc.save {
            SaveState::Clean { .. } => String::new(),
            SaveState::Failed { error, .. } => format!("{}: {error}", t(Key::SaveFailed)),
            _ => t(Key::Saving).to_string(),
        };
        header = header.child(
            text(status)
                .key("save-status")
                .font(TextStyle::new(11., 400.))
                .color(rgb(palette.dim)),
        );
    }
    let mut pane = col().grow(1.).min_w(0.).h_full().child(header);
    if let Some(error) = &state.error {
        pane = pane.child(
            text(format!("{}: {error}", t(Key::SaveFailed)))
                .key("error")
                .color(rgb(palette.callout[3]))
                .padding(12.),
        );
    }
    pane = pane.child(
        col()
            .key("editor-scroll")
            .w_full()
            .grow(1.)
            .min_h(0.)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .items_center()
            .padding(28.)
            .child(
                col()
                    .w_full()
                    .max_w(state.settings.text_width().max_col())
                    .child(editor::view(state, palette)),
            ),
    );
    col()
        .w_full()
        .h_full()
        .fill(rgb(palette.bg))
        .child(row().w_full().h(30.).fill(rgb(palette.panel)).window_drag())
        .child(
            row()
                .w_full()
                .grow(1.)
                .min_h(0.)
                .child(sidebar::view(state, palette, dark))
                .child(pane),
        )
}

pub fn run() {
    rune::set_app_id("abstract");
    let settings = Settings::load();
    let family: &'static str = Box::leak(settings.font().to_owned().into_boxed_str());
    let spec = AppSpec {
        fonts: FONTS,
        families: (family, "Noto Sans Mono"),
        poll: Some(State::poll),
        on_key: Some(keymap::handle),
        ..AppSpec::new("abstract", (1100., 760.), State::default, view)
    };
    App::new(spec)
        .init(move || State::new(settings))
        .on_quit(|s, _| s.shutdown())
        .run();
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
