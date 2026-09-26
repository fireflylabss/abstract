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
        match crate::md::list_prefix(&self.text[line.clone()]) {
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
mod tests {
    use super::*;

    fn buf_with(text: &str) -> Buffer {
        let mut b = Buffer::new();
        b.set_text(text.to_string());
        b
    }

    #[test]
    fn insert_advances_cursor() {
        let mut b = buf_with("ab");
        b.move_to(1);
        b.insert("x");
        assert_eq!(b.text(), "axb");
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn typing_coalesces_into_one_undo() {
        let mut b = Buffer::new();
        b.insert("a");
        b.insert("b");
        assert_eq!(b.undo.len(), 1);
        assert!(b.restore(true));
        assert_eq!(b.text(), "");
        assert!(!b.restore(true));
    }

    #[test]
    fn newline_breaks_coalescing() {
        let mut b = Buffer::new();
        b.insert("a");
        b.insert("\n");
        assert_eq!(b.undo.len(), 2);
        assert!(b.restore(true));
        assert_eq!(b.text(), "a");
        assert!(b.restore(true));
        assert_eq!(b.text(), "");
    }

    #[test]
    fn undo_restores_text_and_selection_redo_reapplies() {
        let mut b = buf_with("hello");
        b.move_to(5);
        b.insert(" world");
        b.move_to(2);
        assert!(b.restore(true));
        assert_eq!(b.text(), "hello");
        assert_eq!(b.sel(), 5..5);
        assert!(b.restore(false));
        assert_eq!(b.text(), "hello world");
        // Redo restores the sel captured at undo time (2..2), not the post-edit one.
        assert_eq!(b.sel(), 2..2);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut b = buf_with("x");
        b.move_to(1);
        b.insert("y");
        assert!(b.restore(true));
        b.insert("z");
        assert!(!b.restore(false));
        assert_eq!(b.text(), "xz");
    }

    #[test]
    fn undo_stack_capped_at_300() {
        let mut b = Buffer::new();
        for _ in 0..301 {
            b.insert("ab");
        }
        assert_eq!(b.undo.len(), 300);
    }

    #[test]
    fn wrap_marks_and_resleects() {
        let mut b = buf_with("abc");
        b.move_to(0);
        b.select_to(3);
        b.wrap("**");
        assert_eq!(b.text(), "**abc**");
        assert_eq!(b.sel(), 2..5);

        let mut b = buf_with("ab");
        b.move_to(1);
        b.wrap("**");
        assert_eq!(b.text(), "a****b");
        assert_eq!(b.cursor(), 3);
    }

    #[test]
    fn backspace_and_delete_remove_grapheme_clusters() {
        let mut b = buf_with("a\u{1F44D}\u{1F3FD}b"); // "a👍🏽b": cluster is bytes 1..9
        b.move_to(9);
        b.backspace();
        assert_eq!(b.text(), "ab");

        let mut b = buf_with("a\u{1F44D}\u{1F3FD}b");
        b.move_to(1);
        b.delete();
        assert_eq!(b.text(), "ab");
    }

    #[test]
    fn backspace_with_selection_deletes_only_selection() {
        let mut b = buf_with("abc");
        b.move_to(0);
        b.select_to(1);
        b.backspace();
        assert_eq!(b.text(), "bc");
    }

    #[test]
    fn word_movement_and_delete() {
        let b = buf_with("foo  bar-baz qux");
        assert_eq!(b.word_left(b.text().len()), 13);
        assert_eq!(b.word_right(0), 3);
        assert_eq!(b.word_right(3), 8);

        let mut b = buf_with("foo  bar-baz qux");
        b.move_to(8);
        b.delete_word_left();
        assert_eq!(b.text(), "foo  -baz qux");
    }

    #[test]
    fn selection_reversal() {
        let mut b = buf_with("abcdef");
        b.move_to(3);
        b.select_to(1);
        assert_eq!(b.sel(), 1..3);
        assert!(b.reversed());
        assert_eq!(b.cursor(), 1);
        b.select_to(5);
        assert_eq!(b.sel(), 3..5);
        assert!(!b.reversed());
        assert_eq!(b.cursor(), 5);
    }

    #[test]
    fn select_all_covers_buffer() {
        let mut b = buf_with("hello");
        b.select_all();
        assert_eq!(b.sel(), 0..5);
        assert!(!b.reversed());
    }

    #[test]
    fn utf16_bridging() {
        let b = buf_with("aé\u{1F600}b"); // "aé😀b": utf16 units 1+1+2+1 = 5
        assert_eq!(b.to_utf16(b.text().len()), 5);
        assert_eq!(b.offset_from_utf16(2), 3); // utf16 2 = before 😀
        assert_eq!(b.offset_from_utf16(4), 7); // utf16 4 = after 😀 / before b
        assert_eq!(b.offset_from_utf16(5), b.text().len());
        assert_eq!(b.offset_from_utf16(99), b.text().len());
        assert_eq!(b.range_from_utf16(&(1..4)), 1..7);
    }

    #[test]
    fn restore_cursor_clamps_to_char_boundary() {
        let mut b = buf_with("é"); // 2 bytes
        b.restore_cursor(1);
        assert_eq!(b.cursor(), 0);
        b.restore_cursor(99);
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn enter_continues_and_ends_lists() {
        let mut b = buf_with("- item");
        b.move_to(6);
        b.enter(0..6);
        assert_eq!(b.text(), "- item\n- ");

        let mut b = buf_with("- ");
        b.move_to(2);
        b.enter(0..2);
        assert_eq!(b.text(), "");

        let mut b = buf_with("abc");
        b.move_to(3);
        b.enter(0..3);
        assert_eq!(b.text(), "abc\n");
    }

    #[test]
    fn set_text_resets_state() {
        let mut b = buf_with("abc");
        b.move_to(1);
        b.insert("x");
        b.set_text("new".to_string());
        assert_eq!(b.sel(), 0..0);
        assert!(b.undo.is_empty());
        assert!(b.redo.is_empty());
    }
}
