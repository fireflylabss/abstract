use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use rune::{App, AppSpec, Cx, El, ScrollSpec, TextEdit, TextStyle, Tx, col, row, text};

use crate::core::{
    i18n::{self, Key, Lang, LangPref, t},
    spaces,
    store::{Session, SessionNote, Settings},
    tabs::Tabs,
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
    pub disk_hash: u64,
    pub save: SaveState,
    pub analysis: Cache,
    pub scroll: Rc<Cell<f32>>,
}

impl Document {
    fn new(source: String) -> Self {
        let disk_hash = crate::io::writer::hash(&source);
        let mut edit = TextEdit::new(source);
        edit.set_caret(edit.text.len());
        edit.focused = true;
        let mut analysis = Cache::new();
        analysis.update(&edit.text, 0);
        Self {
            disk_hash,
            edit,
            revision: 0,
            save: SaveState::Clean { revision: 0 },
            analysis,
            scroll: Rc::new(Cell::new(0.)),
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
    pub sidebar_open: bool,
    pub focus_mode: bool,
    documents: HashMap<PathBuf, Document>,
    writer: Writer,
    scans: Scans,
    scanning: bool,
    files: Files,
    file_ticket: u64,
    files_pending: usize,
    session: Session,
    session_path: PathBuf,
    settings_path: PathBuf,
    settings_pending: usize,
    tabs: Tabs,
    hovered_tab: Option<PathBuf>,
    restore_started: bool,
    edit_revision: u64,
    find: Option<find::FindBar>,
    search: Option<search::SearchBar>,
    navigation: crate::io::navigation::Navigation,
    jump: Option<(u64, PathBuf, std::ops::Range<usize>)>,
    customization: settings::Customization,
    file_ops: file_ops::FileOps,
    export: export::ExportUi,
    attachments: attachments::Attachments,
    spelling: spelling::SpellUi,
    services: services::Services,
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
        let settings_path = settings.snapshot().0;
        let sidebar_open = session.sidebar_open().unwrap_or(true);
        Self {
            spelling: spelling::SpellUi::new(settings.snapshot().0.with_file_name("userdict")),
            services: services::Services::new(true),
            file_ops: file_ops::FileOps::default(),
            customization: settings::Customization::new(&settings),
            settings,
            root,
            tree: vec![],
            expanded: HashSet::new(),
            active: None,
            error: None,
            sidebar_open,
            focus_mode: false,
            documents: HashMap::new(),
            writer: Writer::new(),
            scans,
            scanning: true,
            files: Files::new(),
            file_ticket: 0,
            files_pending: 0,
            session,
            session_path,
            settings_path,
            settings_pending: 0,
            tabs: Tabs::default(),
            hovered_tab: None,
            restore_started: false,
            edit_revision: 0,
            find: None,
            search: None,
            navigation: crate::io::navigation::Navigation::new(),
            jump: None,
            export: export::ExportUi::new(),
            attachments: attachments::Attachments::new(),
        }
    }

    pub fn document(&self) -> Option<&Document> {
        self.active.as_ref().and_then(|p| self.documents.get(p))
    }

    pub fn document_mut(&mut self) -> Option<&mut Document> {
        self.active.as_ref().and_then(|p| self.documents.get_mut(p))
    }

    pub(super) fn load_document(&mut self, path: PathBuf, source: String) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.documents.entry(path.clone())
        {
            if let Err(error) = self.writer.forget(path) {
                self.error = Some(error);
            }
            entry.insert(Document::new(source));
        }
    }

    pub fn open(&mut self, path: PathBuf, tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        self.customization.open = false;
        self.open_with_fallback(path, None, tx);
    }

    fn open_with_fallback(&mut self, path: PathBuf, fallback: Option<PathBuf>, tx: &mut Tx) {
        self.file_ticket += 1;
        if self.documents.contains_key(&path) {
            self.tabs.open(path);
            self.focus_tab(tx);
        } else {
            self.files_pending += 1;
            self.files.open(self.file_ticket, path, fallback);
            tx.wake_after(Duration::from_millis(16));
        }
        self.error = None;
    }

