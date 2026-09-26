use super::*;

impl AbstractApp {
    pub(crate) fn current_text(&self, cx: &App) -> String {
        self.editor.read(cx).text().to_string()
    }

    /// Debounced background write of the open note.
    pub(crate) fn schedule_save(&mut self, cx: &mut Context<Self>) {
        let Some(cur) = &self.current else { return };
        let file = cur.file.clone();
        let synced = cur.synced;
        let lock = self.write_lock.clone();
        self.save = SaveState::Pending;
        self._save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            // Path and text are captured now, not when the timer was set.
            let Ok(text) = this.update(cx, |this, cx| this.current_text(cx)) else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { write_note(&lock, &file, synced, &text) })
                .await;
            this.update(cx, |this, cx| this.finish_save(result, cx))
                .ok();
        }));
    }

    /// Ctrl+S: write now instead of waiting for the debounce.
    pub(crate) fn save_now(&mut self, cx: &mut Context<Self>) {
        let Some(cur) = &self.current else { return };
        self._save_task = None;
        let file = cur.file.clone();
        let synced = cur.synced;
        let lock = self.write_lock.clone();
        let text = self.current_text(cx);
        self.save = SaveState::Pending;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { write_note(&lock, &file, synced, &text) })
                .await;
            this.update(cx, |this, cx| this.finish_save(result, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn finish_save(&mut self, result: std::io::Result<bool>, cx: &mut Context<Self>) {
        match result {
            Ok(changed) => {
                self.save = SaveState::Saved;
                self.notice = None;
                if changed {
                    self.rescan_tree(cx);
                }
            }
            Err(err) => {
                eprintln!("abstract: failed to save note: {err}");
                self.save = SaveState::Failed;
            }
        }
        cx.notify();
    }

    /// Hand a pending save to the background before the buffer is replaced.
    pub(crate) fn flush(&mut self, cx: &mut Context<Self>) {
        self._save_task = None;
        if self.save != SaveState::Pending {
            return;
        }
        self.save = SaveState::Saved;
        if let Some(cur) = &self.current {
            let file = cur.file.clone();
            let synced = cur.synced;
            let lock = self.write_lock.clone();
            let text = self.current_text(cx);
            let write = cx.background_spawn(async move { write_note(&lock, &file, synced, &text) });
            cx.spawn(async move |this, cx| {
                let result = write.await;
                this.update(cx, |this, cx| this.finish_save(result, cx))
                    .ok();
            })
            .detach();
        }
    }

    /// On quit: write the pending save inline (the write lock waits for any
    /// in-flight op), then the session — note entry included.
    pub(crate) fn flush_blocking(&mut self, cx: &mut Context<Self>) {
        self._save_task = None;
        if self.save == SaveState::Pending
            && let Some(cur) = &self.current
        {
            let lock = self.write_lock.clone();
            let file = cur.file.clone();
            let _ = write_note(&lock, &file, cur.synced, &self.current_text(cx));
            self.save = SaveState::Saved;
        }
        self.record_session_note(cx);
        self.session.set_sidebar(self.sidebar_open);
        if let Some(w) = self.window_state {
            self.session.set_window(&w);
        }
        self.session.set_notes(&self.session_notes);
        self.session.save();
    }

    /// Remember where the open note was left (space + rel path + view).
    pub(crate) fn record_session_note(&mut self, cx: &App) {
        let Some(cur) = &self.current else { return };
        let path = cur.path();
        let Ok(rel) = path.strip_prefix(&self.dir) else {
            return;
        };
        // Nothing on disk yet (unwritten pending note) means nothing to
        // restore later.
        if rel.as_os_str().is_empty() || !path.exists() {
            return;
        }
        let (cursor, scroll) = self.editor.read(cx).view_state();
        // Drop the old entry for `rel` and any rel that no longer exists on
        // disk (renamed-away pending notes, externally deleted files).
        let dir = self.dir.clone();
        self.session_notes
            .retain(|n| n.space != dir || (n.rel != rel && dir.join(&n.rel).exists()));
        self.session_notes.push(SessionNote {
            space: self.dir.clone(),
            rel: rel.to_path_buf(),
            cursor,
            scroll,
        });
    }

    pub(crate) fn save_session(&self, cx: &mut Context<Self>) {
        let mut session = self.session.clone();
        session.set_sidebar(self.sidebar_open);
        if let Some(w) = self.window_state {
            session.set_window(&w);
        }
        session.set_notes(&self.session_notes);
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

    /// Returning to the window: rescan the tree and stat the open file.
    pub(crate) fn activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.is_window_active() {
            return;
        }
        self.rescan_tree(cx);
        self.check_open_file(cx);
    }

    /// Watch the space folder: filesystem events trigger a tree rescan and a
    /// stat of the open file (debounced). Falls back to activation polling.
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
            while rx.next().await.is_some() {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                // Drain the burst that piled up during the debounce.
                while rx.try_recv().is_ok() {}
                if this
                    .update(cx, |this, cx| {
                        this.rescan_tree(cx);
                        this.check_open_file(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    /// Stat the open file: reload on external change, notice on removal.
    pub(crate) fn check_open_file(&mut self, cx: &mut Context<Self>) {
        let Some(cur) = &self.current else { return };
        let file = cur.file.clone();
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
                if this
                    .current
                    .as_ref()
                    .is_none_or(|c| !Arc::ptr_eq(&c.file, &file))
                {
                    return;
                }
                if this.current.as_ref().unwrap().path() != path {
                    return;
                }
                match mtime {
                    // Vanished: keep the buffer; the next edit recreates it.
                    None if recorded.is_some() => {
                        guard(&file).mtime = None;
                        this.notice = Some("Arquivo removido fora do app".into());
                        cx.notify();
                    }
                    Some(m) if recorded != Some(m) => {
                        // A pending save wins; it will record the new mtime.
                        if this.save == SaveState::Pending {
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

    /// The file changed on disk and nothing is pending: reload, keeping the
    /// cursor (clamped) and scroll.
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
                if this
                    .current
                    .as_ref()
                    .is_none_or(|c| !Arc::ptr_eq(&c.file, &file))
                {
                    return;
                }
                {
                    let mut f = guard(&file);
                    if f.path != path || f.deleted {
                        return;
                    }
                    f.mtime = read.1;
                }
                let (cursor, scroll) = this.editor.read(cx).view_state();
                let synced = vault::synced_stem(&stem_of(&path), &title_of(&read.0));
                if let Some(cur) = &mut this.current {
                    cur.synced = synced;
                }
                this.words = read.0.split_whitespace().count();
                this.editor.update(cx, |ed, cx| {
                    ed.set_text(read.0, cx);
                    ed.restore_view(cursor, scroll, cx);
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_pref == ThemePref::System {
            theme::apply(ThemePref::System, window.appearance(), cx);
            cx.notify();
        }
    }
}
