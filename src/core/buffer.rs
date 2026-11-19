//! Text buffer: content, selection, IME marked range and undo/redo. Pure —
//! no gpui types, so it can be tested headless. `LiveEditor` owns one and
//! re-runs analysis/autoscroll after each mutating call.

use std::ops::Range;
use std::time::{Duration, Instant};

use unicode_segmentation::UnicodeSegmentation;

struct Snapshot {
    text: String,
    sel: Range<usize>,
}

pub struct Buffer {
    text: String,
    sel: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_typed: Option<Instant>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            sel: 0..0,
            reversed: false,
            marked: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_typed: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn sel(&self) -> Range<usize> {
        self.sel.clone()
    }

    pub fn reversed(&self) -> bool {
        self.reversed
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.sel.start
        } else {
            self.sel.end
        }
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn set_marked(&mut self, marked: Option<Range<usize>>) {
        self.marked = marked;
    }

    /// Replace the whole buffer (loading a note).
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.sel = 0..0;
        self.reversed = false;
        self.marked = None;
        self.undo.clear();
        self.redo.clear();
        self.last_typed = None;
    }

    /// Collapsed selection clamped to a char boundary.
    pub fn restore_cursor(&mut self, cursor: usize) {
        let mut c = cursor.min(self.text.len());
        while c > 0 && !self.text.is_char_boundary(c) {
            c -= 1;
        }
        self.sel = c..c;
        self.reversed = false;
    }

    /// Selection without the reversal logic (IME `new_sel` application).
    pub fn set_sel(&mut self, sel: Range<usize>) {
        self.sel = sel;
    }

    // ── Editing ──────────────────────────────────────────────────────────

    pub fn edit(&mut self, range: Range<usize>, new: &str, select: Option<Range<usize>>) {
        let typing = range.is_empty() && new.chars().count() == 1 && new != "\n";
        let coalesce = typing
            && self
                .last_typed
                .is_some_and(|t| t.elapsed() < Duration::from_millis(800));
        if !coalesce {
            self.undo.push(Snapshot {
                text: self.text.clone(),
                sel: self.sel.clone(),
            });
            if self.undo.len() > 300 {
                self.undo.remove(0);
            }
        }
        self.last_typed = typing.then(Instant::now);
        self.redo.clear();
        self.text.replace_range(range.clone(), new);
        let end = range.start + new.len();
        self.sel = select.unwrap_or(end..end);
        self.reversed = false;
        self.marked = None;
    }

    pub fn insert(&mut self, s: &str) {
        let r = self.sel.clone();
        self.edit(r, s, None);
    }

    /// Undo (`from_undo`) or redo. Returns false when the stack was empty.
    pub fn restore(&mut self, from_undo: bool) -> bool {
        let (src, dst) = if from_undo {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(snap) = src.pop() else { return false };
        dst.push(Snapshot {
            text: std::mem::replace(&mut self.text, snap.text),
            sel: self.sel.clone(),
        });
        self.sel = snap.sel.start.min(self.text.len())..snap.sel.end.min(self.text.len());
        self.reversed = false;
        self.marked = None;
        self.last_typed = None;
        true
    }

    pub fn wrap(&mut self, marker: &str) {
        let r = self.sel.clone();
        let inner = self.text[r.clone()].to_string();
        let start = r.start + marker.len();
        let new = format!("{marker}{inner}{marker}");
        self.edit(r, &new, Some(start..start + inner.len()));
    }

    // ── Movement ─────────────────────────────────────────────────────────

    pub fn move_to(&mut self, off: usize) {
        self.sel = off..off;
        self.reversed = false;
        self.last_typed = None;
    }

    pub fn select_to(&mut self, off: usize) {
        if self.reversed {
            self.sel.start = off;
        } else {
            self.sel.end = off;
        }
        if self.sel.end < self.sel.start {
            self.reversed = !self.reversed;
            self.sel = self.sel.end..self.sel.start;
        }
        self.last_typed = None;
    }

    pub fn select_all(&mut self) {
        self.sel = 0..self.text.len();
        self.reversed = false;
    }

    pub fn prev_boundary(&self, off: usize) -> usize {
        self.text[..off]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    pub fn next_boundary(&self, off: usize) -> usize {
        self.text[off..]
            .graphemes(true)
            .next()
            .map_or(self.text.len(), |g| off + g.len())
    }

    pub fn word_left(&self, off: usize) -> usize {
        let s = self.text[..off].trim_end_matches(|c: char| !c.is_alphanumeric());
        s.trim_end_matches(char::is_alphanumeric).len()
    }

    pub fn word_right(&self, off: usize) -> usize {
        let s = &self.text[off..];
        let a = s.len() - s.trim_start_matches(|c: char| !c.is_alphanumeric()).len();
        let r = &s[a..];
        off + a + (r.len() - r.trim_start_matches(char::is_alphanumeric).len())
    }

    // ── Key action bodies ────────────────────────────────────────────────

    pub fn backspace(&mut self) {
        if self.sel.is_empty() {
            let c = self.cursor();
            let p = self.prev_boundary(c);
            self.edit(p..c, "", None);
        } else {
            self.insert("");
        }
    }

    pub fn delete(&mut self) {
        if self.sel.is_empty() {
            let c = self.cursor();
            let n = self.next_boundary(c);
            self.edit(c..n, "", None);
        } else {
            self.insert("");
        }
    }

    pub fn delete_word_left(&mut self) {
        let c = self.cursor();
        let from = if self.sel.is_empty() {
            self.word_left(c)
        } else {
            self.sel.start
        };
        self.edit(from..self.sel.end.max(c), "", None);
    }

    /// Enter key. `line` is the current line's buffer range (from the
    /// analysis, which stays in the editor).
    pub fn enter(&mut self, line: Range<usize>) {
        match crate::core::md::list_prefix(&self.text[line.clone()]) {
            // Enter on an empty item ends the list.
            Some((len, _)) if line.len() == len && self.sel.is_empty() => {
                self.edit(line.start..line.end, "", None)
            }
            Some((_, next)) => self.insert(&format!("\n{next}")),
            None => self.insert("\n"),
        }
    }

    // ── UTF-16 bridging for the platform input handler ───────────────────

    pub fn to_utf16(&self, off: usize) -> usize {
        self.text[..off.min(self.text.len())].encode_utf16().count()
    }

    pub fn offset_from_utf16(&self, off16: usize) -> usize {
        let mut n = 0;
        for (i, ch) in self.text.char_indices() {
            if n >= off16 {
                return i;
            }
            n += ch.len_utf16();
        }
        self.text.len()
    }

    pub fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end)
    }
}

#[cfg(test)]
mod tests;