    pub fn new_note(&mut self, tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        self.customization.open = false;
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
        let Some(path) = self.active.clone() else {
            return;
        };
        self.edited_path(&path, tx);
    }

    pub fn edited_path(&mut self, path: &std::path::Path, tx: &mut Tx) {
        let delay = self.settings.number("autosave_delay") as f64;
        self.edit_revision += 1;
        let revision = self.edit_revision;
        let Some(doc) = self.documents.get_mut(path) else {
            return;
        };
        doc.revision = revision;
        let effects = save::step_after(
            &mut doc.save,
            Event::Edited {
                revision: doc.revision,
                now: tx.clock(),
            },
            delay,
        );
        doc.analysis.update(&doc.edit.text, doc.revision);
        self.services.hold(path, doc.revision, &doc.edit.text);
        for effect in effects {
            if let Effect::WakeAt(due) = effect {
                tx.wake_at(due);
            }
        }
        if doc.analysis.running() {
            tx.wake_after(Duration::from_millis(16));
        }
        self.refresh_find(false, tx);
        if self.search.is_some() {
            self.start_search(tx);
        }
    }

    pub fn save_now(&mut self, tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        for (path, doc) in &mut self.documents {
            if self.file_ops.conflicts.contains_key(path) {
                continue;
            }
            Self::effects(&self.writer, path, doc, Event::Flush, tx);
        }
    }

