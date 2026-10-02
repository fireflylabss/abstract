use super::*;
use crate::attach::{self, Incoming};

impl AbstractApp {
    /// Tell every tab's editor where relative image sources resolve.
    pub(crate) fn sync_editor_dirs(&mut self, cx: &mut Context<Self>) {
        let root = self.dir.clone();
        for tab in &self.tabs {
            let dir = tab.path().parent().map(Path::to_path_buf);
            tab.editor
                .update(cx, |ed, cx| ed.set_dirs(dir, root.clone(), cx));
        }
    }

    /// Store pasted/dropped items off-thread, then reference them at the
    /// caret, unless another note was opened meanwhile.
    /// Drop cached decodes of the open note's images whose file changed (or
    /// appeared) since they were last seen, so edits on disk show up.
    pub(crate) fn refresh_images(&mut self, cx: &mut Context<Self>) {
        let paths = self.editor.read(cx).image_paths();
        if paths.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let stamps: Vec<(PathBuf, Option<SystemTime>)> = cx
                .background_executor()
                .spawn(async move {
                    paths
                        .into_iter()
                        .map(|p| {
                            let m = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
                            (p, m)
                        })
                        .collect()
                })
                .await;
            this.update(cx, |this, cx| {
                let mut stale = false;
                for (p, m) in stamps {
                    if this
                        .image_stamps
                        .insert(p.clone(), m)
                        .is_some_and(|old| old != m)
                    {
                        cx.remove_asset::<ImgResourceLoader>(&Resource::Path(Arc::from(
                            p.as_path(),
                        )));
                        stale = true;
                    }
                }
                if stale {
                    this.editor.update(cx, |_, cx| cx.notify());
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn attach(&mut self, items: Vec<Incoming>, cx: &mut Context<Self>) {
        let Some(cur) = &self.current else { return };
        let Some(note_dir) = cur.path().parent().map(Path::to_path_buf) else {
            return;
        };
        let file = cur.file.clone();
        let root = self.dir.clone();
        let stamp = attach::stamp();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { attach::import(items, &note_dir, &root, &stamp) })
                .await;
            this.update(cx, |this, cx| {
                match res {
                    Ok(refs) if !refs.is_empty() => {
                        let same = this
                            .current
                            .as_ref()
                            .is_some_and(|c| Arc::ptr_eq(&c.file, &file));
                        if same {
                            let text = refs.join("\n");
                            this.editor.update(cx, |ed, cx| ed.insert_text(&text, cx));
                        }
                        this.rescan_tree(cx);
                    }
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("abstract: cannot attach: {err}");
                        this.notice = Some(t(Key::AttachFailed).into());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn insert_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t(Key::InsertImage).into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.attach(paths.into_iter().map(Incoming::Path).collect(), cx)
            })
            .ok();
        })
        .detach();
    }
}
