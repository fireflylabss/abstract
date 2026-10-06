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
                // picked — the shared file tracks whichever landed.
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

    /// Build a tab around a fresh `LiveEditor` holding `file` + `text`.
    fn make_tab(
        &mut self,
        file: NoteFile,
        synced: bool,
        text: String,
        restore: Option<(usize, f32)>,
        pending: bool,
        cx: &mut Context<Self>,
    ) -> NoteTab {
        let editor = cx.new(LiveEditor::new);
        let subs = Self::watch_editor(&editor, cx);
        let note_dir = file.path.parent().map(Path::to_path_buf);
        let root = self.dir.clone();
        self.configure_editor(&editor, cx);
        editor.update(cx, |ed, cx| {
            ed.set_dirs(note_dir, root, cx);
            // `set_text` emits no `Changed`, so nothing is re-saved.
            ed.set_text(text, cx);
            if let Some((cursor, scroll)) = restore {
                ed.restore_view(cursor, scroll, cx);
            }
        });
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        NoteTab {
            id,
            file: Arc::new(Mutex::new(file)),
            synced,
            pending,
            save: SaveState::Saved,
            editor,
            _save_task: None,
            _subs: subs,
        }
    }

    /// A blank editor for the empty state (no tabs open). Its subscriptions
    /// live in `_scratch_subs` until a tab takes over.
    pub(crate) fn scratch_editor(&mut self, cx: &mut Context<Self>) -> Entity<LiveEditor> {
        let editor = cx.new(LiveEditor::new);
        self._scratch_subs = Some(Self::watch_editor(&editor, cx));
        self.configure_editor(&editor, cx);
        editor
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
        let tab = self.make_tab(file, synced, text, restore, pending, cx);
        self.tabs.push(tab);
        self.tabs.len() - 1
    }

    /// Retarget tab `ix` at another note in place (the sidebar click model):
    /// the slot stays put, the buffer is swapped for the new file.
    #[allow(clippy::too_many_arguments)]
    fn load_tab(
        &mut self,
        ix: usize,
        file: NoteFile,
        synced: bool,
        text: String,
        restore: Option<(usize, f32)>,
        pending: bool,
        cx: &mut Context<Self>,
    ) {
        if ix >= self.tabs.len() {
            return;
        }
        // Persist whatever the slot was showing before retargeting it.
        self.flush_tab(ix, cx);
        // Keep the slot's stable id so its strip element state survives.
        let id = self.tabs[ix].id;
        let mut tab = self.make_tab(file, synced, text, restore, pending, cx);
        tab.id = id;
        self.tabs[ix] = tab;
    }

    /// Show tab `ix`: swap the mirrors and restore focus.
    pub(crate) fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(ix) else {
            return;
        };
        // Already on this tab — unless the slot was just retargeted at
        // another note (its editor swapped), which still needs the mirrors.
        if self.active == Some(ix) && self.editor.entity_id() == tab.editor.entity_id() {
            tab.editor.update(cx, |ed, cx| ed.focus(window, cx));
            return;
        }
        let (editor, file, synced, save, path, words, title) = {
            let text = tab.editor.read(cx).text();
            (
                tab.editor.clone(),
                tab.file.clone(),
                tab.synced,
                tab.save,
                tab.path(),
                text.split_whitespace().count(),
                title_of(text).to_string(),
            )
        };
        self._scratch_subs = None;
        self.editor = editor;
        self.save = save;
        self.words = words;
        self.current = Some(CurrentNote { file, synced });
        self.active = Some(ix);
        self.open_gen += 1;
        self.editing = None;
        self.target_folder = None;
        self.clear_completion(cx);
        self.expand_to(&path);
        self.presence.set(Some(title), spaces::name_of(&self.dir));
        self.refresh_find(false, None, cx);
        self.refresh_backlinks(cx);
        self.refresh_images(cx);
        self.editor.update(cx, |ed, cx| ed.focus(window, cx));
        self.save_session(cx);
        cx.notify();
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
        self.activate(next, window, cx);
    }

    /// Cmd/Ctrl+1..9: jump to tab `ix`; `last` (`9`) goes to the final one.
    pub(crate) fn goto_tab(
        &mut self,
        ix: usize,
        last: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ix = if last {
            self.tabs.len().saturating_sub(1)
        } else {
            ix
        };
        if ix < self.tabs.len() && Some(ix) != self.active {
            self.activate(ix, window, cx);
        }
    }

    /// Cmd/Ctrl+W: close the front-most tab.
    pub(crate) fn close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.close_tab(ix, window, cx);
        }
    }

    /// Persist pending text, remember the path for `reopen_closed` and hand
    /// focus to a neighbor.
    pub(crate) fn close_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(ix) {
            let path = tab.path();
            if path.exists() {
                self.closed_tabs.retain(|(p, _)| *p != path);
                self.closed_tabs.push((path, ix));
                if self.closed_tabs.len() > 20 {
                    self.closed_tabs.remove(0);
                }
            }
        }
        self.flush_tab(ix, cx);
        self.remove_tab(ix, window, cx);
    }

    /// Cmd/Ctrl+Shift+T: reopen the most recently closed note at the strip
    /// slot it left.
    pub(crate) fn reopen_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some((path, ix)) = self.closed_tabs.pop() {
            if path.exists() {
                self.open(path, None, true, Some(ix), window, cx);
                return;
            }
        }
    }

    /// Drop tab `ix` without touching its file.
    fn remove_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        let t = self.tabs.remove(ix);
        // The strip keeps a fading ghost pill at this slot for MOTION_OUT_MS.
        self.closing_tabs.push(ClosingTab {
            id: t.id,
            ix,
            title: if t.pending {
                title_of(t.editor.read(cx).text())
            } else {
                SharedString::from(stem_of(&t.path()))
            },
            dirty: t.save != SaveState::Saved,
            failed: t.save == SaveState::Failed,
            at: Instant::now(),
        });
        if self.closing_tabs.len() > 8 {
            self.closing_tabs.remove(0);
        }
        let was_active = self.active == Some(ix);
        self.active = self.active.and_then(|a| match a.cmp(&ix) {
            std::cmp::Ordering::Greater => Some(a - 1),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Less => Some(a),
        });
        if was_active {
            if self.tabs.is_empty() {
                // Back to the empty state: a blank editor, nothing open.
                self.current = None;
                self.save = SaveState::Saved;
                self.words = 0;
                self.completion = None;
                self.backlinks.clear();
                self.backlinks_key = None;
                self.open_gen += 1;
                self.editor = self.scratch_editor(cx);
                // Nothing focusable is mounted without a note — park keys
                // on the root so shortcuts keep dispatching.
                self.empty_focus.focus(window, cx);
                self.presence.set(None, spaces::name_of(&self.dir));
            } else {
                let next = ix.min(self.tabs.len() - 1);
                self.activate(next, window, cx);
                return;
            }
        }
        self.save_session(cx);
        cx.notify();
    }

    /// Open `path` reusing the active tab's slot (sidebar click model).
    pub(crate) fn open_path(
        &mut self,
        path: PathBuf,
        restore: Option<(usize, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open(path, restore, false, None, window, cx);
    }

    /// Open `path` in its own tab (Cmd+click, middle-click, menu item).
    pub(crate) fn open_path_tab(
        &mut self,
        path: PathBuf,
        restore: Option<(usize, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open(path, restore, true, None, window, cx);
    }

    /// `at` only applies to a fresh tab: insert at that strip slot instead
    /// of appending (clamped to the end).
    fn open(
        &mut self,
        path: PathBuf,
        restore: Option<(usize, f32)>,
        new_tab: bool,
        at: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ix) = self.tabs.iter().position(|t| t.path() == path) {
            if let Some((cursor, scroll)) = restore {
                self.tabs[ix]
                    .editor
                    .update(cx, |ed, cx| ed.restore_view(cursor, scroll, cx));
            }
            self.activate(ix, window, cx);
            return;
        }
        self.save_session(cx);
        // Reuse flushes the slot being retargeted; a fresh tab needs none.
        if !new_tab {
            self.flush(cx);
        }
        self.editing = None;
        self.target_folder = None;
        self.expand_to(&path);
        // Reuse retargets the active tab — pinned by file identity so a
        // tab close/reorder while reading lands on the right slot anyway.
        let reuse = if new_tab {
            None
        } else {
            self.cur_tab().map(|t| t.file.clone())
        };
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
                let file = NoteFile {
                    path,
                    mtime: read.1,
                    deleted: false,
                };
                let ix = match reuse
                    .and_then(|f| this.tabs.iter().position(|t| Arc::ptr_eq(&t.file, &f)))
                {
                    Some(ix) => {
                        this.load_tab(ix, file, synced, read.0, restore, false, cx);
                        ix
                    }
                    // The tab meant for reuse was closed meanwhile.
                    None => {
                        let ix = this.push_tab(file, synced, read.0, restore, false, cx);
                        match at {
                            Some(at) if at < ix => {
                                let tab = this.tabs.remove(ix);
                                this.tabs.insert(at, tab);
                                if let Some(a) = this.active
                                    && a >= at
                                {
                                    this.active = Some(a + 1);
                                }
                                at
                            }
                            _ => ix,
                        }
                    }
                };
                this.activate(ix, window, cx);
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
        // An untouched pending tab is already a fresh note — just refocus it.
        if let Some(tab) = self.cur_tab()
            && tab.pending
            && tab.editor.read(cx).text().is_empty()
        {
            self.editor.update(cx, |ed, cx| ed.focus(window, cx));
            return;
        }
        self.save_session(cx);
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
        self.activate(ix, window, cx);
    }

    /// Send a path to the trash off-thread; failure only shows a notice.
    /// Folder deletes route here; notes go through `trash_note` for undo.
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

    /// Trash a note, keeping its text + slot in memory so the notice's Undo
    /// can bring it back. `text` of `None` reads the file first (row delete
    /// of a note that isn't open).
    fn trash_note(
        &mut self,
        path: PathBuf,
        tab_ix: Option<usize>,
        view: Option<(usize, f32)>,
        text: Option<String>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_executor()
                .spawn({
                    let p = path.clone();
                    async move {
                        let text = match text {
                            Some(t) => Some(t),
                            None => std::fs::read_to_string(&p).ok(),
                        }?;
                        trash::delete(&p)
                            .map_err(|e| eprintln!("abstract: cannot trash {}: {e}", p.display()))
                            .ok()?;
                        Some(text)
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                match ok {
                    Some(text) => {
                        this.trash_undo = Some(PendingUndo {
                            path,
                            text,
                            tab_ix,
                            view,
                        });
                        this.arm_undo_notice(cx);
                    }
                    None => this.notice = Some(t(Key::TrashFailed).into()),
                }
                this.rescan_tree(cx);
            })
            .ok();
        })
        .detach();
    }

    /// The 5s "moved to trash · Undo" notice on the status chip.
    fn arm_undo_notice(&mut self, cx: &mut Context<Self>) {
        self.trash_undo_gen += 1;
        let seq = self.trash_undo_gen;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            this.update(cx, |this, cx| {
                if this.trash_undo_gen == seq {
                    this.trash_undo = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The notice's Undo: rewrite the note's file and reopen its tab at the
    /// old slot with its cursor back.
    pub(crate) fn undo_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(undo) = self.trash_undo.take() else {
            return;
        };
        self.trash_undo_gen += 1;
        self.status_open = false;
        cx.spawn_in(window, async move |this, cx| {
            let ok = cx
                .background_executor()
                .spawn({
                    let p = undo.path.clone();
                    async move {
                        if let Some(d) = p.parent() {
                            let _ = std::fs::create_dir_all(d);
                        }
                        store::write_atomic(&p, undo.text.as_bytes()).is_ok()
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                if ok {
                    this.rescan_tree(cx);
                    this.open(undo.path, undo.view, true, undo.tab_ix, window, cx);
                } else {
                    this.notice = Some(t(Key::TrashFailed).into());
                }
                cx.notify();
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
        let view = tab.editor.read(cx).view_state();
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let open = self.tab_paths();
        self._io_task = None;
        cx.spawn(async move |this, cx| {
            let gone = cx
                .background_executor()
                .spawn({
                    let text = text.clone();
                    async move {
                        if pending {
                            let _ = write_note(&lock, &file, synced, &text, Some(&space), &open);
                        }
                        let mut f = guard(&file);
                        f.deleted = true;
                        let p = f.path.clone();
                        (p.clone(), p.exists())
                    }
                })
                .await;
            this.update(cx, |this, cx| match gone {
                (p, true) => this.trash_note(p, Some(ix), Some(view), Some(text), cx),
                // Never hit the disk (untouched pending note): still offer
                // Undo — it just writes the buffer back.
                (path, false) => {
                    this.trash_undo = Some(PendingUndo {
                        path,
                        text,
                        tab_ix: Some(ix),
                        view: Some(view),
                    });
                    this.arm_undo_notice(cx);
                    this.rescan_tree(cx);
                }
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
                    self.trash_note(path, None, None, None, cx);
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