    pub fn save_settings(&mut self, tx: &mut Tx) {
        let (_, text) = self.settings.snapshot();
        match self.writer.submit(Snapshot {
            path: self.settings_path.clone(),
            revision: 0,
            text,
        }) {
            Ok(()) => {
                self.settings_pending += 1;
                tx.wake_after(Duration::from_millis(16));
            }
            Err(error) => self.error = Some(error),
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
                    if let Err(error) = writer.submit_document(
                        Snapshot {
                            path: path.to_owned(),
                            revision,
                            text: doc.edit.text.clone(),
                        },
                        doc.disk_hash,
                    ) {
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
            if completion.path == self.settings_path || completion.path == self.session_path {
                self.settings_pending = self.settings_pending.saturating_sub(1);
                if let Err(error) = completion.result {
                    self.error = Some(error);
                }
                changed = true;
                continue;
            }
            if let Some(doc) = self.documents.get_mut(&completion.path) {
                if completion.result.is_ok() {
                    doc.disk_hash = completion.disk_hash;
                    self.services.saved(&completion.path, completion.revision);
                }
                if let Some(external) = completion.external {
                    self.file_ops
                        .conflicts
                        .insert(completion.path.clone(), external);
                }
                let failed = completion.result.as_ref().err().cloned();
                save::step(
                    &mut doc.save,
                    Event::Completed {
                        revision: completion.revision,
                        result: completion.result,
                    },
                );
                if let Some(error) = failed {
                    self.error = Some(error);
                    self.tabs.insert(completion.path.clone());
                    if self.active.is_none() {
                        self.tabs.open(completion.path.clone());
                        self.active = Some(completion.path);
                    }
                }
                changed = true;
            }
        }
        self.documents.retain(|path, doc| {
            self.tabs.paths().contains(path) || !matches!(doc.save, SaveState::Clean { .. })
        });
        changed
    }

    fn poll(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = self.completions();
        changed |= self.poll_export(tx);
        changed |= self.poll_attachments(tx);
        changed |= self.poll_file_ops(tx);
        changed |= self.poll_caret(now, tx);
        changed |= self.poll_services(tx);
        changed |= self.poll_spelling(tx);
        while let Ok(reply) = self.scans.replies.try_recv() {
            if !self.scans.generation.accepts(&reply) {
                continue;
            }
            self.scanning = false;
            match reply.result {
                Ok(tree) => {
                    self.tree = tree;
                    if !self.restore_started {
                        self.restore_started = true;
                        if self.active.is_none() && self.files_pending == 0 {
                            self.restore_tabs(tx);
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
            match reply.result {
                Ok((path, source)) => {
                    self.load_document(path.clone(), source);
                    self.tabs.insert(path.clone());
                    if reply.ticket == self.file_ticket {
                        self.tabs.open(path);
                        self.focus_tab(tx);
                    } else {
                        self.persist_tabs(tx);
                    }
                }
                Err(error) if reply.ticket == self.file_ticket => self.error = Some(error),
                Err(_) => {}
            }
            changed = true;
        }
        changed |= self.restored_tabs(tx);
        changed |= self.poll_navigation(tx);
        changed |= self.poll_search();
        self.apply_jump(tx);
        let mut working = self.scanning
            || self.files_busy()
            || self.files_pending > 0
            || self.settings_pending > 0
            || self.navigation.busy
            || self.searching();
        for (path, doc) in &mut self.documents {
            changed |= doc.analysis.poll(&doc.edit.text, doc.revision);
            let was_pending = matches!(doc.save, SaveState::Pending { .. });
            if !matches!(self.file_ops.mode, file_ops::Mode::Busy)
                && !self.file_ops.conflicts.contains_key(path)
            {
                Self::effects(&self.writer, path, doc, Event::Timer(now), tx);
            }
            changed |= was_pending && matches!(doc.save, SaveState::Saving { .. });
            working |= doc.analysis.running() || matches!(doc.save, SaveState::Saving { .. });
        }
        if working {
            tx.wake_after(Duration::from_millis(16));
        }
        changed
    }

    fn shutdown(&mut self) {
        if let Err(error) = self.spelling.finish() {
            eprintln!("abstract: {error}");
            self.error = Some(error);
        }
        self.flush_export();
        self.flush_attachments();
        if self.files_busy() {
            match self.writer.operations.recv() {
                Ok(result) => self.finish_pending_file_operation(result),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        self.completions();
        self.session.set_sidebar(self.sidebar_open);
        for (path, doc) in &mut self.documents {
            if self.file_ops.conflicts.contains_key(path) {
                continue;
            }
            for effect in save::step(&mut doc.save, Event::Flush) {
                if let Effect::Write(revision) = effect
                    && let Err(error) = self.writer.submit_document(
                        Snapshot {
                            path: path.clone(),
                            revision,
                            text: doc.edit.text.clone(),
                        },
                        doc.disk_hash,
                    )
                {
                    eprintln!(
                        "abstract: {}: {}: {error}",
                        t(Key::SaveFailed),
                        path.display()
                    );
                }
            }
        }
        self.snapshot_tabs();
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
        self.recover_conflicts();
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn view(state: &State, cx: &mut Cx<State>) -> El {
    cx.zoom(state.settings.zoom());
    let (mut palette, _) = theme::resolve(&state.settings, cx.env().system_dark);
    let accent = state.settings.preference("accent");
    let accent = if accent == "system" {
        let c = cx.accent();
        let channel = |v| (rune::color::srgb_encode(v) * 255.).round() as u32;
        Some(channel(c.r) << 16 | channel(c.g) << 8 | channel(c.b))
    } else {
        u32::from_str_radix(&accent, 16).ok()
    };
    if let Some(accent) = accent {
        palette.callout[0] = accent;
        palette.caret = accent;
        palette.selection = accent << 8 | 0x40;
    }
    if state.settings.glass() && state.settings.glass_text_contrast() {
        palette.body = palette.fg;
    }
    let mut header = row()
        .w_full()
        .h(30.)
        .items_center()
        .justify_end()
        .window_drag();
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
    let mut pane = col().grow(1.).min_w(0.).h_full();
    pane = pane.child(file_ops::notice(state, palette));
    pane = pane.child(export::notice(state, palette));
    pane = pane.child(attachments::notice(state, palette));
    pane = pane.child(services::notice(state, palette));
    pane = pane.child(services::crash_notice(state, palette));
    if !state.focus_mode && (state.tabs.paths().len() > 1 || state.settings.enabled("single_tab")) {
        pane = pane.child(settings::surface(
            tabs::view(state, palette),
            &state.settings,
            crate::core::prefs::Surface::Tabs,
            palette.panel,
        ));
    }
    if state.find.is_some() {
        pane = pane.child(find::view(state, palette));
    }
    if state.search.is_some() {
        pane = pane.child(search::view(state, palette));
    }
    if let Some(error) = &state.error {
        pane = pane.child(
            text(format!("{}: {error}", t(Key::SaveFailed)))
                .key("error")
                .color(rgb(palette.callout[3]))
                .padding(12.),
        );
    }
    let scroll = state.document().map(|doc| doc.scroll.clone());
    if state.settings_open() {
        pane = pane.child(settings::view(state, palette));
    } else {
        pane = pane.child(settings::surface(
            col()
                .key("editor-scroll")
                .w_full()
                .grow(1.)
                .min_h(0.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .on_scroll_offset(move |pos| {
                    if let Some(scroll) = &scroll {
                        scroll.set(pos.offset[1]);
                    }
                })
                .items_center()
                .padding(28.)
                .child(
                    col()
                        .w_full()
                        .max_w(state.settings.text_width().max_col())
                        .child(editor::view(state, palette)),
                ),
            &state.settings,
            crate::core::prefs::Surface::Editor,
            palette.bg,
        ));
    }
    let mut body = row().w_full().grow(1.).min_h(0.);
    if state.sidebar_open && !state.focus_mode {
        body = body
            .child(sidebar::view(state, palette))
            .child(settings::resize_handle(palette));
    }
    let mut root = col()
        .w_full()
        .h_full()
        .child(settings::surface(
            header.child(
                text(t(Key::Settings))
                    .key("settings-open")
                    .role("button")
                    .focusable(true)
                    .font(TextStyle::new(12., 400.))
                    .color(rgb(palette.dim))
                    .padding(6.)
                    .on_press_tx(State::toggle_settings),
            ),
            &state.settings,
            crate::core::prefs::Surface::Toolbar,
            palette.panel,
        ))
        .child(body.child(pane));
    if let Some(caret) = settings::caret(state, cx, palette) {
        root = root.child(caret);
    }
    if let Some(overlay) = file_ops::overlay(state, palette) {
        root = root.child(overlay);
    }
    if let Some(overlay) = spelling::overlay(state, palette) {
        root = root.child(overlay);
    }
    root
}

pub fn run() {
    crate::core::crash::install();
    rune::set_app_id("abstract");
    let settings = Settings::load();
    let family: &'static str = Box::leak(settings.preference("ui_font").into_boxed_str());
    let spec = AppSpec {
        fonts: FONTS,
        families: (family, "Noto Sans Mono"),
        scenes: snapshot_scenes::ALL,
        poll: Some(State::poll),
        on_key: Some(keymap::handle),
        ..AppSpec::new("abstract", (1100., 760.), State::default, view)
    };
    let app = App::new(spec);
    let handle = app.handle();
    app.window_options(rune::WindowOptions {
        transparent: cfg!(target_os = "macos"),
        ..Default::default()
    })
    .init(move || State::new(settings))
    .on_quit(|s, _| s.shutdown())
    .on_ready(move |_| handle.update(|s, tx| s.services_ready(tx)))
    .run();
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

#[path = "tabs.rs"]
mod tabs;

#[path = "find.rs"]
mod find;
#[path = "navigation.rs"]
mod navigation;
#[path = "search.rs"]
mod search;

#[path = "settings.rs"]
pub(super) mod settings;

#[path = "file_ops.rs"]
pub(super) mod file_ops;

#[path = "export.rs"]
pub(super) mod export;

#[path = "attachments.rs"]
pub(super) mod attachments;

#[path = "spelling.rs"]
pub(super) mod spelling;

#[path = "services.rs"]
pub(super) mod services;

#[path = "snapshot_scenes.rs"]
mod snapshot_scenes;
