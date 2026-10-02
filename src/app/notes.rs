use super::*;

impl AbstractApp {
    /// Re-read the whole folder tree off-thread.
    pub(crate) fn rescan_tree(&mut self, cx: &mut Context<Self>) {
        // `dir` is empty until `enter_space` runs on window activation.
        if self.dir.as_os_str().is_empty() {
            return;
        }
        let dir = self.dir.clone();
        cx.spawn(async move |this, cx| {
            let scanned = cx
                .background_executor()
                .spawn(async move { vault::scan(&dir) })
                .await;
            this.update(cx, |this, cx| {
                match scanned {
                    Ok(tree) => this.tree = tree,
                    Err(err) => eprintln!("abstract: cannot read space: {err}"),
                }
                // A pending note materializes on disk under either name: the
                // planned `nota-*` one, or the title-synced one `write_note`
                // picked. Both cases retire the ghost row.
                for tab in this.tabs.iter_mut().filter(|t| t.pending) {
                    if tab.path().exists() {
                        tab.pending = false;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Expand every folder between the space root and `path`'s parent.
    pub(crate) fn expand_to(&mut self, path: &Path) {
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d == self.dir || !d.starts_with(&self.dir) {
                break;
            }
            self.expanded.insert(d.to_path_buf());
            dir = d.parent();
        }
    }

    /// Create a tab holding `file` + `text`; returns its index.
    pub(crate) fn push_tab(
        &mut self,
        file: NoteFile,
        synced: bool,
        text: String,
        restore: Option<(usize, f32)>,
        pending: bool,
        cx: &mut Context<Self>,
    ) -> usize {
        let editor = cx.new(LiveEditor::new);
        let subs = Self::watch_editor(&editor, cx);
        let note_dir = file.path.parent().map(Path::to_path_buf);
        let root = self.dir.clone();
        let raw = self.settings.raw_tables();
        editor.update(cx, |ed, cx| {
            ed.set_raw_tables(raw, cx);
            ed.set_dirs(note_dir, root, cx);
            // `set_text` emits no `Changed`, so nothing is re-saved.
            ed.set_text(text, cx);
            if let Some((cursor, scroll)) = restore {
                ed.restore_view(cursor, scroll, cx);
            }
        });
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(NoteTab {
            id,
            file: Arc::new(Mutex::new(file)),
            synced,
            pending,
            save: SaveState::Saved,
            editor,
            _save_task: None,
            _subs: subs,
        });
        self.tabs.len() - 1
    }

    /// Show tab `ix`: swap the mirrors, restore focus and record the visit.
    pub(crate) fn activate(
        &mut self,
        ix: usize,
        push_history: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get(ix) else {
            return;
        };
        if self.active == Some(ix) {
            tab.editor.update(cx, |ed, cx| ed.focus(window, cx));
            return;
        }
        self.editor = tab.editor.clone();
        self.save = tab.save;
        self.words = tab.editor.read(cx).text().split_whitespace().count();
        self.current = Some(CurrentNote {
            file: tab.file.clone(),
            synced: tab.synced,
        });
        self.active = Some(ix);
        let id = tab.id;
        let path = tab.path();
        self.open_gen += 1;
        self.editing = None;
        self.target_folder = None;
        self.clear_completion(cx);
        self.expand_to(&path);
        self.refresh_find(false, None, cx);
        self.refresh_backlinks(cx);
        self.refresh_images(cx);
        self.editor.update(cx, |ed, cx| ed.focus(window, cx));
        if push_history {
            self.history_push(id);
        }
        self.save_session(cx);
        cx.notify();
    }

    /// Browser semantics: forward entries die on a fresh visit, revisiting
    /// the current entry never duplicates it.
    fn history_push(&mut self, id: u64) {
        self.history.truncate(self.history_ix.saturating_add(1));
        if self.history.last() != Some(&id) {
            self.history.push(id);
        }
        self.history_ix = self.history.len() - 1;
    }

    /// Alt+Left/Right (Cmd+[/] on macOS): walk the visit log.
    pub(crate) fn history_nav(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ix = if forward {
            self.history_ix.saturating_add(1)
        } else if self.history_ix > 0 {
            self.history_ix - 1
        } else {
            return;
        };
        let Some(&id) = self.history.get(ix) else {
            return;
        };
        let Some(t) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.history_ix = ix;
        self.activate(t, false, window, cx);
    }

    /// Ctrl+Tab / Ctrl+Shift+Tab: cycle through the tab strip.
    pub(crate) fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() < 2 {
            return;
        }
        let cur = self.active.unwrap_or(0);
        let next = if forward {
            (cur + 1) % self.tabs.len()
        } else {
            cur.checked_sub(1).unwrap_or(self.tabs.len() - 1)
        };
        self.activate(next, true, window, cx);
    }

    /// Cmd/Ctrl+W: close the front-most tab.
    pub(crate) fn close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.close_tab(ix, window, cx);
        }
    }

    /// Persist pending text, drop the tab and hand focus to a neighbor.
    pub(crate) fn close_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_tab(ix, cx);
        self.remove_tab(ix, window, cx);
    }

    /// Drop tab `ix` without touching its file; its history entries go too.
    fn remove_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        let closed = self.tabs.remove(ix).id;
        self.history.retain(|&id| id != closed);
        let was_active = self.active == Some(ix);
        self.active = self.active.and_then(|a| match a.cmp(&ix) {
            std::cmp::Ordering::Greater => Some(a - 1),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Less => Some(a),
        });
        if was_active {
            if self.tabs.is_empty() {
                self.current = None;
                self.save = SaveState::Saved;
                self.words = 0;
                self.completion = None;
                self.backlinks.clear();
                self.backlinks_key = None;
                self.history_ix = self.history.len().saturating_sub(1);
            } else {
                let next = ix.min(self.tabs.len() - 1);
                self.history_ix = self
                    .history
                    .iter()
                    .rposition(|&h| h == self.tabs[next].id)
                    .unwrap_or_else(|| self.history.len().saturating_sub(1));
                self.activate(next, true, window, cx);
                return;
            }
        } else {
            let active_id = self.active.map(|a| self.tabs[a].id);
            self.history_ix = active_id
                .and_then(|id| self.history.iter().rposition(|&h| h == id))
                .unwrap_or_else(|| self.history.len().saturating_sub(1));
        }
        self.save_session(cx);
        cx.notify();
    }

    pub(crate) fn open_path(
        &mut self,
        path: PathBuf,
        restore: Option<(usize, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ix) = self.tabs.iter().position(|t| t.path() == path) {
            if let Some((cursor, scroll)) = restore {
                self.tabs[ix]
                    .editor
                    .update(cx, |ed, cx| ed.restore_view(cursor, scroll, cx));
            }
            self.activate(ix, true, window, cx);
            return;
        }
        self.save_session(cx);
        self.flush(cx);
        self.editing = None;
        self.target_folder = None;
        self.expand_to(&path);
        self._io_task = Some(cx.spawn_in(window, async move |this, cx| {
            let p = path.clone();
            let read = cx
                .background_executor()
                .spawn(async move {
                    (
                        std::fs::read_to_string(&p).unwrap_or_default(),
                        std::fs::metadata(&p).and_then(|m| m.modified()).ok(),
                    )
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                let synced = vault::synced_stem(&stem_of(&path), &title_of(&read.0));
                let ix = this.push_tab(
                    NoteFile {
                        path,
                        mtime: read.1,
                        deleted: false,
                    },
                    synced,
                    read.0,
                    restore,
                    false,
                    cx,
                );
                this.activate(ix, true, window, cx);
            })
            .ok();
        }));
    }

    /// The folder new notes/folders go into: last clicked folder, else the
    /// open note's parent, else the space root.
    pub(crate) fn target_dir(&self) -> PathBuf {
        self.target_folder
            .clone()
            .or_else(|| {
                self.current
                    .as_ref()
                    .and_then(|c| c.path().parent().map(Path::to_path_buf))
            })
            .unwrap_or_else(|| self.dir.clone())
    }

    /// The file is created on the first keystroke, so untouched notes leave no trace.
    pub(crate) fn new_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_session(cx);
        self.flush(cx);
        self._io_task = None;
        let dir = self.target_dir();
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let path = vault::unique_path(&dir, &format!("nota-{stamp}"), None);
        self.expand_to(&path);
        self.target_folder = None;
        let ix = self.push_tab(
            NoteFile {
                path,
                mtime: None,
                deleted: false,
            },
            true,
            String::new(),
            None,
            true,
            cx,
        );
        self.activate(ix, true, window, cx);
    }

