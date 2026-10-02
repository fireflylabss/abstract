use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};

use super::*;

/// A sidebar row being dragged onto a folder (or the space root).
#[derive(Clone)]
pub(crate) struct DraggedRow {
    pub path: PathBuf,
    pub kind: NodeKind,
    pub name: SharedString,
}

impl Render for DraggedRow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        div()
            .px(z(10.))
            .py(z(5.))
            .rounded(z(6.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .shadow_md()
            .text_size(z(13.))
            .text_color(rgb(pal.fg))
            .font_family(cx.global::<Fonts>().sans.clone())
            .child(self.name.clone())
    }
}

impl AbstractApp {
    /// Move `row` into `folder`, keeping its name. Refuses self-nesting and
    /// name collisions.
    pub(crate) fn move_into(&mut self, row: &DraggedRow, folder: PathBuf, cx: &mut Context<Self>) {
        let Some(name) = row.path.file_name() else {
            return;
        };
        let to = folder.join(name);
        if to == row.path || folder.starts_with(&row.path) {
            return;
        }
        if to.exists() {
            self.notice = Some(t(Key::NameTaken).into());
            cx.notify();
            return;
        }
        if folder != self.dir {
            self.expanded.insert(folder);
        }
        match row.kind {
            NodeKind::Note => self.rename_note_file(row.path.clone(), to, cx),
            NodeKind::Folder => self.rename_folder(row.path.clone(), to, cx),
        }
    }

    /// Copy a note next to itself (`name 2.md`) and open the copy.
    pub(crate) fn duplicate_note(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.flush(cx);
        // The open note's buffer may be ahead of its file.
        let text = self
            .current
            .as_ref()
            .filter(|c| c.path() == path)
            .map(|_| self.editor.read(cx).text().to_string());
        let lock = self.write_lock.clone();
        cx.spawn_in(window, async move |this, cx| {
            let copied = cx
                .background_executor()
                .spawn(async move {
                    let _w = guard(&lock);
                    let dir = path.parent()?.to_path_buf();
                    let to = vault::unique_path(&dir, &stem_of(&path), None);
                    let res = match text {
                        Some(text) => std::fs::write(&to, text),
                        None => std::fs::copy(&path, &to).map(|_| ()),
                    };
                    res.ok().map(|_| to)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                match copied {
                    Some(to) => this.open_path(to, None, window, cx),
                    None => this.notice = Some(t(Key::RenameFailed).into()),
                }
                this.rescan_tree(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn new_note_in(
        &mut self,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.target_folder = Some(folder.clone());
        self.expanded.insert(folder);
        self.new_note(window, cx);
    }

    pub(crate) fn new_folder_in(
        &mut self,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.target_folder = Some(folder.clone());
        self.expanded.insert(folder);
        self.new_folder(window, cx);
    }

    /// Right-click menu for a sidebar row.
    pub(crate) fn row_menu(
        this: &WeakEntity<Self>,
        path: &Path,
        kind: NodeKind,
        mut menu: PopupMenu,
    ) -> PopupMenu {
        let item =
            |label: &'static str,
             f: fn(&mut Self, PathBuf, NodeKind, &mut Window, &mut Context<Self>)| {
                let this = this.clone();
                let path = path.to_path_buf();
                PopupMenuItem::new(label).on_click(move |_, window, cx| {
                    let path = path.clone();
                    this.update(cx, |this, cx| f(this, path, kind, window, cx))
                        .ok();
                })
            };
        match kind {
            NodeKind::Note => {
                menu = menu
                    .item(item(t(Key::Open), |this, p, _, w, cx| {
                        this.open_path(p, None, w, cx)
                    }))
                    .item(item(t(Key::Rename), |this, p, k, w, cx| {
                        this.start_rename(p, k, w, cx)
                    }))
                    .item(item(t(Key::Duplicate), |this, p, _, w, cx| {
                        this.duplicate_note(p, w, cx)
                    }))
                    .separator()
                    .item(item(t(Key::CopyNoteLink), |_, p, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(format!(
                            "[[{}]]",
                            stem_of(&p)
                        )))
                    }))
                    .item(item(t(Key::ExportAsHtml), |this, p, _, w, cx| {
                        this.export_note(p, w, cx)
                    }));
            }
            NodeKind::Folder => {
                menu = menu
                    .item(item(t(Key::NewNoteHere), |this, p, _, w, cx| {
                        this.new_note_in(p, w, cx)
                    }))
                    .item(item(t(Key::NewFolderHere), |this, p, _, w, cx| {
                        this.new_folder_in(p, w, cx)
                    }))
                    .item(item(t(Key::Rename), |this, p, k, w, cx| {
                        this.start_rename(p, k, w, cx)
                    }))
                    .separator();
            }
        }
        menu.item(item(t(Key::CopyPath), |_, p, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(p.display().to_string()))
        }))
        .item(item(t(Key::CopyRelativePath), |this, p, _, _, cx| {
            let rel = p
                .strip_prefix(&this.dir)
                .unwrap_or(&p)
                .display()
                .to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(rel))
        }))
        .item(item(t(Key::Reveal), |_, p, _, _, cx| cx.reveal_path(&p)))
        .item(item(t(Key::OpenDefault), |_, p, _, _, cx| {
            cx.open_with_system(&p)
        }))
        .separator()
        .item(item(t(Key::MoveToTrash), |this, p, k, w, cx| {
            this.delete_row(p, k, w, cx)
        }))
    }
}
