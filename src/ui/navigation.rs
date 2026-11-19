use super::*;
use crate::core::links::follow::{self, Target};
use std::ops::Range;

impl State {
    pub(super) fn buffers(&self) -> HashMap<PathBuf, String> {
        self.documents
            .iter()
            .map(|(path, doc)| (path.clone(), doc.edit.text.clone()))
            .collect()
    }

    pub(super) fn open_at(&mut self, path: PathBuf, range: Range<usize>, tx: &mut Tx) {
        self.open(path.clone(), tx);
        self.jump = Some((self.file_ticket, path, range));
        self.apply_jump(tx);
    }

    pub fn follow_link(&mut self, tx: &mut Tx) {
        let Some(doc) = self.document() else { return };
        let Some(analysis) = doc.analysis.get(doc.revision) else {
            return;
        };
        let current = self.active.clone().unwrap();
        let Some(target) = follow::at(
            &doc.edit.text,
            doc.edit.focus,
            analysis,
            &self.root,
            &current,
        ) else {
            return;
        };
        if let Target::External(url) = target {
            if let Err(error) = tx.open_url(&url) {
                self.error = Some(error.to_string());
            }
            return;
        }
        self.file_ticket += 1;
        self.navigation.start(
            self.file_ticket,
            self.root.clone(),
            current,
            target,
            self.buffers(),
        );
        tx.wake_after(Duration::from_millis(16));
    }

    pub(super) fn apply_jump(&mut self, tx: &mut Tx) {
        let Some((ticket, path, _)) = &self.jump else {
            return;
        };
        if *ticket != self.file_ticket {
            self.jump = None;
            return;
        }
        if self.active.as_ref() != Some(path) {
            return;
        }
        let (_, _, range) = self.jump.take().unwrap();
        if let Some(doc) = self.document_mut() {
            doc.edit.set_selection(range.start, range.end);
            tx.focus("editor", false);
            self.refresh_find(false, tx);
            self.persist_tabs(tx);
        }
    }

    pub(super) fn poll_navigation(&mut self, tx: &mut Tx) -> bool {
        let Some(reply) = self.navigation.poll() else {
            return false;
        };
        self.scans.start(self.root.clone());
        self.scanning = true;
        if reply.ticket != self.file_ticket {
            return false;
        }
        match reply.result {
            Ok(loaded) => {
                self.documents
                    .entry(loaded.path.clone())
                    .or_insert_with(|| Document::new(loaded.source));
                self.tabs.open(loaded.path.clone());
                self.focus_tab(tx);
                let range = self
                    .document()
                    .map(|doc| follow::heading_range(&doc.edit.text, &loaded.heading))
                    .unwrap_or(0..0);
                self.jump = Some((reply.ticket, loaded.path, range));
                self.apply_jump(tx);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{}: {error}", t(Key::OpenFailed))),
        }
        true
    }
}