    /// Send a path to the trash off-thread; failure only shows a notice.
    pub(crate) fn trash_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_executor()
                .spawn({
                    let p = path.clone();
                    async move {
                        trash::delete(&p)
                            .map_err(|e| eprintln!("abstract: cannot trash {}: {e}", p.display()))
                            .is_ok()
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if !ok {
                    this.notice = Some(t(Key::TrashFailed).into());
                }
                this.rescan_tree(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn delete_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else {
            return;
        };
        self.delete_tab(ix, window, cx);
    }

    /// Trash the note open in tab `ix`, then close the tab.
    fn delete_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(ix) else {
            return;
        };
        let path = tab.path();
        let pending = tab.save == SaveState::Pending;
        let file = tab.file.clone();
        let synced = tab.synced;
        let text = tab.editor.read(cx).text().to_string();
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let open = self.tab_paths();
        self._io_task = None;
        cx.spawn(async move |this, cx| {
            let gone = cx
                .background_executor()
                .spawn(async move {
                    if pending {
                        let _ = write_note(&lock, &file, synced, &text, Some(&space), &open);
                    }
                    let mut f = guard(&file);
                    f.deleted = true;
                    let p = f.path.clone();
                    p.exists().then_some(p)
                })
                .await;
            this.update(cx, |this, cx| match gone {
                Some(p) => this.trash_path(p, cx),
                None => this.rescan_tree(cx),
            })
            .ok();
        })
        .detach();
        self.remove_tab(ix, window, cx);
        // Nothing left open: land on the next newest note, or a fresh one.
        if self.tabs.is_empty() {
            match vault::newest_note(&self.tree, Some(&path)) {
                Some(next) => self.open_path(next, None, window, cx),
                None => self.new_note(window, cx),
            }
        }
        cx.notify();
    }

    /// Row delete: notes go straight to the trash, folders ask first.
    pub(crate) fn delete_row(
        &mut self,
        path: PathBuf,
        kind: NodeKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match kind {
            NodeKind::Note => {
                if let Some(ix) = self.tabs.iter().position(|t| t.path() == path) {
                    self.delete_tab(ix, window, cx);
                } else {
                    self.trash_path(path, cx);
                }
            }
            NodeKind::Folder => self.delete_folder(path, window, cx),
        }
    }

    pub(crate) fn delete_folder(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let prompt = window.prompt(
            PromptLevel::Warning,
            &tf(Key::TrashFolderPrompt, &[("name", &name)]),
            Some(t(Key::TrashFolderHint)),
            &[t(Key::MoveToTrash), t(Key::Cancel)],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if prompt.await != Ok(0) {
                return;
            }
            this.update_in(cx, |this, _, cx| {
                this.expanded.retain(|p| !p.starts_with(&path));
                if this
                    .target_folder
                    .as_ref()
                    .is_some_and(|t| t.starts_with(&path))
                {
                    this.target_folder = None;
                }
                // Open notes inside it keep their buffers as "removed".
                let mut hit = false;
                for tab in &this.tabs {
                    if tab.path().starts_with(&path) {
                        guard(&tab.file).mtime = None;
                        hit = true;
                    }
                }
                if hit {
                    this.notice = Some(t(Key::FileRemovedOutside).into());
                }
                this.trash_path(path.clone(), cx);
            })
            .ok();
        })
        .detach();
    }
}
