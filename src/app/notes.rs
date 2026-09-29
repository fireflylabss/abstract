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
                if let Some(p) = &this.pending_new {
                    let real = this.current.as_ref().map(|c| c.path());
                    if p.exists() || real.is_some_and(|r| r != *p) {
                        this.pending_new = None;
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

    pub(crate) fn load_buffer(
        &mut self,
        file: NoteFile,
        synced: bool,
        text: String,
        restore: Option<(usize, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.current = Some(CurrentNote {
            file: Arc::new(Mutex::new(file)),
            synced,
        });
        self.clear_completion(cx);
        self.open_gen += 1;
        self.save = SaveState::Saved;
        self.words = text.split_whitespace().count();
        self.sync_editor_dirs(cx);
        // `set_text` emits no `Changed`, so nothing is re-saved.
        self.editor.update(cx, |ed, cx| {
            ed.set_text(text, cx);
            if let Some((cursor, scroll)) = restore {
                ed.restore_view(cursor, scroll, cx);
            }
            ed.focus(window, cx);
        });
        self.refresh_find(false, None, cx);
        self.refresh_backlinks(cx);
        self.refresh_images(cx);
        cx.notify();
    }

    pub(crate) fn open_path(
        &mut self,
        path: PathBuf,
        restore: Option<(usize, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current.as_ref().is_some_and(|c| c.path() == path) {
            return;
        }
        self.record_session_note(cx);
        self.save_session(cx);
        self.flush(cx);
        self.pending_new = None;
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
                this.load_buffer(
                    NoteFile {
                        path,
                        mtime: read.1,
                        deleted: false,
                    },
                    synced,
                    read.0,
                    restore,
                    window,
                    cx,
                );
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
        self.record_session_note(cx);
        self.save_session(cx);
        self.flush(cx);
        self._io_task = None;
        let dir = self.target_dir();
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let path = vault::unique_path(&dir, &format!("nota-{stamp}"), None);
        self.expand_to(&path);
        self.pending_new = Some(path.clone());
        self.target_folder = None;
        self.load_buffer(
            NoteFile {
                path,
                mtime: None,
                deleted: false,
            },
            true,
            String::new(),
            None,
            window,
            cx,
        );
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
        let Some(cur) = self.current.take() else {
            return;
        };
        let path = cur.path();
        self._save_task = None;
        self._io_task = None;
        self.pending_new = None;
        let pending = self.save == SaveState::Pending;
        self.save = SaveState::Saved;
        let file = cur.file.clone();
        let synced = cur.synced;
        let lock = self.write_lock.clone();
        let space = self.dir.clone();
        let text = self.current_text(cx);
        cx.spawn(async move |this, cx| {
            let gone = cx
                .background_executor()
                .spawn(async move {
                    if pending {
                        let _ = write_note(&lock, &file, synced, &text, Some(&space));
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
        // Open the next newest note, or a fresh one.
        match vault::newest_note(&self.tree, Some(&path)) {
            Some(next) => self.open_path(next, None, window, cx),
            None => self.new_note(window, cx),
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
                let is_current = self.current.as_ref().is_some_and(|c| c.path() == path);
                if is_current {
                    self.delete_note(window, cx);
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
                // The open note inside it keeps its buffer as "removed".
                if let Some(cur) = &this.current
                    && cur.path().starts_with(&path)
                {
                    guard(&cur.file).mtime = None;
                    this.notice = Some(t(Key::FileRemovedOutside).into());
                }
                this.trash_path(path.clone(), cx);
            })
            .ok();
        })
        .detach();
    }
}
