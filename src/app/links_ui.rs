use super::*;
use crate::editor::{CompletionKey, CompletionMove};

/// Open `[[…]]` completion: buffer range of the prefix plus the items shown.
pub(super) struct Completion {
    range: Range<usize>,
    items: Vec<String>,
    selected: usize,
}

impl AbstractApp {
    /// Ctrl/`Cmd`-click on `[[target]]`: open the note it names, creating it
    /// with a `# target` heading when nothing resolves.
    pub(crate) fn open_link(&mut self, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = crate::links::resolve(&self.tree, target) {
            self.open_path(path, None, window, cx);
            return;
        }
        let stem = vault::stem_for_title(target);
        let path = vault::unique_path(&self.dir, &stem, None);
        if let Err(err) = store::write_atomic(&path, format!("# {target}\n\n").as_bytes()) {
            eprintln!("abstract: cannot create linked note: {err}");
            self.notice = Some(t(Key::CreateNoteFailed).into());
            cx.notify();
            return;
        }
        self.expand_to(&path);
        self.rescan_tree(cx);
        self.open_path(path, None, window, cx);
    }

    /// Recompute the `[[…]]` completion on every buffer change.
    pub(crate) fn update_completion(&mut self, cx: &mut Context<Self>) {
        let prefix = self.editor.read(cx).wiki_prefix();
        match prefix {
            Some((range, p)) => {
                let items = crate::links::complete(&self.tree, &p);
                if items.is_empty() {
                    self.clear_completion(cx);
                    return;
                }
                let keep = self
                    .completion
                    .as_ref()
                    .filter(|c| c.range.start == range.start)
                    .map(|c| c.selected.min(items.len() - 1))
                    .unwrap_or(0);
                self.completion = Some(Completion {
                    range,
                    items,
                    selected: keep,
                });
                self.editor.update(cx, |ed, _| ed.set_completing(true));
            }
            None => self.clear_completion(cx),
        }
    }

    pub(crate) fn clear_completion(&mut self, cx: &mut Context<Self>) {
        if self.completion.take().is_some() {
            self.editor.update(cx, |ed, _| ed.set_completing(false));
            cx.notify();
        }
    }

    fn accept_completion(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(c) = self.completion.take() else {
            return;
        };
        let Some(item) = c.items.get(ix).cloned() else {
            return;
        };
        self.editor.update(cx, |ed, cx| {
            ed.set_completing(false);
            ed.complete_wiki(c.range, &item, cx);
        });
        cx.notify();
    }

    pub(crate) fn completion_key(&mut self, key: &CompletionKey, cx: &mut Context<Self>) {
        match key.0 {
            CompletionMove::Up | CompletionMove::Down => {
                let Some(c) = &mut self.completion else {
                    return;
                };
                let n = c.items.len();
                c.selected = if matches!(key.0, CompletionMove::Down) {
                    (c.selected + 1) % n
                } else {
                    (c.selected + n - 1) % n
                };
                cx.notify();
            }
            CompletionMove::Accept => {
                let ix = self.completion.as_ref().map(|c| c.selected).unwrap_or(0);
                self.accept_completion(ix, cx);
            }
            CompletionMove::Cancel => self.clear_completion(cx),
        }
    }

    /// Recompute the "Referenciada por" list off-thread; applied only while
    /// the same note is still open.
    pub(crate) fn refresh_backlinks(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.current.as_ref().map(|c| c.path()) else {
            self.backlinks_key = None;
            if !self.backlinks.is_empty() {
                self.backlinks.clear();
                cx.notify();
            }
            return;
        };
        let names = crate::links::names_of(&path, self.editor.read(cx).text());
        self.backlinks_key = Some((path.clone(), names.clone()));
        let dir = self.dir.clone();
        let note = path.clone();
        let index = self.link_index.clone();
        self._backlinks_task = Some(cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { guard(&index).backlinks(&dir, &note, &names) })
                .await;
            this.update(cx, |this, cx| {
                if this.current.as_ref().is_some_and(|c| c.path() == path) {
                    this.backlinks = found;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    /// Recompute backlinks only when the open note's path or names moved
    /// since the last computation (its own edits cannot change who links to it).
    pub(crate) fn refresh_backlinks_if_renamed(&mut self, cx: &mut Context<Self>) {
        let key = self.current.as_ref().map(|c| {
            let path = c.path();
            let names = crate::links::names_of(&path, self.editor.read(cx).text());
            (path, names)
        });
        if key != self.backlinks_key {
            self.refresh_backlinks(cx);
        }
    }

    /// Backlinks strip pinned to the bottom of the editor column.
    pub(crate) fn render_backlinks(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.backlinks.is_empty() {
            return None;
        }
        let pal = cx.palette();
        let mut rows = div()
            .flex()
            .flex_col()
            .px(px(10.))
            .pb(px(8.))
            .max_h(px(120.))
            .overflow_hidden();
        rows = rows.child(
            div()
                .pt(px(6.))
                .pb(px(2.))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(pal.faint))
                .child(t(Key::ReferencedBy)),
        );
        for (ix, (path, title)) in self.backlinks.iter().enumerate() {
            let p = path.clone();
            rows = rows.child(
                div()
                    .id(("backlink", ix))
                    .h(px(22.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(rgb(pal.body))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_path(p.clone(), None, window, cx)
                    }))
                    .child(icon("icons/link.svg", pal.faint).size(px(12.)))
                    .child(title.clone()),
            );
        }
        Some(
            div()
                .flex_none()
                .w_full()
                .child(div().h(px(1.)).mx(px(10.)).bg(rgb(pal.line)))
                .child(rows),
        )
    }

    /// Popup under the caret listing completion stems; mounted inside the
    /// (relative) editor column.
    pub(crate) fn render_completion(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let c = self.completion.as_ref()?;
        let anchor = self.editor.read(cx).caret_anchor()?;
        let pal = cx.palette();
        let mut rows = div().flex().flex_col();
        for (ix, item) in c.items.iter().enumerate() {
            rows = rows.child(
                div()
                    .id(("cmpl", ix))
                    .px(px(8.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(rgb(if ix == c.selected { pal.fg } else { pal.body }))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(ix == c.selected, |s| s.bg(rgb(pal.active)))
                    .when(ix != c.selected, |s| s.hover(|s| s.bg(rgb(pal.hover))))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(move |this, _, _, cx| this.accept_completion(ix, cx)))
                    .child(item.clone()),
            );
        }
        Some(
            div()
                .id("wiki-completion")
                .absolute()
                .left(px(anchor.x.max(8.)))
                .top(px(anchor.y + 4.))
                .w(px(240.))
                .max_h(px(180.))
                .overflow_hidden()
                .bg(rgb(pal.menu_bg))
                .border_1()
                .border_color(rgb(pal.menu_border))
                .rounded(px(8.))
                .shadow_lg()
                .occlude()
                .p(px(4.))
                .child(rows),
        )
    }
}
