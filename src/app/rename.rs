use super::*;

impl AbstractApp {
    // ── Inline rename / create ────────────────────────────────────────────

    pub(crate) fn start_edit(
        &mut self,
        target: PathBuf,
        kind: NodeKind,
        create: bool,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let sub = cx.subscribe(&state, |this: &mut Self, _, ev: &InputEvent, cx| match ev {
            InputEvent::PressEnter { .. } => this.commit_edit(cx),
            InputEvent::Blur => this.cancel_edit(cx),
            _ => {}
        });
        self.editing = Some(RenameEdit {
            state: state.clone(),
            target,
            kind,
            create,
            _sub: sub,
        });
        state.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn start_rename(
        &mut self,
        path: PathBuf,
        kind: NodeKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.is_some() {
            return;
        }
        let name = match kind {
            NodeKind::Folder => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            NodeKind::Note => stem_of(&path),
        };
        self.start_edit(path, kind, false, name, window, cx);
    }

    /// F2 on the open note.
    pub(crate) fn rename_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cur) = &self.current else { return };
        let path = cur.path();
        self.start_rename(path, NodeKind::Note, window, cx);
    }

    pub(crate) fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let dir = self.target_dir();
        self.expand_to(&dir.join("x"));
        if dir != self.dir {
            self.expanded.insert(dir.clone());
        }
        self.start_edit(
            dir,
            NodeKind::Folder,
            true,
            t(Key::NewFolder).to_string(),
            window,
            cx,
        );
    }

    pub(crate) fn focus_edit(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.editing.as_ref().map(|e| e.state.clone()) else {
            return;
        };
        if let Some(w) = cx.windows().first().copied() {
            w.update(cx, |_, window, cx| {
                state.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
            })
            .ok();
        }
    }

    pub(crate) fn focus_editor(&mut self, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        if let Some(w) = cx.windows().first().copied() {
            w.update(cx, |_, window, cx| {
                editor.update(cx, |ed, cx| ed.focus(window, cx))
            })
            .ok();
        }
    }

    /// Keep the input open and complain.
    pub(crate) fn edit_conflict(&mut self, ed: RenameEdit, cx: &mut Context<Self>) {
        self.notice = Some(t(Key::NameConflict).into());
        self.editing = Some(ed);
        self.focus_edit(cx);
        cx.notify();
    }

    pub(crate) fn commit_edit(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.editing.take() else {
            return;
        };
        let raw = ed.state.read(cx).value().to_string();
        if raw.trim().is_empty() {
            self.focus_editor(cx);
            cx.notify();
            return;
        }
        let name = vault::stem_for_title(raw.trim());
        if ed.create {
            let target = ed.target.join(&name);
            if target.exists() {
                self.edit_conflict(ed, cx);
                return;
            }
            self.expanded.insert(ed.target.clone());
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { std::fs::create_dir(&target) })
                    .await;
                this.update(cx, |this, cx| {
                    if let Err(err) = result {
                        eprintln!("abstract: failed to create folder: {err}");
                    }
                    this.rescan_tree(cx);
                })
                .ok();
            })
            .detach();
            self.focus_editor(cx);
            cx.notify();
            return;
        }

        let Some(parent) = ed.target.parent().map(Path::to_path_buf) else {
            return;
        };
        let newp = match ed.kind {
            NodeKind::Folder => parent.join(&name),
            NodeKind::Note => parent.join(format!("{name}.md")),
        };
        if newp == ed.target {
            self.focus_editor(cx);
            cx.notify();
            return;
        }
        if newp.exists() {
            self.edit_conflict(ed, cx);
            return;
        }
        match ed.kind {
            NodeKind::Folder => self.rename_folder(ed.target.clone(), newp, cx),
            NodeKind::Note => self.rename_note_file(ed.target.clone(), newp, cx),
        }
        self.focus_editor(cx);
        cx.notify();
    }

    pub(crate) fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            self.focus_editor(cx);
            cx.notify();
        }
    }

    /// Update every path the app tracks after `old` moves to `new`.
    pub(crate) fn remap_prefix(&mut self, old: &Path, new: &Path, cx: &mut Context<Self>) {
        let remap = |p: &Path| -> PathBuf {
            p.strip_prefix(old).map_or_else(
                |_| p.to_path_buf(),
                |rest| {
                    if rest.as_os_str().is_empty() {
                        new.to_path_buf()
                    } else {
                        new.join(rest)
                    }
                },
            )
        };
        self.expanded = self.expanded.iter().map(|p| remap(p)).collect();
        if let Some(t) = &self.target_folder {
            self.target_folder = Some(remap(t));
        }
        for tab in &self.tabs {
            let mut f = guard(&tab.file);
            f.path = remap(&f.path);
        }
        for n in &mut self.session_notes {
            if n.space == self.dir
                && let Ok(rest) = n
                    .rel
                    .strip_prefix(old.strip_prefix(&self.dir).unwrap_or(old))
            {
                n.rel = new.strip_prefix(&self.dir).unwrap_or(new).join(rest);
            }
        }
        self.pins = self.pins.iter().map(|p| remap(p)).collect();
        for set in self.tag_paths.values_mut() {
            *set = std::mem::take(set).into_iter().map(|p| remap(&p)).collect();
        }
        self.save_session(cx);
        self.sync_editor_dirs(cx);
    }

    pub(crate) fn rename_folder(&mut self, old: PathBuf, new: PathBuf, cx: &mut Context<Self>) {
        let lock = self.write_lock.clone();
        // Open notes inside move their paths with the folder.
        let inside: Vec<Arc<Mutex<NoteFile>>> = self
            .tabs
            .iter()
            .filter(|t| t.path().starts_with(&old))
            .map(|t| t.file.clone())
            .collect();
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_executor()
                .spawn({
                    let old = old.clone();
                    let new = new.clone();
                    async move {
                        let _w = guard(&lock);
                        match std::fs::rename(&old, &new) {
                            Ok(()) => {
                                for f in &inside {
                                    let mut f = guard(f);
                                    if let Ok(rest) = f.path.strip_prefix(&old) {
                                        f.path = new.join(rest);
                                    }
                                }
                                true
                            }
                            Err(err) => {
                                eprintln!("abstract: failed to rename folder: {err}");
                                false
                            }
                        }
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if ok {
                    this.remap_prefix(&old, &new, cx);
                } else {
                    this.notice = Some(t(Key::RenameFailed).into());
                }
                this.rescan_tree(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn rename_note_file(&mut self, old: PathBuf, new: PathBuf, cx: &mut Context<Self>) {
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let open = self.tab_paths();
        let current = self
            .tabs
            .iter()
            .find(|t| t.path() == old)
            .map(|t| t.file.clone());
        cx.spawn(async move |this, cx| {
            let (ok, linked) = cx
                .background_executor()
                .spawn({
                    let old = old.clone();
                    let new = new.clone();
                    let current = current.clone();
                    let open = open.clone();
                    async move {
                        let _w = guard(&lock);
                        let moved = if let Some(f) = &current {
                            // Pending new note: the file does not exist yet, just
                            // update the planned path.
                            let mut f = guard(f);
                            if !old.exists() || std::fs::rename(&old, &new).is_ok() {
                                f.path = new.clone();
                                true
                            } else {
                                false
                            }
                        } else if std::fs::rename(&old, &new).is_ok() {
                            rebase_file(&new, &old);
                            true
                        } else {
                            false
                        };
                        let linked = if moved {
                            crate::links::relink(&space, &new, &stem_of(&old), &open, false)
                        } else {
                            None
                        };
                        (moved, linked)
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(failed) = linked {
                    let (from, to) = (stem_of(&old), stem_of(&new));
                    // Every still-open buffer retargets its own links; a tab
                    // closed mid-rename gets its file rewritten instead.
                    for tab in &this.tabs {
                        tab.editor
                            .update(cx, |ed, cx| ed.retarget_links(&from, &to, cx));
                    }
                    for prev in &open {
                        if *prev != old && !this.tabs.iter().any(|t| t.path() == *prev) {
                            this.relink_closed(prev.clone(), from.clone(), to.clone(), cx);
                        }
                    }
                    if failed > 0 {
                        this.notice = Some(t(Key::RelinkFailed).into());
                    }
                }
                if ok {
                    // Manual rename recomputes synced against the buffer.
                    if let Some(f) = current
                        && let Some(tix) = this.tabs.iter().position(|t| Arc::ptr_eq(&t.file, &f))
                    {
                        let title = title_of(this.tabs[tix].editor.read(cx).text());
                        let synced = vault::synced_stem(&stem_of(&new), &title);
                        this.tabs[tix].synced = synced;
                        if this.active == Some(tix)
                            && let Some(cur) = &mut this.current
                        {
                            cur.synced = synced;
                        }
                        if let (Some(from), Some(to)) = (old.parent(), new.parent()) {
                            this.tabs[tix]
                                .editor
                                .update(cx, |ed, cx| ed.rebase_images(from, to, cx));
                        }
                    }
                    this.remap_prefix(&old, &new, cx);
                } else {
                    eprintln!("abstract: failed to rename {}", old.display());
                    this.notice = Some(t(Key::RenameFailed).into());
                }
                this.rescan_tree(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Retargets links in a note that was open when a rename started and has
    /// since been closed, after its pending save.
    fn relink_closed(&mut self, path: PathBuf, from: String, to: String, cx: &mut Context<Self>) {
        let lock = self.write_lock.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let _w = guard(&lock);
                    crate::links::retarget_file(&path, &from, &to)
                })
                .await;
            if let Err(err) = result {
                eprintln!("abstract: cannot relink: {err}");
                this.update(cx, |this, cx| {
                    this.notice = Some(t(Key::RelinkFailed).into());
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

/// Keep relative image sources of the note moved from `old` to `new` valid.
fn rebase_file(new: &Path, old: &Path) {
    let (Some(from), Some(to)) = (old.parent(), new.parent()) else {
        return;
    };
    if from == to {
        return;
    }
    let Ok(mut text) = std::fs::read_to_string(new) else {
        return;
    };
    let images = crate::md::Analyzer::new().analyze(&text).images;
    let edits = crate::attach::rebase(&text, &images, from, to);
    if edits.is_empty() {
        return;
    }
    for (r, s) in edits {
        text.replace_range(r, &s);
    }
    if let Err(err) = std::fs::write(new, text) {
        eprintln!("abstract: cannot rewrite {}: {err}", new.display());
    }
}
