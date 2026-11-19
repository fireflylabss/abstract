use super::*;
use crate::io::operations::{Backup, Operation, Outcome};
use rune::{Key as RuneKey, KeyEvent, PointerPress, text_input};

pub(super) enum Mode {
    Idle,
    Menu {
        path: PathBuf,
        folder: bool,
        at: [f32; 2],
    },
    Rename {
        path: PathBuf,
        folder: bool,
        creating: bool,
        edit: Box<TextEdit>,
    },
    Move(PathBuf),
    Confirm(PathBuf),
    Busy,
}

pub(super) struct FileOps {
    pub mode: Mode,
    pub undo: Option<Backup>,
    pub conflicts: HashMap<PathBuf, Option<String>>,
    pub watch: Option<crate::io::watch::Watch>,
    pub recovered: HashMap<PathBuf, u64>,
}

impl Default for FileOps {
    fn default() -> Self {
        Self {
            mode: Mode::Idle,
            undo: None,
            conflicts: HashMap::new(),
            watch: None,
            recovered: HashMap::new(),
        }
    }
}

impl State {
    pub fn files_busy(&self) -> bool {
        matches!(self.file_ops.mode, Mode::Busy)
    }

    pub fn file_menu(&mut self, path: PathBuf, folder: bool, at: [f32; 2], tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        self.file_ops.mode = Mode::Menu { path, folder, at };
        tx.focus("file-action-rename", false);
    }

