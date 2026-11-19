use super::*;
use crate::core::i18n::tf;
use crate::io::export::{Exports, Format, Job, Snapshot};
use rune::FileDialogOptions;

#[derive(Debug, PartialEq)]
pub(super) enum Phase {
    Idle,
    Choosing,
    Rendering,
}

pub(super) struct ExportUi {
    worker: Exports,
    pub(super) phase: Phase,
    pub(super) notice: Option<String>,
}

impl ExportUi {
    pub fn new() -> Self {
        Self {
            worker: Exports::new(),
            phase: Phase::Idle,
            notice: None,
        }
    }
}

fn snapshot_tree(nodes: &[vault::Node]) -> Vec<vault::Node> {
    nodes
        .iter()
        .map(|node| vault::Node {
            path: node.path.clone(),
            name: node.name.clone(),
            kind: node.kind,
            modified: node.modified,
            children: snapshot_tree(&node.children),
        })
        .collect()
}

impl State {
    pub fn export_snapshot(&self, path: PathBuf) -> Snapshot {
        Snapshot {
            text: self.documents.get(&path).map(|doc| doc.edit.text.clone()),
            path,
            root: self.root.clone(),
            tree: snapshot_tree(&self.tree),
        }
    }

    pub fn export_note(&mut self, path: PathBuf, format: Format, tx: &mut Tx) {
        if self.export.phase != Phase::Idle || self.files_busy() {
            return;
        }
        let source = self.export_snapshot(path);
        let suggested = source
            .path
            .with_extension(format.extension())
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let options = FileDialogOptions {
            title: t(if format == Format::Html {
                Key::ExportAsHtml
            } else {
                Key::ExportAsPdf
            })
            .into(),
            directory: source.path.parent().map(std::path::Path::to_owned),
            file_name: Some(suggested),
            ..Default::default()
        }
        .filter(format.extension().to_uppercase(), &[format.extension()]);
        self.file_ops.mode = file_ops::Mode::Idle;
        self.export.phase = Phase::Choosing;
        tx.save_file(options, move |s: &mut State, result, tx| {
            s.export.phase = Phase::Idle;
            match result {
                Ok(Some(destination)) => {
                    s.start_export(
                        Job {
                            source,
                            destination,
                            format,
                        },
                        tx,
                    );
                }
                Ok(None) => {}
                Err(error) => s.export.notice = Some(format!("{}: {error}", t(Key::ExportFailed))),
            }
            tx.focus("editor", false);
        });
    }

    pub fn start_export(&mut self, job: Job, tx: &mut Tx) {
        match self.export.worker.submit(job) {
            Ok(()) => {
                self.export.phase = Phase::Rendering;
                self.export.notice = None;
                tx.wake_after(Duration::from_millis(16));
            }
            Err(error) => self.export.notice = Some(format!("{}: {error}", t(Key::ExportFailed))),
        }
    }

    fn finish_export(&mut self, result: Result<PathBuf, String>) {
        self.export.phase = Phase::Idle;
        self.export.notice = Some(match result {
            Ok(path) => tf(Key::Exported, &[("name", &path.display().to_string())]),
            Err(error) => format!("{}: {error}", t(Key::ExportFailed)),
        });
    }

    pub fn poll_export(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Ok(result) = self.export.worker.replies.try_recv() {
            self.finish_export(result);
            changed = true;
        }
        if self.export.phase == Phase::Rendering {
            tx.wake_after(Duration::from_millis(16));
        }
        changed
    }

    pub fn flush_export(&mut self) {
        if self.export.phase == Phase::Rendering {
            let result = self
                .export
                .worker
                .replies
                .recv()
                .unwrap_or_else(|e| Err(e.to_string()));
            self.finish_export(result);
        }
    }
}

pub(super) fn notice(s: &State, p: theme::Palette) -> El {
    let message = if s.export.phase == Phase::Rendering {
        Some(t(Key::Exporting).to_owned())
    } else {
        s.export.notice.clone()
    };
    let mut row = row().w_full().items_center().gap(8.).fill(rgb(p.panel));
    if let Some(message) = message {
        row = row.padding(6.).child(
            text(message)
                .key("export-status")
                .color(rgb(p.dim))
                .grow(1.),
        );
        if s.export.phase == Phase::Idle {
            row = row.child(
                text("×")
                    .key("export-dismiss")
                    .role("button")
                    .label(t(Key::Dismiss))
                    .focusable(true)
                    .color(rgb(p.dim))
                    .hover(rgb(p.hover))
                    .padding(4.)
                    .on_press(|s: &mut State| s.export.notice = None),
            );
        }
    }
    row
}

pub(super) fn menu(path: &std::path::Path, p: theme::Palette) -> El {
    let mut entries = col().w_full();
    for (format, key, id) in [
        (Format::Html, Key::ExportAsHtml, "export-html"),
        (Format::Pdf, Key::ExportAsPdf, "export-pdf"),
    ] {
        let path = path.to_owned();
        entries = entries.child(
            text(t(key))
                .key(id)
                .role("button")
                .focusable(true)
                .padding(7.)
                .color(rgb(p.fg))
                .hover(rgb(p.hover))
                .on_press_tx(move |s: &mut State, tx| s.export_note(path.clone(), format, tx)),
        );
    }
    entries
}
