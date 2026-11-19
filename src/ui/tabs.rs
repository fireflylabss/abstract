use super::*;
use crate::ui::theme::Palette;

impl State {
    pub fn focus_tab(&mut self, tx: &mut Tx) {
        self.active = self.tabs.active().map(std::path::Path::to_owned);
        self.refresh_find(false, tx);
        if let Some(doc) = self.document() {
            tx.focus("editor", false);
            tx.scroll_to_y("editor-scroll", doc.scroll.get());
        }
        self.persist_tabs(tx);
    }

    pub fn cycle_tab(&mut self, forward: bool, tx: &mut Tx) {
        self.tabs.cycle(forward);
        self.focus_tab(tx);
    }

    pub fn select_tab(&mut self, digit: usize, tx: &mut Tx) {
        self.tabs.digit(digit);
        self.focus_tab(tx);
    }

    pub fn close_tab(&mut self, path: &std::path::Path, tx: &mut Tx) {
        if self.files_busy() {
            return;
        }
        if self.file_ops.conflicts.contains_key(path) {
            if self.file_ops.conflicts.get(path) == Some(&None)
                && self
                    .documents
                    .get(path)
                    .is_some_and(|doc| !doc.save.unacknowledged())
            {
                match self.writer.forget(path.to_owned()) {
                    Ok(()) => {
                        self.file_ops.conflicts.remove(path);
                        self.documents.remove(path);
                        self.tabs.close(path);
                        self.focus_tab(tx);
                        self.start_search(tx);
                    }
                    Err(error) => self.error = Some(error),
                }
                return;
            }
            self.error = Some(t(Key::ResolveFileConflict).into());
            return;
        }
        if let Some(doc) = self.documents.get_mut(path) {
            Self::effects(&self.writer, path, doc, Event::Flush, tx);
        }
        self.tabs.close(path);
        self.focus_tab(tx);
    }

    pub(super) fn snapshot_tabs(&mut self) {
        let mut notes: Vec<_> = self
            .session
            .tabs()
            .into_iter()
            .filter(|note| note.space != self.root)
            .collect();
        notes.extend(
            self.tabs
                .paths()
                .iter()
                .filter_map(|path| {
                    let doc = self.documents.get(path)?;
                    Some(SessionNote {
                        space: self.root.clone(),
                        rel: path.strip_prefix(&self.root).ok()?.to_owned(),
                        cursor: doc.edit.focus,
                        scroll: doc.scroll.get(),
                    })
                })
                .collect::<Vec<_>>(),
        );
        self.session.set_notes(&[]);
        self.session.set_tabs(&notes);
        self.session.set_sidebar(self.sidebar_open);
        self.session.set_active_tab(
            self.active
                .as_ref()
                .and_then(|p| p.strip_prefix(&self.root).ok())
                .map(|rel| (self.root.as_path(), rel)),
        );
    }