    pub fn rename_file(&mut self, path: PathBuf, folder: bool, creating: bool, tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        self.sidebar_open = true;
        self.focus_mode = false;
        self.expanded.extend(
            path.ancestors()
                .skip(1)
                .take_while(|parent| parent.starts_with(&self.root))
                .map(std::path::Path::to_owned),
        );
        tx.scroll_into_view("file-rename");
        let name = if creating {
            t(Key::NewFolder).to_owned()
        } else if folder {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let mut edit = TextEdit::new(name);
        edit.focused = true;
        edit.set_selection(0, edit.text.len());
        self.file_ops.mode = Mode::Rename {
            path,
            folder,
            creating,
            edit: Box::new(edit),
        };
        tx.focus("file-rename", false);
    }

    pub fn operate(&mut self, operation: Operation, tx: &mut Tx) {
        if self.files_busy()
            || self.files_pending > 0
            || self.navigation.busy
            || !self.file_ops.conflicts.is_empty()
        {
            self.error = Some(t(Key::ResolveFileConflict).into());
            return;
        }
        self.save_now(tx);
        let mut paths: Vec<_> = self
            .documents
            .keys()
            .cloned()
            .chain(self.expanded.iter().cloned())
            .collect();
        if let Some(undo) = &self.file_ops.undo {
            paths.push(undo.path.clone());
        }
        match self.writer.operate_tracked(operation, paths) {
            Ok(()) => {
                self.file_ops.mode = Mode::Busy;
                tx.wake_after(Duration::from_millis(16));
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn finish_operation(&mut self, result: Result<Outcome, String>, tx: &mut Tx) {
        self.completions();
        self.file_ops.mode = Mode::Idle;
        match result {
            Ok(Outcome::Renamed { from, to, paths }) => {
                let relocate = |path: &std::path::Path| {
                    paths
                        .iter()
                        .find(|(old, _)| old == path)
                        .map(|(_, new)| new.clone())
                        .unwrap_or_else(|| vault::relocated(path, &from, &to))
                };
                let old = std::mem::take(&mut self.documents);
                for (path, doc) in old {
                    let path = relocate(&path);
                    self.documents.insert(path, doc);
                }
                self.tabs.relocate(&from, &to);
                for (old, new) in &paths {
                    self.tabs.relocate(old, new);
                }
                if let Some(undo) = &mut self.file_ops.undo {
                    undo.path = relocate(&undo.path);
                }
                self.active = self.tabs.active().map(std::path::Path::to_owned);
                self.expanded = self.expanded.drain().map(|path| relocate(&path)).collect();
                self.focus_tab(tx);
            }
            Ok(Outcome::Trashed(backup)) => {
                let paths: Vec<_> = self
                    .documents
                    .keys()
                    .filter(|p| p.starts_with(&backup.path))
                    .cloned()
                    .collect();
                for path in paths {
                    self.documents.remove(&path);
                    self.tabs.close(&path);
                }
                self.file_ops.undo = Some(backup);
                self.focus_tab(tx);
            }
            Ok(Outcome::Restored(path, directory)) => {
                self.file_ops.undo = None;
                if directory {
                    self.expanded.insert(path);
                } else {
                    self.open(path, tx);
                }
            }
            Ok(Outcome::Created(path)) => {
                self.expanded.insert(path);
            }
            Ok(Outcome::Read(path, text)) => {
                self.file_ops.conflicts.remove(&path);
                self.documents.insert(path, Document::new(text));
                self.focus_tab(tx);
            }
            Ok(Outcome::Discarded(path)) => {
                self.file_ops.conflicts.remove(&path);
                self.documents.remove(&path);
                self.tabs.close(&path);
                self.focus_tab(tx);
            }
            Ok(Outcome::Recovered(_)) => {}
            Err(error) => self.error = Some(error),
        }
        self.scans.start(self.root.clone());
        self.scanning = true;
        self.persist_tabs(tx);
        self.start_search(tx);
    }

    pub fn finish_pending_file_operation(&mut self, result: Result<Outcome, String>) {
        self.finish_operation(result, &mut Tx::default());
    }

    pub fn file_key(&mut self, key: &KeyEvent, tx: &mut Tx) -> bool {
        if matches!(self.file_ops.mode, Mode::Idle)
            && matches!(key.key, RuneKey::Function(2))
            && tx.focused() == Some("editor")
            && let Some(path) = self.active.clone()
        {
            self.rename_file(path, false, false, tx);
            return true;
        }
        if matches!(key.key, RuneKey::Escape)
            && !matches!(self.file_ops.mode, Mode::Idle | Mode::Busy)
        {
            self.file_ops.mode = Mode::Idle;
            tx.focus("editor", false);
            return true;
        }
        if matches!(key.key, RuneKey::Enter)
            && let Mode::Rename {
                path,
                folder,
                creating,
                edit,
            } = &self.file_ops.mode
        {
            let name = vault::stem_for_title(&edit.text);
            let operation = if *creating {
                Operation::Folder(path.join(name))
            } else {
                let to = path.with_file_name(if *folder { name } else { format!("{name}.md") });
                Operation::Rename {
                    from: path.clone(),
                    to,
                }
            };
            self.operate(operation, tx);
            return true;
        }
        false
    }

    pub fn sidebar_file_key(
        &mut self,
        path: &std::path::Path,
        folder: bool,
        key: &KeyEvent,
        tx: &mut Tx,
    ) -> bool {
        if matches!(key.key, RuneKey::Function(2) | RuneKey::Enter)
            && !key.mods.cmd
            && !key.mods.ctrl
            && !key.mods.alt
        {
            self.rename_file(path.to_owned(), folder, false, tx);
        } else if matches!(key.key, RuneKey::Delete | RuneKey::Backspace)
            && (!cfg!(target_os = "macos") || key.mods.cmd)
        {
            self.file_ops.mode = Mode::Confirm(path.to_owned());
            tx.focus("file-trash-confirm", false);
        } else if key.mods.shift && matches!(key.key, RuneKey::Function(10)) {
            self.file_menu(path.to_owned(), folder, [200., 100.], tx);
        } else {
            return false;
        }
        true
    }

    pub fn poll_file_ops(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Ok(reply) = self.writer.operations.try_recv() {
            self.finish_operation(reply, tx);
            changed = true;
        }
        if self.file_ops.watch.is_none() {
            if !self.restore_started {
                tx.wake_after(Duration::from_millis(100));
                return changed;
            }
            self.file_ops.watch = Some(crate::io::watch::Watch::start(self.root.clone(), 1));
        }
        if !self.files_busy() {
            let watch = self.file_ops.watch.as_ref().unwrap();
            watch.track(self.documents.keys().cloned().collect());
            let replies: Vec<_> = watch.replies.try_iter().collect();
            let generation = watch.generation;
            for reply in replies {
                match reply {
                    Ok(batch) if batch.generation == generation => {
                        let mut external = false;
                        for change in batch.changes {
                            if change
                                .text
                                .as_ref()
                                .is_some_and(|text| self.writer.acknowledged(&change.path, text))
                                && self.file_ops.conflicts.get(&change.path) != Some(&None)
                            {
                                continue;
                            }
                            external = true;
                            self.external_change(change);
                        }
                        if external {
                            self.start_search(tx);
                            self.refresh_find(false, tx);
                            self.scans.start(self.root.clone());
                            self.scanning = true;
                            changed = true;
                        }
                    }
                    Err(error) => {
                        self.error = Some(error);
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
        tx.wake_after(Duration::from_millis(100));
        changed
    }

    pub fn external_change(&mut self, change: crate::io::watch::Change) {
        let Some(doc) = self.documents.get_mut(&change.path) else {
            return;
        };
        if let Some(text) = &change.text {
            if self.file_ops.conflicts.get(&change.path) == Some(&None) {
                self.file_ops.conflicts.remove(&change.path);
            }
            if crate::io::writer::hash(text) == doc.disk_hash
                || *text == doc.edit.text
                || self.writer.acknowledged(&change.path, text)
            {
                return;
            }
            if !doc.save.unacknowledged() {
                self.writer.forget_ack(&change.path);
                if let Err(error) = self.writer.forget(change.path.clone()) {
                    self.error = Some(error);
                    return;
                }
                let caret = doc.edit.focus.min(text.len());
                *doc = Document::new(text.clone());
                let mut caret = caret;
                while !text.is_char_boundary(caret) {
                    caret -= 1;
                }
                doc.edit.set_caret(caret);
                return;
            }
        }
        self.writer.forget_ack(&change.path);
        self.file_ops.conflicts.insert(change.path, change.text);
    }

    pub fn reload_conflict(&mut self, tx: &mut Tx) {
        let Some(path) = self.active.clone() else {
            return;
        };
        if self.file_ops.conflicts.contains_key(&path) && !self.files_busy() {
            let operation = if self.file_ops.conflicts.get(&path) == Some(&None) {
                Operation::Discard(path)
            } else {
                Operation::Read(path)
            };
            match self.writer.operate(operation) {
                Ok(()) => {
                    self.file_ops.mode = Mode::Busy;
                    tx.wake_after(Duration::from_millis(16));
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    pub fn keep_local(&mut self, tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|path| self.file_ops.conflicts.remove(path).is_some())
        {
            if let Some(path) = &self.active
                && let Err(error) = self.writer.accept_disk(path.clone())
            {
                self.error = Some(error);
                return;
            }
            self.edited(tx);
            self.save_now(tx);
        }
    }

    pub fn recover_conflicts(&mut self) {
        for (path, doc) in &self.documents {
            if !self.file_ops.conflicts.contains_key(path)
                && !matches!(doc.save, SaveState::Failed { .. })
            {
                continue;
            }
            let hash = crate::io::writer::hash(&doc.edit.text);
            if !doc.save.unacknowledged() || self.file_ops.recovered.get(path) == Some(&hash) {
                continue;
            }
            let result = self
                .writer
                .operate(Operation::Recover {
                    path: path.clone(),
                    text: doc.edit.text.clone(),
                    fallback: self
                        .session_path
                        .parent()
                        .unwrap_or(&self.root)
                        .join("recovery"),
                })
                .and_then(|()| self.writer.operations.recv().map_err(|e| e.to_string()))
                .and_then(|result| result);
            match result {
                Ok(Outcome::Recovered(recovered)) => {
                    self.file_ops.recovered.insert(path.clone(), hash);
                    eprintln!("{}: {}", t(Key::ConflictRecovered), recovered.display())
                }
                Err(error) => eprintln!("{}: {error}", t(Key::SaveFailed)),
                _ => {}
            }
        }
    }
}

fn button(key: &str, label: &'static str, p: theme::Palette) -> El {
    text(label)
        .key(key)
        .role("button")
        .focusable(true)
        .font(TextStyle::new(12., 400.))
        .color(rgb(p.fg))
        .padding(7.)
        .hover(rgb(p.hover))
}

pub(crate) fn rename_view(s: &State, path: &std::path::Path, p: theme::Palette) -> Option<El> {
    let Mode::Rename {
        path: target,
        creating,
        edit,
        ..
    } = &s.file_ops.mode
    else {
        return None;
    };
    if target != path || *creating {
        return None;
    }
    Some(rename_input(edit, p))
}

fn rename_input(edit: &TextEdit, p: theme::Palette) -> El {
    text_input(edit, TextStyle::new(13., 400.))
        .key("file-rename")
        .label(t(Key::Rename))
        .w_full()
        .padding(7.)
        .border(1., rgb(p.faint))
        .color(rgb(p.fg))
        .fill(rgb(p.bg))
        .on_input_key(State::file_key)
        .on_edit(|s: &mut State| match &mut s.file_ops.mode {
            Mode::Rename { edit, .. } => edit,
            _ => unreachable!(),
        })
}

pub(super) fn notice(s: &State, p: theme::Palette) -> El {
    let mut notice = row()
        .key("file-notices")
        .w_full()
        .items_center()
        .gap(8.)
        .fill(rgb(p.panel));
    if s.file_ops.undo.is_some() {
        notice = notice.child(
            button("trash-undo", t(Key::FilesTrashedUndo), p)
                .disabled(s.files_busy())
                .on_press_tx(|s: &mut State, tx| {
                    if let Some(backup) = s.file_ops.undo.clone() {
                        s.operate(Operation::Restore(backup), tx);
                    }
                }),
        );
    }
    if s.active
        .as_ref()
        .is_some_and(|p| s.file_ops.conflicts.contains_key(p))
    {
        notice = notice
            .child(
                text(t(Key::ExternalConflict))
                    .key("external-conflict")
                    .color(rgb(p.fg))
                    .padding(7.),
            )
            .child(
                button(
                    "conflict-reload",
                    t(
                        if s.active
                            .as_ref()
                            .is_some_and(|path| s.file_ops.conflicts.get(path) == Some(&None))
                        {
                            Key::DiscardEdits
                        } else {
                            Key::ReloadDiscard
                        },
                    ),
                    p,
                )
                .on_press_tx(State::reload_conflict),
            )
            .child(
                button(
                    "conflict-keep",
                    t(
                        if s.active
                            .as_ref()
                            .is_some_and(|path| s.file_ops.conflicts.get(path) == Some(&None))
                        {
                            Key::RecreateNote
                        } else {
                            Key::KeepLocal
                        },
                    ),
                    p,
                )
                .on_press_tx(State::keep_local),
            );
    }
    notice
}

pub(super) fn overlay(s: &State, p: theme::Palette) -> Option<El> {
    let panel = col()
        .key("file-menu")
        .w(230.)
        .padding(6.)
        .gap(3.)
        .fill(rgb(p.panel))
        .border(1., rgb(p.faint));
    match &s.file_ops.mode {
        Mode::Menu { path, folder, at } => {
            let rename = path.clone();
            let create = if *folder {
                path.clone()
            } else {
                path.parent().unwrap_or(&s.root).to_owned()
            };
            let move_path = path.clone();
            let trash = path.clone();
            let folder = *folder;
            Some(
                panel
                    .absolute(at[0], at[1])
                    .child(button("file-action-rename", t(Key::Rename), p).on_press_tx(
                        move |s: &mut State, tx| s.rename_file(rename.clone(), folder, false, tx),
                    ))
                    .child(
                        button("file-action-folder", t(Key::NewFolder), p).on_press_tx(
                            move |s: &mut State, tx| s.rename_file(create.clone(), true, true, tx),
                        ),
                    )
                    .child(
                        button("file-action-move", t(Key::MoveFileTo), p).on_press_tx(
                            move |s: &mut State, tx| {
                                s.file_ops.mode = Mode::Move(move_path.clone());
                                tx.focus("move-folder-0", false);
                            },
                        ),
                    )
                    .child(
                        button("file-action-trash", t(Key::MoveToTrash), p).on_press_tx(
                            move |s: &mut State, tx| {
                                s.file_ops.mode = Mode::Confirm(trash.clone());
                                tx.focus("file-trash-confirm", false);
                            },
                        ),
                    ),
            )
        }
        Mode::Rename {
            creating: true,
            edit,
            ..
        } => Some(
            panel
                .absolute(250., 110.)
                .child(text(t(Key::NewFolder)).color(rgb(p.fg)))
                .child(rename_input(edit, p)),
        ),
        Mode::Move(path) => {
            let mut dirs = vec![s.root.clone()];
            fn collect(nodes: &[vault::Node], dirs: &mut Vec<PathBuf>) {
                for node in nodes {
                    if node.is_folder() {
                        dirs.push(node.path.clone());
                        collect(&node.children, dirs);
                    }
                }
            }
            collect(&s.tree, &mut dirs);
            let mut panel = panel.absolute(250., 100.).max_h(420.).scroll(ScrollSpec {
                stick_bottom: false,
            });
            for (i, dir) in dirs
                .into_iter()
                .filter(|d| !d.starts_with(path))
                .enumerate()
            {
                let target = dir.join(path.file_name().unwrap_or_default());
                let from = path.clone();
                let name = if dir == s.root {
                    spaces::name_of(&s.root)
                } else {
                    dir.strip_prefix(&s.root)
                        .unwrap_or(&dir)
                        .display()
                        .to_string()
                };
                panel = panel.child(
                    text(name)
                        .key(format!("move-folder-{i}"))
                        .role("button")
                        .focusable(true)
                        .color(rgb(p.fg))
                        .padding(7.)
                        .hover(rgb(p.hover))
                        .on_press_tx(move |s: &mut State, tx| {
                            s.operate(
                                Operation::Rename {
                                    from: from.clone(),
                                    to: target.clone(),
                                },
                                tx,
                            )
                        }),
                );
            }
            Some(panel)
        }
        Mode::Confirm(path) => {
            let path = path.clone();
            Some(
                panel
                    .absolute(250., 100.)
                    .child(text(t(Key::MoveToTrash)).color(rgb(p.fg)).padding(7.))
                    .child(
                        button("file-trash-confirm", t(Key::MoveToTrash), p).on_press_tx(
                            move |s: &mut State, tx| s.operate(Operation::Trash(path.clone()), tx),
                        ),
                    )
                    .child(
                        button("file-cancel", t(Key::Cancel), p)
                            .on_press_tx(|s: &mut State, _| s.file_ops.mode = Mode::Idle),
                    ),
            )
        }
        _ => None,
    }
}

pub(crate) fn secondary(
    s: &mut State,
    path: PathBuf,
    folder: bool,
    press: PointerPress,
    tx: &mut Tx,
) {
    s.file_menu(path, folder, [press.x, press.y], tx);
}
