use super::*;

impl AbstractApp {
    /// Debounced background write of tab `ix`'s note.
    pub(crate) fn schedule_tab_save(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get_mut(ix) else {
            return;
        };
        tab.save = SaveState::Pending;
        let file = tab.file.clone();
        let synced = tab.synced;
        let editor = tab.editor.clone();
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let open = self.tab_paths();
        if self.active == Some(ix) {
            self.save = SaveState::Pending;
        }
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            // Path and text are captured now, not when the timer was set.
            // A tab retargeted meanwhile (sidebar click) no longer owns this
            // `file`, so the write is skipped — the slot's own flush wrote it.
            let Ok(payload) = this.update(cx, |this, cx| {
                this.tabs
                    .iter()
                    .any(|t| Arc::ptr_eq(&t.file, &file))
                    .then(|| editor.read(cx).text().to_string())
            }) else {
                return;
            };
            let Some(text) = payload else { return };
            let done = file.clone();
            let stem = stem_of(&guard(&file).path);
            let result = cx
                .background_executor()
                .spawn(async move { write_note(&lock, &file, synced, &text, Some(&space), &open) })
                .await;
            this.update(cx, |this, cx| this.finish_save(done, stem, result, cx))
                .ok();
        });
        if let Some(tab) = self.tabs.get_mut(ix) {
            tab._save_task = Some(task);
        }
    }

    /// Ctrl+S: write now instead of waiting for the debounce.
    pub(crate) fn save_now(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.active else { return };
        let Some((text, file, synced)) = self.tabs.get_mut(ix).map(|tab| {
            tab._save_task = None;
            (
                tab.editor.read(cx).text().to_string(),
                tab.file.clone(),
                tab.synced,
            )
        }) else {
            return;
        };
        self.tabs[ix].save = SaveState::Pending;
        self.save = SaveState::Pending;
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let open = self.tab_paths();
        cx.spawn(async move |this, cx| {
            let done = file.clone();
            let stem = stem_of(&guard(&file).path);
            let result = cx
                .background_executor()
                .spawn(async move { write_note(&lock, &file, synced, &text, Some(&space), &open) })
                .await;
            this.update(cx, |this, cx| this.finish_save(done, stem, result, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn finish_save(
        &mut self,
        file: Arc<Mutex<NoteFile>>,
        stem_before: String,
        result: std::io::Result<bool>,
        cx: &mut Context<Self>,
    ) {
        let ix = self.tabs.iter().position(|t| Arc::ptr_eq(&t.file, &file));
        let state = match &result {
            Ok(_) => SaveState::Saved,
            Err(_) => SaveState::Failed,
        };
        // `ix` misses when the tab closed while the write was in flight.
        if let Some(ix) = ix {
            self.tabs[ix].save = state;
            if self.active == Some(ix) {
                self.save = state;
            }
        }
        match result {
            Ok(changed) => {
                self.notice = None;
                // A synced save may have renamed the file to its title stem;
                // the other open buffers retarget their `[[old]]` links so a
                // later autosave doesn't write the stale name back.
                if changed {
                    let stem_after = stem_of(&guard(&file).path);
                    if stem_after != stem_before {
                        for (tix, tab) in self.tabs.iter().enumerate() {
                            if Some(tix) != ix {
                                tab.editor.update(cx, |ed, cx| {
                                    ed.retarget_links(&stem_before, &stem_after, cx)
                                });
                            }
                        }
                    }
                    self.rescan_tree(cx);
                }
                self.refresh_backlinks_if_renamed(cx);
            }
            Err(err) => eprintln!("abstract: failed to save note: {err}"),
        }
        cx.notify();
    }

    /// Hand a pending save of tab `ix` to the background.
    pub(crate) fn flush_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some((text, file, synced)) = self.tabs.get_mut(ix).and_then(|tab| {
            (tab.save != SaveState::Saved).then(|| {
                tab._save_task = None;
                tab.save = SaveState::Saved;
                (
                    tab.editor.read(cx).text().to_string(),
                    tab.file.clone(),
                    tab.synced,
                )
            })
        }) else {
            return;
        };
        if self.active == Some(ix) {
            self.save = SaveState::Saved;
        }
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let open = self.tab_paths();
        let done = file.clone();
        let stem = stem_of(&guard(&file).path);
        let write = cx.background_spawn(async move {
            write_note(&lock, &file, synced, &text, Some(&space), &open)
        });
        cx.spawn(async move |this, cx| {
            let result = write.await;
            this.update(cx, |this, cx| this.finish_save(done, stem, result, cx))
                .ok();
        })
        .detach();
    }

    /// Hand the active tab's pending save to the background.
    pub(crate) fn flush(&mut self, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.flush_tab(ix, cx);
        }
    }

    /// Flush every dirty tab (space switches, quit).
    pub(crate) fn flush_all(&mut self, cx: &mut Context<Self>) {
        for ix in 0..self.tabs.len() {
            self.flush_tab(ix, cx);
        }
    }

    /// On quit: write the pending saves inline (the write lock waits for any
    /// in-flight op), then the session — tab entries included.
    pub(crate) fn flush_blocking(&mut self, cx: &mut Context<Self>) {
        let (lock, space, open) = (self.write_lock.clone(), self.dir.clone(), self.tab_paths());
        for tab in &mut self.tabs {
            if tab.save != SaveState::Pending {
                continue;
            }
            tab._save_task = None;
            tab.save = SaveState::Saved;
            let _ = write_note(
                &lock,
                &tab.file,
                tab.synced,
                tab.editor.read(cx).text(),
                Some(&space),
                &open,
            );
        }
        self.save = SaveState::Saved;
        self.record_tabs(cx);
        self.session.set_sidebar(self.sidebar_open);
        if let Some(w) = self.window_state {
            self.session.set_window(&w);
        }
        self.session.set_tabs(&self.session_notes);
        self.session
            .set_notes(&latest_per_space(&self.session_notes));
        let active = self.active_rel();
        self.session
            .set_active_tab(active.as_ref().map(|(s, r)| (s.as_path(), r.as_path())));
        self.session.save();
    }

    /// Active tab's path relative to the space, for `tab_active`.
    fn active_rel(&self) -> Option<(PathBuf, PathBuf)> {
        let tab = self.cur_tab()?;
        let path = tab.path();
        if tab.pending && !path.exists() {
            return None;
        }
        path.strip_prefix(&self.dir)
            .ok()
            .map(|r| (self.dir.clone(), r.to_path_buf()))
    }

    /// Rebuild this space's `session_notes` from the live tabs, active last
    /// so the legacy `note` fallback reopens the right note.
    pub(crate) fn record_tabs(&mut self, cx: &App) {
        if self.dir.as_os_str().is_empty() {
            return;
        }
        let dir = self.dir.clone();
        self.session_notes.retain(|n| n.space != dir);
        let active = self.active.unwrap_or(usize::MAX);
        let mut entries = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let path = tab.path();
            if tab.pending && !path.exists() {
                continue;
            }
            let Ok(rel) = path.strip_prefix(&dir) else {
                continue;
            };
            if rel.as_os_str().is_empty() {
                continue;
            }
            let (cursor, scroll) = tab.editor.read(cx).view_state();
            entries.push((
                i,
                SessionNote {
                    space: dir.clone(),
                    rel: rel.to_path_buf(),
                    cursor,
                    scroll,
                },
            ));
        }
        entries.sort_by_key(|(i, _)| *i == active);
        self.session_notes
            .extend(entries.into_iter().map(|(_, n)| n));
    }

    pub(crate) fn save_session(&mut self, cx: &mut Context<Self>) {
        self.record_tabs(cx);
        let active = self.active_rel();
        self.session.set_sidebar(self.sidebar_open);
        if let Some(w) = self.window_state {
            self.session.set_window(&w);
        }
        self.session.set_tabs(&self.session_notes);
        self.session
            .set_notes(&latest_per_space(&self.session_notes));
        self.session
            .set_active_tab(active.as_ref().map(|(s, r)| (s.as_path(), r.as_path())));
        let session = self.session.clone();
        cx.background_spawn(async move { session.save() }).detach();
    }

    /// Debounced (500ms) bounds persistence; the final write is on quit.
    pub(crate) fn bounds_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_state = Some(session_window(window));
        self._bounds_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            this.update(cx, |this, cx| this.save_session(cx)).ok();
        }));
    }

    /// Returning to the window: rescan the tree and stat the open files.
    pub(crate) fn activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.is_window_active() {
            return;
        }
        self.rescan_tree(cx);
        self.check_open_files(cx);
    }

    /// Watch the space folder: filesystem events trigger a tree rescan and a
    /// stat of the open files (debounced). Falls back to activation polling.
    pub(crate) fn start_watch(&mut self, cx: &mut Context<Self>) {
        self._watcher = None;
        self._watch_task = None;
        let (watcher, mut rx) = match crate::watch::watch(&self.dir) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("abstract: cannot watch space: {err}");
                return;
            }
        };
        self._watcher = Some(watcher);
        self._watch_task = Some(cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(mut changes) = rx.next().await {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                // Drain the burst that piled up during the debounce.
                while let Ok(more) = rx.try_recv() {
                    changes.extend(more);
                }
                let Ok(open) = this.update(cx, |this, _| {
                    this.tabs
                        .iter()
                        .map(|t| {
                            let f = guard(&t.file);
                            (f.path.clone(), f.mtime)
                        })
                        .collect::<Vec<_>>()
                }) else {
                    break;
                };
                // Our own save lands as a rename onto an open note; the
                // mtime it recorded tells it apart from an outside edit.
                for (path, recorded) in open {
                    let Some(recorded) = recorded else {
                        continue;
                    };
                    let now = cx
                        .background_executor()
                        .spawn({
                            let p = path.clone();
                            async move { std::fs::metadata(&p).and_then(|m| m.modified()).ok() }
                        })
                        .await;
                    if now == Some(recorded) {
                        changes.retain(|c| c.path != path);
                    }
                }
                if changes.is_empty() {
                    continue;
                }
                let tree = changes.iter().any(|c| !c.content);
                let is_note = |p: &Path| p.extension().is_some_and(|x| x == "md");
                let notes = tree || changes.iter().any(|c| is_note(&c.path));
                let assets = tree || changes.iter().any(|c| !is_note(&c.path));
                if this
                    .update(cx, |this, cx| {
                        if tree {
                            this.rescan_tree(cx);
                        }
                        this.check_open_files(cx);
                        if notes {
                            this.refresh_backlinks(cx);
                        }
                        if assets {
                            this.refresh_images(cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    /// Stat every open tab's file: reload on external change, notice on removal.
    pub(crate) fn check_open_files(&mut self, cx: &mut Context<Self>) {
        for ix in 0..self.tabs.len() {
            let file = self.tabs[ix].file.clone();
            cx.spawn(async move |this, cx| {
                let (path, recorded, deleted) = {
                    let f = guard(&file);
                    (f.path.clone(), f.mtime, f.deleted)
                };
                if deleted {
                    return;
                }
                let mtime = cx
                    .background_executor()
                    .spawn({
                        let p = path.clone();
                        async move { std::fs::metadata(&p).and_then(|m| m.modified()).ok() }
                    })
                    .await;
                this.update(cx, |this, cx| {
                    let Some(tix) = this.tabs.iter().position(|t| Arc::ptr_eq(&t.file, &file))
                    else {
                        return;
                    };
                    if this.tabs[tix].path() != path {
                        return;
                    }
                    match mtime {
                        // Vanished: keep the buffer; the next edit recreates it.
                        None if recorded.is_some() => {
                            guard(&file).mtime = None;
                            this.notice = Some(t(Key::FileRemovedOutside).into());
                            cx.notify();
                        }
                        Some(m) if recorded != Some(m) => {
                            // A pending save wins; it will record the new mtime.
                            if this.tabs[tix].save == SaveState::Pending {
                                return;
                            }
                            this.reload_file(file.clone(), path.clone(), cx);
                        }
                        _ => {}
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    /// The file changed on disk and nothing is pending: reload, keeping the
    /// tab's own cursor (clamped) and scroll.
    pub(crate) fn reload_file(
        &mut self,
        file: Arc<Mutex<NoteFile>>,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn({
                    let p = path.clone();
                    async move {
                        (
                            std::fs::read_to_string(&p).unwrap_or_default(),
                            std::fs::metadata(&p).and_then(|m| m.modified()).ok(),
                        )
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                let Some(tix) = this.tabs.iter().position(|t| Arc::ptr_eq(&t.file, &file)) else {
                    return;
                };
                {
                    let mut f = guard(&file);
                    if f.path != path || f.deleted {
                        return;
                    }
                    f.mtime = read.1;
                }
                let editor = this.tabs[tix].editor.clone();
                let (cursor, scroll) = editor.read(cx).view_state();
                let synced = vault::synced_stem(&stem_of(&path), &title_of(&read.0));
                this.tabs[tix].synced = synced;
                if this.active == Some(tix) {
                    if let Some(cur) = &mut this.current {
                        cur.synced = synced;
                    }
                    this.words = read.0.split_whitespace().count();
                }
                editor.update(cx, |ed, cx| {
                    ed.set_text(read.0, cx);
                    ed.restore_view(cursor, scroll, cx);
                });
                if this.active == Some(tix) {
                    this.refresh_find(false, None, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_pref == ThemePref::System {
            theme::apply(&self.settings, window.appearance(), cx);
            cx.notify();
        }
    }
}

/// Last entry per space — written as `note` lines so pre-tab versions
/// restore their most recent note.
fn latest_per_space(notes: &[SessionNote]) -> Vec<SessionNote> {
    let mut latest: Vec<SessionNote> = Vec::new();
    for n in notes {
        if let Some(prev) = latest.iter_mut().find(|p| p.space == n.space) {
            *prev = n.clone();
        } else {
            latest.push(n.clone());
        }
    }
    latest
}
