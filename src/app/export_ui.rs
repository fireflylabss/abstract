use super::*;

impl AbstractApp {
    /// "Export as HTML…" for the note at `path`: the open note renders from
    /// the live buffer, any other note is read from disk.
    pub(crate) fn export_note(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self
            .current
            .as_ref()
            .filter(|c| c.path() == path)
            .map(|_| self.editor.read(cx).text().to_string())
            .unwrap_or_else(|| std::fs::read_to_string(&path).unwrap_or_default());
        let title = title_of(&text).to_string();
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.dir.clone());
        let suggested = format!("{}.html", vault::stem_for_title(&title));
        let picked = cx.prompt_for_new_path(&dir, Some(&suggested));
        let html = crate::html::export(
            &text,
            &title,
            &crate::html::Context {
                note_dir: Some(&dir),
                root: Some(&self.dir),
                tree: Some(&self.tree),
            },
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(dest))) = picked.await else {
                return;
            };
            let written = cx
                .background_executor()
                .spawn(async move {
                    store::write_atomic(&dest, html.as_bytes())
                        .is_ok()
                        .then_some(dest)
                })
                .await;
            this.update_in(cx, |this, _, cx| {
                this.notice = Some(match written {
                    Some(p) => tf(Key::Exported, &[("name", &p.display().to_string())]).into(),
                    None => t(Key::ExportFailed).into(),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Export action: the open note, when there is one.
    pub(crate) fn export_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.current.as_ref().map(|c| c.path()) else {
            return;
        };
        self.export_note(path, window, cx);
    }

    /// "Copy as HTML": the selection (or the whole note) as `text/html` with
    /// a `text/plain` fallback.
    pub(crate) fn copy_as_html(&mut self, cx: &mut Context<Self>) {
        let note_dir = self
            .current
            .as_ref()
            .and_then(|c| c.path().parent().map(Path::to_path_buf));
        let (html, plain) = {
            let ed = self.editor.read(cx);
            let sel = ed.selection();
            let text = ed.text();
            let range = if sel.is_empty() { 0..text.len() } else { sel };
            let ctx = crate::html::Context {
                note_dir: note_dir.as_deref(),
                root: Some(&self.dir),
                tree: Some(&self.tree),
            };
            (
                crate::html::fragment(text, ed.analysis(), range.clone(), &ctx),
                crate::html::plain(text, ed.analysis(), range),
            )
        };
        let ok = arboard::Clipboard::new()
            .and_then(|mut c| c.set_html(html, Some(plain)))
            .is_ok();
        if !ok {
            self.notice = Some(t(Key::CopyFailed).into());
            cx.notify();
        }
    }
}