    pub(super) fn persist_tabs(&mut self, tx: &mut Tx) {
        self.snapshot_tabs();
        let (_, text) = self.session.snapshot();
        match self.writer.submit(Snapshot {
            path: self.session_path.clone(),
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

    pub(super) fn restore_tabs(&mut self, tx: &mut Tx) {
        let mut notes: Vec<_> = self
            .session
            .tabs()
            .into_iter()
            .filter(|note| note.space == self.root && valid_relative(&note.rel))
            .collect();
        if let Some((space, rel)) = self.session.active_tab()
            && space == self.root
            && valid_relative(&rel)
            && !notes.iter().any(|n| n.rel == rel)
        {
            notes.push(SessionNote {
                space,
                rel,
                cursor: 0,
                scroll: 0.,
            });
        }
        self.file_ticket += 1;
        self.files_pending += 1;
        self.files
            .restore(self.file_ticket, self.root.clone(), notes);
        tx.wake_after(Duration::from_millis(16));
    }

    pub(super) fn restored_tabs(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Ok(reply) = self.files.restored.try_recv() {
            self.files_pending = self.files_pending.saturating_sub(1);
            if reply.ticket != self.file_ticket {
                continue;
            }
            let preferred = self
                .session
                .active_tab()
                .map(|(_, rel)| self.root.join(rel));
            for (note, source) in reply.notes {
                let path = self.root.join(note.rel);
                if self.documents.contains_key(&path) {
                    continue;
                }
                self.load_document(path.clone(), source);
                let doc = self.documents.get_mut(&path).unwrap();
                let mut cursor = note.cursor.min(doc.edit.text.len());
                while !doc.edit.text.is_char_boundary(cursor) {
                    cursor -= 1;
                }
                doc.edit.set_caret(cursor);
                doc.scroll.set(if note.scroll.is_finite() {
                    note.scroll.max(0.)
                } else {
                    0.
                });
                self.tabs.insert(path);
            }
            if let Some(path) = preferred.filter(|p| self.tabs.paths().contains(p)) {
                self.tabs.open(path);
            } else {
                self.tabs.select(0);
            }
            if self.tabs.paths().is_empty() {
                let restore_candidate = self.session.active_tab().is_some()
                    || self
                        .session
                        .tabs()
                        .iter()
                        .any(|note| note.space == self.root)
                    || self.session.sidebar_open().is_none();
                if restore_candidate && let Some(path) = vault::newest_note(&self.tree, None) {
                    let fallback = vault::newest_note(&self.tree, Some(&path));
                    self.open_with_fallback(path, fallback, tx);
                }
            } else {
                self.focus_tab(tx);
            }
            changed = true;
        }
        changed
    }
}

fn valid_relative(path: &std::path::Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

pub fn view(state: &State, p: Palette) -> El {
    let mut strip = row()
        .key("tab-strip")
        .role("tablist")
        .label(t(Key::GlassTabs))
        .w_full()
        .h(34.)
        .gap(3.)
        .padding(4.)
        .fill(rgb(p.panel))
        .scroll_x();
    for path in state.tabs.paths() {
        let active = state.active.as_ref() == Some(path);
        let hovered = state.hovered_tab.as_ref() == Some(path);
        let title = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let dirty = state
            .documents
            .get(path)
            .is_some_and(|d| !matches!(d.save, SaveState::Clean { .. }));
        let target = path.clone();
        let hover = path.clone();
        let close = path.clone();
        let pill = row()
            .key(format!("tab:{}", path.display()))
            .role("tab")
            .label(title.clone())
            .focusable(true)
            .focus_selected(active)
            .items_center()
            .gap(6.)
            .padding(5.)
            .fill(if active {
                rgb(p.active)
            } else {
                rgb(p.panel).alpha(0.)
            })
            .hover(rgb(p.hover))
            .on_hover_change(move |s: &mut State, over, _| {
                if over {
                    s.hovered_tab = Some(hover.clone());
                } else if s.hovered_tab.as_ref() == Some(&hover) {
                    s.hovered_tab = None;
                }
            })
            .on_press_tx(move |s: &mut State, tx| s.open(target.clone(), tx))
            .child(
                text(title)
                    .max_w(160.)
                    .truncate()
                    .font(TextStyle::new(12., 500.))
                    .color(rgb(if active { p.fg } else { p.dim })),
            )
            .child(
                text(if dirty { "•" } else { "" })
                    .key(format!("tab-dirty:{}", path.display()))
                    .color(rgb(p.dim)),
            )
            .child(
                text("×")
                    .key(format!("tab-close:{}", path.display()))
                    .role("button")
                    .label(t(Key::CloseTab))
                    .focusable(true)
                    .color(rgb(p.dim))
                    .opacity(if active || hovered { 1. } else { 0. })
                    .on_press_tx(move |s: &mut State, tx| s.close_tab(&close, tx)),
            );
        strip = strip.child(pill);
    }
    strip
}
