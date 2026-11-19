use super::*;
use crate::core::attach::Incoming;
use crate::io::attachments::{Imports, Job};

struct Pending {
    path: PathBuf,
    revision: u64,
    selection: (usize, usize),
}

pub(super) struct Attachments {
    worker: Imports,
    pending: Option<Pending>,
    notice: Option<String>,
}

impl Attachments {
    pub fn new() -> Self {
        Self {
            worker: Imports::new(),
            pending: None,
            notice: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn pending_for(&self, path: &std::path::Path) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.path == path)
    }
}

impl State {
    pub fn paste_attachment(&mut self, content: &rune::ClipboardContent, tx: &mut Tx) -> bool {
        let items = if let Some(image) = content.image() {
            vec![Incoming::Image {
                bytes: image.png().to_vec(),
                ext: "png",
            }]
        } else {
            content
                .files()
                .iter()
                .filter(|p| crate::core::md::is_image_path(&p.to_string_lossy()))
                .cloned()
                .map(Incoming::Path)
                .collect()
        };
        if items.is_empty() {
            return false;
        }
        self.import_attachments(items, tx);
        true
    }

    pub fn drop_images(&mut self, paths: &[PathBuf], tx: &mut Tx) {
        let items = paths
            .iter()
            .filter(|p| crate::core::md::is_image_path(&p.to_string_lossy()))
            .cloned()
            .map(Incoming::Path)
            .collect();
        self.import_attachments(items, tx);
    }

    pub fn import_attachments(&mut self, items: Vec<Incoming>, tx: &mut Tx) {
        if items.is_empty() {
            return;
        }
        if self.attachments.busy() || self.files_busy() {
            return;
        }
        let (Some(path), Some(doc)) = (self.active.clone(), self.document()) else {
            return;
        };
        let pending = Pending {
            path: path.clone(),
            revision: doc.revision,
            selection: (doc.edit.anchor, doc.edit.focus),
        };
        match self.attachments.worker.submit(Job {
            path,
            root: self.root.clone(),
            items,
        }) {
            Ok(()) => {
                self.attachments.pending = Some(pending);
                self.attachments.notice = None;
            }
            Err(error) => {
                self.attachments.notice = Some(format!("{}: {error}", t(Key::AttachFailed)))
            }
        }
        tx.wake_after(Duration::from_millis(16));
    }

    fn finish_import(&mut self, result: Result<Vec<String>, String>, tx: &mut Tx) {
        let Some(pending) = self.attachments.pending.take() else {
            return;
        };
        self.attachments.notice = None;
        match result {
            Ok(items) => {
                let Some(doc) = self.documents.get_mut(&pending.path) else {
                    return;
                };
                let unchanged = doc.revision == pending.revision;
                let selection = (doc.edit.anchor, doc.edit.focus);
                if unchanged {
                    doc.edit
                        .set_selection(pending.selection.0, pending.selection.1);
                } else {
                    doc.edit.set_caret(doc.edit.text.len());
                }
                let prefix =
                    if doc.edit.focus > 0 && !doc.edit.text[..doc.edit.focus].ends_with('\n') {
                        "\n"
                    } else {
                        ""
                    };
                doc.edit.insert(&format!("{prefix}{}\n", items.join("\n")));
                if !unchanged {
                    doc.edit.set_selection(selection.0, selection.1);
                }
                self.edited_path(&pending.path, tx);
            }
            Err(error) => {
                self.attachments.notice = Some(format!("{}: {error}", t(Key::AttachFailed)))
            }
        }
    }

    pub fn poll_attachments(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Ok(result) = self.attachments.worker.replies.try_recv() {
            self.finish_import(result, tx);
            changed = true;
        }
        if self.attachments.busy() {
            tx.wake_after(Duration::from_millis(16));
        }
        changed
    }

    pub fn flush_attachments(&mut self) {
        if self.attachments.busy() {
            let result = self
                .attachments
                .worker
                .replies
                .recv()
                .unwrap_or_else(|e| Err(e.to_string()));
            self.finish_import(result, &mut Tx::default());
        }
    }
}

pub(super) fn notice(s: &State, p: theme::Palette) -> El {
    let message = if s.attachments.busy() {
        Some(t(Key::ImportingImages).to_owned())
    } else {
        s.attachments.notice.clone()
    };
    let mut row = row().w_full().items_center().gap(8.).fill(rgb(p.panel));
    if let Some(message) = message {
        row = row.padding(6.).child(
            text(message)
                .key("attachment-status")
                .color(rgb(p.dim))
                .grow(1.),
        );
        if !s.attachments.busy() {
            row = row.child(
                text("×")
                    .key("attachment-dismiss")
                    .role("button")
                    .label(t(Key::Dismiss))
                    .focusable(true)
                    .color(rgb(p.dim))
                    .hover(rgb(p.hover))
                    .padding(4.)
                    .on_press(|s: &mut State| s.attachments.notice = None),
            );
        }
    }
    row
}
