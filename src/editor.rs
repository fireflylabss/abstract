//! Live markdown editor: one entity owns the buffer, selection, IME state and
//! undo; a custom element shapes each logical line at its own size (headings
//! render large) and conceals markdown syntax the selection is not touching.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::menu::{ContextMenuExt, PopupMenu};
use gpui_kit::*;

use crate::assets::{MONO, SANS};
use crate::attach::Incoming;
use crate::buffer::Buffer;
use crate::i18n::{Key, t};
use crate::md::{self, Analysis, Analyzer, Kind};
use crate::theme::Palette;

const MAX_COL: f32 = 700.;
const PAD_X: f32 = 48.;
const PAD_TOP: f32 = 28.;

actions!(
    live_editor,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        DocStart,
        DocEnd,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Enter,
        Escape,
        Tab,
        Undo,
        Redo,
        Bold,
        Italic,
        PastePlain,
        Strike,
        InlineCode,
        WikiLink,
        ExternalLink,
        Heading1,
        Heading2,
        Heading3,
        PlainText,
        BulletList,
        NumberedList,
        TaskList,
        Quote,
        CodeBlock,
        Divider,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let c = Some("LiveEditor");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("ctrl-left", WordLeft, c),
        KeyBinding::new("ctrl-right", WordRight, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("ctrl-home", DocStart, c),
        KeyBinding::new("ctrl-end", DocEnd, c),
        KeyBinding::new("cmd-up", DocStart, c),
        KeyBinding::new("cmd-down", DocEnd, c),
        KeyBinding::new("cmd-left", Home, c),
        KeyBinding::new("cmd-right", End, c),
        KeyBinding::new("cmd-shift-left", SelectHome, c),
        KeyBinding::new("cmd-shift-right", SelectEnd, c),
        KeyBinding::new("ctrl-a", SelectAll, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("ctrl-c", Copy, c),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("ctrl-x", Cut, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("ctrl-v", Paste, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("escape", Escape, c),
        KeyBinding::new("shift-enter", Enter, c),
        KeyBinding::new("tab", Tab, c),
        KeyBinding::new("ctrl-z", Undo, c),
        KeyBinding::new("cmd-z", Undo, c),
        KeyBinding::new("ctrl-shift-z", Redo, c),
        KeyBinding::new("cmd-shift-z", Redo, c),
        KeyBinding::new("ctrl-y", Redo, c),
        KeyBinding::new("ctrl-b", Bold, c),
        KeyBinding::new("cmd-b", Bold, c),
        KeyBinding::new("ctrl-i", Italic, c),
        KeyBinding::new("cmd-i", Italic, c),
        KeyBinding::new("ctrl-shift-v", PastePlain, c),
        KeyBinding::new("cmd-shift-v", PastePlain, c),
        KeyBinding::new("ctrl-shift-x", Strike, c),
        KeyBinding::new("cmd-shift-x", Strike, c),
        KeyBinding::new("ctrl-e", InlineCode, c),
        KeyBinding::new("cmd-e", InlineCode, c),
        KeyBinding::new("ctrl-k", ExternalLink, c),
        KeyBinding::new("cmd-k", ExternalLink, c),
    ]);
}

pub struct Changed;

/// Secondary-click (Ctrl/`Cmd`) on a `[[wiki-link]]` target.
pub struct OpenLink(pub String);

/// Navigation keys while the `[[…]]` autocomplete popup is open.
pub enum CompletionMove {
    Up,
    Down,
    Accept,
    Cancel,
}

pub struct CompletionKey(pub CompletionMove);

/// Images/files pasted or dropped: the app stores them and calls
/// `insert_text` with the markdown that references them.
pub struct Attach(pub Vec<Incoming>);

pub struct LiveEditor {
    focus: FocusHandle,
    buf: Buffer,
    analyzer: Analyzer,
    analysis: Analysis,
    scroll_y: f32,
    autoscroll: bool,
    goal_x: Option<f32>,
    selecting: bool,
    layout: Option<Layout>,
    /// Drawn caret position in content coordinates; glides toward the target.
    caret: Option<Point<f32>>,
    /// Wiki-link completion popup is open: arrows/enter/escape route to it.
    completing: bool,
    /// Folder of the open note and the space root: relative image sources
    /// resolve against them.
    note_dir: Option<PathBuf>,
    root: Option<PathBuf>,
}

impl EventEmitter<Changed> for LiveEditor {}
impl EventEmitter<OpenLink> for LiveEditor {}
impl EventEmitter<CompletionKey> for LiveEditor {}
impl EventEmitter<Attach> for LiveEditor {}

impl Focusable for LiveEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl LiveEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut analyzer = Analyzer::new();
        let analysis = analyzer.analyze("");
        Self {
            focus: cx.focus_handle(),
            buf: Buffer::new(),
            analyzer,
            analysis,
            scroll_y: 0.,
            autoscroll: false,
            goal_x: None,
            selecting: false,
            layout: None,
            caret: None,
            completing: false,
            note_dir: None,
            root: None,
        }
    }

    pub fn set_dirs(&mut self, note_dir: Option<PathBuf>, root: PathBuf, cx: &mut Context<Self>) {
        if self.note_dir != note_dir || self.root.as_ref() != Some(&root) {
            self.note_dir = note_dir;
            self.root = Some(root);
            cx.notify();
        }
    }

    pub fn selected_text(&self) -> &str {
        &self.buf.text()[self.buf.sel()]
    }

    /// Insert at the selection as one undoable edit, emitting `Changed`.
    pub fn insert_text(&mut self, s: &str, cx: &mut Context<Self>) {
        self.insert(s, cx);
    }

    fn image_candidates(&self, im: &md::ImageRef) -> Vec<PathBuf> {
        let Some(dir) = &self.note_dir else {
            return Vec::new();
        };
        let root = self.root.as_ref().unwrap_or(dir);
        crate::attach::candidates(&self.buf.text()[im.src.clone()], im.embed, dir, root)
    }

    fn paste(&mut self, plain: bool, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        if !plain {
            let mut files = Vec::new();
            let mut images = Vec::new();
            let mut text = false;
            for e in item.entries() {
                match e {
                    ClipboardEntry::ExternalPaths(p) => {
                        files.extend(p.paths().iter().cloned().map(Incoming::Path));
                    }
                    ClipboardEntry::Image(img) => images.push(Incoming::Image {
                        bytes: img.bytes.clone(),
                        ext: img.format.extension(),
                    }),
                    ClipboardEntry::String(s) => text |= !s.text.trim().is_empty(),
                }
            }
            // Copied files win; then text (rich copies often carry a bitmap
            // too); a bare bitmap (screenshot) becomes an attachment.
            if !files.is_empty() {
                cx.emit(Attach(files));
                return;
            }
            if !text && !images.is_empty() {
                images.truncate(1);
                cx.emit(Attach(images));
                return;
            }
        }
        if let Some(t) = item.text() {
            self.insert(&t.replace("\r\n", "\n"), cx);
        }
    }

    fn drop_paths(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if let Some(off) = self.offset_at(window.mouse_position()) {
            self.move_to(off, cx);
        }
        let items = paths.paths().iter().cloned().map(Incoming::Path).collect();
        cx.emit(Attach(items));
    }

    /// `[[sel]]` / `[sel](…)` with the caret where typing continues.
    fn link(&mut self, wiki: bool, cx: &mut Context<Self>) {
        let r = self.buf.sel();
        let inner = self.buf.text()[r.clone()].to_string();
        let (new, caret) = if wiki {
            (format!("[[{inner}]]"), r.start + 2 + inner.len())
        } else {
            (format!("[{inner}]()"), r.start + 3 + inner.len())
        };
        self.edit(r, &new, Some(caret..caret), cx);
    }

    /// Replace the block prefix (heading, list, quote) of every selected line.
    fn block_prefix(&mut self, prefix: &str, cx: &mut Context<Self>) {
        let sel = self.buf.sel();
        let a = &self.analysis;
        let first = a.line_of(sel.start);
        let last = a.line_of(sel.end);
        let range = a.lines[first].0.start..a.lines[last].0.end;
        let old = &self.buf.text()[range.clone()];
        let new = old
            .split('\n')
            .map(|l| md::set_block_prefix(l, prefix))
            .collect::<Vec<_>>()
            .join("\n");
        let select = if first == last && sel.is_empty() {
            let c = (sel.start + new.len()).saturating_sub(old.len());
            let c = c.clamp(range.start, range.start + new.len());
            c..c
        } else {
            range.start..range.start + new.len()
        };
        self.edit(range, &new, Some(select), cx);
    }

    /// `text` as a block of its own: a newline first unless at line start.
    fn insert_block(&mut self, text: &str, inner: Option<usize>, cx: &mut Context<Self>) {
        let r = self.buf.sel();
        let at_start = r.start == self.line_range(r.start).start;
        let lead = if at_start { "" } else { "\n" };
        let new = format!("{lead}{text}");
        let select = inner.map(|i| {
            let o = r.start + lead.len() + i;
            o..o
        });
        self.edit(r, &new, select, cx);
    }

    fn code_block(&mut self, cx: &mut Context<Self>) {
        let inner = self.selected_text().to_string();
        let text = format!("```\n{inner}\n```\n");
        self.insert_block(&text, Some(4 + inner.len()), cx);
    }

    fn right_click(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if let Some(off) = self.offset_at(ev.position) {
            let sel = self.buf.sel();
            if !(sel.start <= off && off <= sel.end && !sel.is_empty()) {
                self.move_to(off, cx);
            }
        }
    }

    fn context_menu(
        menu: PopupMenu,
        editor: &WeakEntity<Self>,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let Some(editor) = editor.upgrade() else {
            return menu;
        };
        let focus = editor.read(cx).focus.clone();
        let has_sel = !editor.read(cx).buf.sel().is_empty();
        let (f1, f2, f3) = (focus.clone(), focus.clone(), focus.clone());
        menu.action_context(focus)
            .menu_with_disabled(t(Key::Cut), Box::new(Cut), !has_sel)
            .menu_with_disabled(t(Key::Copy), Box::new(Copy), !has_sel)
            .menu(t(Key::Paste), Box::new(Paste))
            .menu(t(Key::PastePlain), Box::new(PastePlain))
            .menu(t(Key::SelectAll), Box::new(SelectAll))
            .separator()
            .menu(t(Key::AddWikiLink), Box::new(WikiLink))
            .menu(t(Key::AddLink), Box::new(ExternalLink))
            .menu_with_disabled(
                t(Key::SearchSelection),
                Box::new(crate::keymap::SearchSelection),
                !has_sel,
            )
            .separator()
            .submenu(t(Key::Format), window, cx, move |m, _, _| {
                m.action_context(f1.clone())
                    .menu(t(Key::Bold), Box::new(Bold))
                    .menu(t(Key::Italic), Box::new(Italic))
                    .menu(t(Key::Strikethrough), Box::new(Strike))
                    .menu(t(Key::InlineCode), Box::new(InlineCode))
            })
            .submenu(t(Key::Paragraph), window, cx, move |m, _, _| {
                m.action_context(f2.clone())
                    .menu(t(Key::Heading1), Box::new(Heading1))
                    .menu(t(Key::Heading2), Box::new(Heading2))
                    .menu(t(Key::Heading3), Box::new(Heading3))
                    .menu(t(Key::PlainText), Box::new(PlainText))
                    .separator()
                    .menu(t(Key::BulletList), Box::new(BulletList))
                    .menu(t(Key::NumberedList), Box::new(NumberedList))
                    .menu(t(Key::TaskList), Box::new(TaskList))
                    .menu(t(Key::Quote), Box::new(Quote))
            })
            .submenu(t(Key::Insert), window, cx, move |m, _, _| {
                m.action_context(f3.clone())
                    .menu(t(Key::InsertImage), Box::new(crate::keymap::InsertImage))
                    .menu(t(Key::CodeBlock), Box::new(CodeBlock))
                    .menu(t(Key::Divider), Box::new(Divider))
            })
    }

    pub fn text(&self) -> &str {
        self.buf.text()
    }

    /// Replace the whole buffer without emitting `Changed` (loading a note).
    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.buf.set_text(text);
        self.scroll_y = 0.;
        self.caret = None;
        self.analysis = self.analyzer.analyze(self.buf.text());
        cx.notify();
    }

    /// (cursor byte offset, scroll_y) for session restore.
    pub fn view_state(&self) -> (usize, f32) {
        (self.buf.cursor(), self.scroll_y)
    }

    /// Restore cursor + scroll after (re)loading text: collapsed selection
    /// clamped to a char boundary, no autoscroll jump.
    pub fn restore_view(&mut self, cursor: usize, scroll_y: f32, cx: &mut Context<Self>) {
        self.buf.restore_cursor(cursor);
        self.scroll_y = scroll_y.max(0.);
        self.caret = None;
        self.autoscroll = false;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn set_completing(&mut self, on: bool) {
        self.completing = on;
    }

    /// `[[prefix` immediately left of the cursor → `(prefix range, prefix)`.
    /// Aborted by `]]`, `|` or another `[` between the brackets and the caret.
    pub fn wiki_prefix(&self) -> Option<(Range<usize>, String)> {
        let c = self.buf.cursor();
        if !self.buf.sel().is_empty() {
            return None;
        }
        let line = self.line_range(c);
        let seg = &self.buf.text()[line.start..c];
        let i = seg.rfind("[[")? + line.start;
        let inner = &self.buf.text()[i + 2..c];
        if inner.contains("]]") || inner.contains('|') || inner.contains('[') {
            return None;
        }
        Some((i + 2..c, inner.to_string()))
    }

    /// Replace the completion prefix with `target]]`.
    pub fn complete_wiki(&mut self, range: Range<usize>, target: &str, cx: &mut Context<Self>) {
        self.edit(range, &format!("{target}]]"), None, cx);
    }

    /// Caret bottom-left in editor-element coordinates (popup anchor).
    pub fn caret_anchor(&self) -> Option<Point<f32>> {
        let l = self.layout.as_ref()?;
        let (p, lh) = l.position(self.buf.cursor())?;
        Some(point(p.x, p.y - l.scroll + lh))
    }

    // ── Editing ──────────────────────────────────────────────────────────

    fn edit(
        &mut self,
        range: Range<usize>,
        new: &str,
        select: Option<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        self.buf.edit(range, new, select);
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.analysis = self.analyzer.analyze(self.buf.text());
        self.autoscroll = true;
        self.goal_x = None;
        cx.emit(Changed);
        cx.notify();
    }

    fn insert(&mut self, s: &str, cx: &mut Context<Self>) {
        self.buf.insert(s);
        self.changed(cx);
    }

    fn restore(&mut self, from_undo: bool, cx: &mut Context<Self>) {
        if self.buf.restore(from_undo) {
            self.changed(cx);
        }
    }

    fn wrap(&mut self, marker: &str, cx: &mut Context<Self>) {
        self.buf.wrap(marker);
        self.changed(cx);
    }

    // ── Movement ─────────────────────────────────────────────────────────

    fn move_to(&mut self, off: usize, cx: &mut Context<Self>) {
        self.buf.move_to(off);
        self.after_move(cx);
    }

    fn select_to(&mut self, off: usize, cx: &mut Context<Self>) {
        self.buf.select_to(off);
        self.after_move(cx);
    }

    fn after_move(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        self.goal_x = None;
        cx.notify();
    }

    fn line_range(&self, off: usize) -> Range<usize> {
        self.analysis.lines[self.analysis.line_of(off)].0.clone()
    }

    fn vertical_target(&mut self, down: bool) -> usize {
        let c = self.buf.cursor();
        let Some(layout) = &self.layout else {
            return if down { self.buf.text().len() } else { 0 };
        };
        let Some((p, lh)) = layout.position(c) else {
            return c;
        };
        let x = *self.goal_x.get_or_insert(p.x);
        let y = if down { p.y + lh + 1. } else { p.y - 1. };
        if y < 0. {
            0
        } else if y > layout.content_h {
            self.buf.text().len()
        } else {
            layout.hit(x, y)
        }
    }

    fn vertical(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        let goal = self.goal_x;
        let target = self.vertical_target(down);
        let keep = self.goal_x.or(goal);
        if select {
            self.select_to(target, cx)
        } else {
            self.move_to(target, cx)
        }
        self.goal_x = keep;
    }

    // ── Pointer ──────────────────────────────────────────────────────────

    fn offset_at(&self, pos: Point<Pixels>) -> Option<usize> {
        let l = self.layout.as_ref()?;
        let x = f32::from(pos.x - l.bounds.left());
        let y = f32::from(pos.y - l.bounds.top()) + l.scroll;
        Some(if y < 0. {
            0
        } else if y > l.content_h {
            self.buf.text().len()
        } else {
            l.hit(x, y)
        })
    }

    fn mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Ctrl/Cmd+click opens the link instead of moving the caret.
        if ev.modifiers.secondary()
            && let Some(off) = self.offset_at(ev.position)
            && let Some(l) = self
                .analysis
                .wiki_links
                .iter()
                .find(|l| l.range.contains(&off))
        {
            cx.emit(OpenLink(self.buf.text()[l.target.clone()].to_string()));
            return;
        }
        // Click on a painted checkbox toggles the task whose marker is in that
        // line. (Only reachable while the marker is concealed — the check
        // bounds only exist then.)
        if ev.click_count == 1
            && !ev.modifiers.secondary()
            && let Some(l) = self.layout.as_ref()
        {
            let p = point(
                f32::from(ev.position.x - l.bounds.left()),
                f32::from(ev.position.y - l.bounds.top()) + l.scroll,
            );
            if let Some(line) = l
                .lines
                .iter()
                .find(|line| line.check.is_some_and(|c| c.contains(&p)))
                && let Some(t) = self
                    .analysis
                    .tasks
                    .iter()
                    .find(|t| line.buf.contains(&t.marker.start))
            {
                let cur = self.buf.cursor();
                self.buf.edit(
                    t.marker.clone(),
                    if t.checked { "[ ]" } else { "[x]" },
                    None,
                );
                self.buf.restore_cursor(cur);
                self.changed(cx);
                return;
            }
        }
        window.focus(&self.focus, cx);
        let Some(off) = self.offset_at(ev.position) else {
            return;
        };
        // Plain click on a `[ ]`/`[x]` marker toggles the task.
        if ev.click_count == 1
            && !ev.modifiers.shift
            && !ev.modifiers.alt
            && let Some(t) = self.analysis.tasks.iter().find(|t| t.marker.contains(&off))
        {
            let cur = self.buf.cursor();
            self.buf.edit(
                t.marker.clone(),
                if t.checked { "[ ]" } else { "[x]" },
                None,
            );
            self.buf.restore_cursor(cur);
            self.changed(cx);
            return;
        }
        self.selecting = true;
        if ev.modifiers.shift {
            self.select_to(off, cx);
        } else if ev.click_count == 2 {
            let (a, b) = (
                self.buf
                    .word_left(self.buf.next_boundary(off).min(self.buf.text().len())),
                self.buf.word_right(off),
            );
            self.buf.move_to(a.min(off));
            self.buf.select_to(b.max(off));
            self.after_move(cx);
        } else if ev.click_count >= 3 {
            let line = self.line_range(off);
            self.buf.move_to(line.start);
            self.buf.select_to(line.end);
            self.after_move(cx);
        } else {
            self.move_to(off, cx);
        }
    }

    fn mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting
            && ev.pressed_button == Some(MouseButton::Left)
            && let Some(off) = self.offset_at(ev.position)
        {
            self.select_to(off, cx);
        }
    }

    fn scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_y -= f32::from(ev.delta.pixel_delta(px(28.)).y);
        self.autoscroll = false;
        cx.notify();
    }
}

impl EntityInputHandler for LiveEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = self.buf.range_from_utf16(&range);
        actual.replace(self.buf.to_utf16(r.start)..self.buf.to_utf16(r.end));
        Some(self.buf.text()[r].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buf.to_utf16(self.buf.sel().start)..self.buf.to_utf16(self.buf.sel().end),
            reversed: self.buf.reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buf
            .marked()
            .map(|r| self.buf.to_utf16(r.start)..self.buf.to_utf16(r.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.buf.set_marked(None);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = range
            .map(|r| self.buf.range_from_utf16(&r))
            .or(self.buf.marked())
            .unwrap_or(self.buf.sel());
        self.edit(r, new, None, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        new_sel: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = range
            .map(|r| self.buf.range_from_utf16(&r))
            .or(self.buf.marked())
            .unwrap_or(self.buf.sel());
        self.edit(r.clone(), new, None, cx);
        self.buf
            .set_marked((!new.is_empty()).then(|| r.start..r.start + new.len()));
        if let Some(s) = new_sel {
            // `new_sel` is UTF-16 relative to the inserted text.
            let rel = |u: usize| {
                new.char_indices()
                    .scan(0, |n, (i, c)| {
                        let here = *n;
                        *n += c.len_utf16();
                        Some((here, i))
                    })
                    .find(|(n, _)| *n >= u)
                    .map_or(new.len(), |(_, i)| i)
            };
            self.buf
                .set_sel(r.start + rel(s.start)..r.start + rel(s.end));
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let off = self.buf.offset_from_utf16(range.start);
        let l = self.layout.as_ref()?;
        let (p, lh) = l.position(off)?;
        Some(Bounds::new(
            point(
                l.bounds.left() + px(p.x),
                l.bounds.top() + px(p.y - l.scroll),
            ),
            size(px(2.), px(lh)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.offset_at(p).map(|o| self.buf.to_utf16(o))
    }
}

impl Render for LiveEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("live-editor")
            .key_context("LiveEditor")
            .track_focus(&self.focus)
            .role(Role::MultilineTextInput)
            .aria_label(crate::i18n::t(crate::i18n::Key::EditorAria))
            .size_full()
            .cursor_text()
            .on_action(cx.listener(|this, _: &Backspace, _, cx| {
                this.buf.backspace();
                this.changed(cx);
            }))
            .on_action(cx.listener(|this, _: &Delete, _, cx| {
                this.buf.delete();
                this.changed(cx);
            }))
            .on_action(cx.listener(|this, _: &DeleteWordLeft, _, cx| {
                this.buf.delete_word_left();
                this.changed(cx);
            }))
            .on_action(cx.listener(|this, _: &Left, _, cx| {
                let t = if this.buf.sel().is_empty() {
                    this.buf.prev_boundary(this.buf.cursor())
                } else {
                    this.buf.sel().start
                };
                this.move_to(t, cx);
            }))
            .on_action(cx.listener(|this, _: &Right, _, cx| {
                let t = if this.buf.sel().is_empty() {
                    this.buf.next_boundary(this.buf.cursor())
                } else {
                    this.buf.sel().end
                };
                this.move_to(t, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| {
                this.select_to(this.buf.prev_boundary(this.buf.cursor()), cx)
            }))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| {
                this.select_to(this.buf.next_boundary(this.buf.cursor()), cx)
            }))
            .on_action(cx.listener(|this, _: &WordLeft, _, cx| {
                this.move_to(this.buf.word_left(this.buf.cursor()), cx)
            }))
            .on_action(cx.listener(|this, _: &WordRight, _, cx| {
                this.move_to(this.buf.word_right(this.buf.cursor()), cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordLeft, _, cx| {
                this.select_to(this.buf.word_left(this.buf.cursor()), cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordRight, _, cx| {
                this.select_to(this.buf.word_right(this.buf.cursor()), cx)
            }))
            .on_action(cx.listener(|this, _: &Up, _, cx| {
                if this.completing {
                    cx.emit(CompletionKey(CompletionMove::Up));
                } else {
                    this.vertical(false, false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Down, _, cx| {
                if this.completing {
                    cx.emit(CompletionKey(CompletionMove::Down));
                } else {
                    this.vertical(true, false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(false, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(true, true, cx)))
            .on_action(cx.listener(|this, _: &Home, _, cx| {
                this.move_to(this.line_range(this.buf.cursor()).start, cx)
            }))
            .on_action(cx.listener(|this, _: &End, _, cx| {
                this.move_to(this.line_range(this.buf.cursor()).end, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| {
                this.select_to(this.line_range(this.buf.cursor()).start, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| {
                this.select_to(this.line_range(this.buf.cursor()).end, cx)
            }))
            .on_action(cx.listener(|this, _: &DocStart, _, cx| this.move_to(0, cx)))
            .on_action(
                cx.listener(|this, _: &DocEnd, _, cx| this.move_to(this.buf.text().len(), cx)),
            )
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.buf.select_all();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if !this.buf.sel().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        this.buf.text()[this.buf.sel()].to_string(),
                    ));
                }
            }))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                if !this.buf.sel().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        this.buf.text()[this.buf.sel()].to_string(),
                    ));
                    this.insert("", cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.paste(false, cx)))
            .on_action(cx.listener(|this, _: &PastePlain, _, cx| this.paste(true, cx)))
            .on_action(cx.listener(|this, _: &Enter, _, cx| {
                if this.completing {
                    cx.emit(CompletionKey(CompletionMove::Accept));
                } else {
                    let line = this.line_range(this.buf.cursor());
                    this.buf.enter(line);
                    this.changed(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Escape, _, cx| {
                if this.completing {
                    cx.emit(CompletionKey(CompletionMove::Cancel));
                }
            }))
            .on_action(cx.listener(|this, _: &Tab, _, cx| this.insert("  ", cx)))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.restore(true, cx)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.restore(false, cx)))
            .on_action(cx.listener(|this, _: &Bold, _, cx| this.wrap("**", cx)))
            .on_action(cx.listener(|this, _: &Italic, _, cx| this.wrap("*", cx)))
            .on_action(cx.listener(|this, _: &Strike, _, cx| this.wrap("~~", cx)))
            .on_action(cx.listener(|this, _: &InlineCode, _, cx| this.wrap("`", cx)))
            .on_action(cx.listener(|this, _: &WikiLink, _, cx| this.link(true, cx)))
            .on_action(cx.listener(|this, _: &ExternalLink, _, cx| this.link(false, cx)))
            .on_action(cx.listener(|this, _: &Heading1, _, cx| this.block_prefix("# ", cx)))
            .on_action(cx.listener(|this, _: &Heading2, _, cx| this.block_prefix("## ", cx)))
            .on_action(cx.listener(|this, _: &Heading3, _, cx| this.block_prefix("### ", cx)))
            .on_action(cx.listener(|this, _: &PlainText, _, cx| this.block_prefix("", cx)))
            .on_action(cx.listener(|this, _: &BulletList, _, cx| this.block_prefix("- ", cx)))
            .on_action(cx.listener(|this, _: &NumberedList, _, cx| this.block_prefix("1. ", cx)))
            .on_action(cx.listener(|this, _: &TaskList, _, cx| this.block_prefix("- [ ] ", cx)))
            .on_action(cx.listener(|this, _: &Quote, _, cx| this.block_prefix("> ", cx)))
            .on_action(cx.listener(|this, _: &CodeBlock, _, cx| this.code_block(cx)))
            .on_action(cx.listener(|this, _: &Divider, _, cx| this.insert_block("---\n", None, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::right_click))
            .on_drop(cx.listener(Self::drop_paths))
            .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(rgb(cx.global::<Palette>().hover)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_scroll_wheel(cx.listener(Self::scroll))
            .child(EditorElement(cx.entity()))
            .context_menu({
                let editor = cx.entity().downgrade();
                move |menu, window, cx| Self::context_menu(menu, &editor, window, cx)
            })
    }
}

// ── Layout ───────────────────────────────────────────────────────────────

struct LineBox {
    buf: Range<usize>,
    /// Visible buffer sub-ranges, concatenated into the shaped text.
    segs: Vec<Range<usize>>,
    kind: Kind,
    x: f32,
    top: f32,
    lh: f32,
    rows_h: f32,
    pad_bottom: f32,
    wrapped: WrappedLine,
    /// List bullet to paint, when the marker is currently concealed.
    bullet: Option<(md::Bullet, u8)>,
    /// Task checkbox bounds in content coordinates (for hit-testing).
    check: Option<Bounds<f32>>,
    /// Rendered `![](…)` images below the line's text, content coordinates.
    images: Vec<(Arc<RenderImage>, Bounds<f32>)>,
}

impl LineBox {
    fn to_display(&self, off: usize) -> usize {
        let mut acc = 0;
        for s in &self.segs {
            if off < s.start {
                return acc;
            }
            if off <= s.end {
                return acc + off - s.start;
            }
            acc += s.len();
        }
        acc
    }

    fn to_buffer(&self, di: usize) -> usize {
        let mut acc = 0;
        for s in &self.segs {
            if di <= acc + s.len() {
                return s.start + di - acc;
            }
            acc += s.len();
        }
        self.segs.last().map_or(self.buf.end, |s| s.end)
    }

    fn bottom(&self) -> f32 {
        self.top + self.rows_h + self.pad_bottom
    }
}

pub struct Layout {
    bounds: Bounds<Pixels>,
    scroll: f32,
    col_x: f32,
    col_w: f32,
    content_h: f32,
    lines: Vec<LineBox>,
}

impl Layout {
    fn line_for(&self, off: usize) -> Option<&LineBox> {
        let ix = self
            .lines
            .partition_point(|l| l.buf.start <= off)
            .checked_sub(1)?;
        self.lines.get(ix)
    }

    /// Top-left of the caret slot for `off`, in content coordinates.
    fn position(&self, off: usize) -> Option<(Point<f32>, f32)> {
        let line = self.line_for(off)?;
        let di = line.to_display(off).min(line.wrapped.len());
        let p = line
            .wrapped
            .position_for_index(di, px(line.lh))
            .unwrap_or_default();
        Some((
            point(line.x + f32::from(p.x), line.top + f32::from(p.y)),
            line.lh,
        ))
    }

    fn hit(&self, x: f32, y: f32) -> usize {
        let ix = self
            .lines
            .partition_point(|l| l.bottom() <= y)
            .min(self.lines.len().saturating_sub(1));
        let Some(line) = self.lines.get(ix) else {
            return 0;
        };
        let local = point(
            px(x - line.x),
            px((y - line.top).clamp(0., (line.rows_h - 1.).max(0.))),
        );
        let di = line
            .wrapped
            .closest_index_for_position(local, px(line.lh))
            .unwrap_or_else(|i| i);
        line.to_buffer(di)
    }
}

const IMAGE_MAX_H: f32 = 520.;

/// First candidate that decodes; `None` while one is still loading (the
/// asset cache notifies the view when it lands) or when none exists.
fn load_image(
    candidates: &[PathBuf],
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    for p in candidates {
        let res = Resource::Path(Arc::from(p.as_path()));
        match window.use_asset::<ImgResourceLoader>(&res, cx) {
            Some(Ok(img)) => return Some(img),
            Some(Err(_)) => continue,
            None => return None,
        }
    }
    None
}

/// `segs` minus `hole`.
fn cut(segs: Vec<Range<usize>>, hole: &Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::with_capacity(segs.len());
    for s in segs {
        if s.end <= hole.start || s.start >= hole.end {
            out.push(s);
            continue;
        }
        if s.start < hole.start {
            out.push(s.start..hole.start);
        }
        if s.end > hole.end {
            out.push(hole.end..s.end);
        }
    }
    out
}

fn metrics(kind: Kind) -> (f32, f32, f32, f32) {
    // (font size, line height, space above, space below)
    match kind {
        Kind::Heading(1) => (32., 42., 22., 6.),
        Kind::Heading(2) => (25., 34., 18., 4.),
        Kind::Heading(3) => (20.5, 29., 12., 2.),
        Kind::Heading(_) => (17.5, 27., 8., 0.),
        Kind::Code => (14., 23., 0., 0.),
        _ => (16., 28., 0., 0.),
    }
}

fn hsla(c: u32) -> Hsla {
    rgb(c).into()
}

fn run(pal: &Palette, kind: Kind, flags: u16, len: usize) -> TextRun {
    let heading = matches!(kind, Kind::Heading(_));
    let code = kind == Kind::Code || flags & md::CODE != 0;
    let mut f = font(if code { MONO } else { SANS });
    if heading {
        f.weight = if matches!(kind, Kind::Heading(1 | 2)) {
            FontWeight::BOLD
        } else {
            FontWeight::SEMIBOLD
        };
    }
    if flags & md::BOLD != 0 {
        f.weight = FontWeight::BOLD;
    }
    if flags & md::ITALIC != 0 {
        f.style = FontStyle::Italic;
    }
    if kind == Kind::Code {
        if flags & md::KEYWORD != 0 {
            f.weight = FontWeight::SEMIBOLD;
        } else if flags & md::COMMENT != 0 {
            f.style = FontStyle::Italic;
        }
    }
    let color = if flags & md::MARK != 0 {
        pal.mark
    } else if flags & (md::MUTED | md::DONE) != 0 {
        pal.muted
    } else if kind == Kind::Code && flags & md::KEYWORD != 0 {
        pal.code_kw
    } else if kind == Kind::Code && flags & md::STRING != 0 {
        pal.code_str
    } else if kind == Kind::Code && flags & md::COMMENT != 0 {
        pal.code_comment
    } else if kind == Kind::Code && flags & md::NUMBER != 0 {
        pal.code_num
    } else if heading || flags & md::LINK != 0 {
        pal.head
    } else if kind == Kind::Quote {
        pal.quote
    } else {
        pal.body
    };
    let underline = (flags & (md::UNDERLINE | md::LINK) != 0 && flags & md::MARK == 0).then(|| {
        UnderlineStyle {
            thickness: px(1.),
            color: (flags & md::LINK != 0).then(|| hsla(pal.muted)),
            wavy: false,
        }
    });
    let strikethrough =
        (flags & (md::STRIKE | md::DONE) != 0 && flags & md::MARK == 0).then(|| {
            StrikethroughStyle {
                thickness: px(1.),
                color: None,
            }
        });
    let background_color =
        (flags & md::CODE != 0 && kind != Kind::Code).then(|| hsla(pal.inline_code_bg));
    TextRun {
        len,
        font: f,
        color: hsla(color),
        background_color,
        underline,
        strikethrough,
    }
}

pub struct EditorElement(Entity<LiveEditor>);

impl IntoElement for EditorElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Layout>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Layout> {
        let wanted: Vec<Vec<PathBuf>> = {
            let ed = self.0.read(cx);
            ed.analysis
                .images
                .iter()
                .map(|im| ed.image_candidates(im))
                .collect()
        };
        let loaded: Vec<Option<Arc<RenderImage>>> =
            wanted.iter().map(|c| load_image(c, window, cx)).collect();
        let ed = self.0.read(cx);
        let pal = *cx.global::<Palette>();
        let width = f32::from(bounds.size.width);
        let view_h = f32::from(bounds.size.height);
        let col_w = (width - PAD_X * 2.).clamp(120., MAX_COL);
        let col_x = ((width - col_w) / 2.).max(0.);
        // Unfocused: everything renders; focused: the selection reveals syntax.
        let reveal = if ed.focus.is_focused(window) {
            ed.buf.sel()
        } else {
            usize::MAX..usize::MAX
        };
        let text = ed.buf.text();
        let a = &ed.analysis;

        let mut lines = Vec::with_capacity(a.lines.len());
        let mut y = PAD_TOP;
        let mut display = String::new();
        let mut runs = Vec::new();
        let mut items = a.items.iter().peekable();
        for (ix, (buf, kind)) in a.lines.iter().enumerate() {
            let (fs, lh, above, below) = metrics(*kind);
            // A line can carry several markers (e.g. "- 1. x"); the
            // innermost item decides the indent and the painted bullet.
            let mut item = None;
            while matches!(items.peek(), Some(i) if i.line == ix) {
                item = items.next();
            }
            let mut indent = if *kind == Kind::Quote { 18. } else { 0. };
            if let Some(i) = item {
                indent += 22. * (i.depth as f32 + 1.);
            }
            display.clear();
            runs.clear();
            let line_images: Vec<(&md::ImageRef, &Arc<RenderImage>)> = a
                .images
                .iter()
                .zip(&loaded)
                .filter(|(im, _)| buf.start <= im.range.start && im.range.start <= buf.end)
                .filter_map(|(im, img)| img.as_ref().map(|img| (im, img)))
                .collect();
            let segs = if text.is_empty() && ix == 0 {
                let placeholder = crate::i18n::t(crate::i18n::Key::EditorPlaceholder);
                display.push_str(placeholder);
                runs.push(run(&pal, *kind, md::MARK, placeholder.len()));
                std::iter::once(0..0).collect()
            } else {
                let mut segs = a.visible(buf.clone(), &reveal);
                // A displayed image replaces its source until the selection
                // touches it.
                for (im, _) in &line_images {
                    if !(im.range.start <= reveal.end && reveal.start <= im.range.end) {
                        segs = cut(segs, &im.range);
                    }
                }
                if segs.is_empty() {
                    segs.push(buf.start..buf.start);
                }
                for s in &segs {
                    let mut i = s.start;
                    while i < s.end {
                        let f = a.flags[i];
                        let mut j = i + 1;
                        while j < s.end && a.flags[j] == f {
                            j += 1;
                        }
                        runs.push(run(&pal, *kind, f, j - i));
                        display.push_str(&text[i..j]);
                        i = j;
                    }
                }
                segs
            };
            // A concealed marker (selection outside the item's owner line)
            // shows as a painted bullet instead.
            let bullet = item.and_then(|i| {
                let hidden = !segs
                    .iter()
                    .any(|s| s.start < i.marker.end && i.marker.start < s.end);
                hidden.then_some((i.bullet, i.depth))
            });
            let wrapped = window
                .text_system()
                .shape_text(
                    SharedString::from(display.clone()),
                    px(fs),
                    &runs,
                    Some(px(col_w - indent)),
                    None,
                )
                .ok()
                .and_then(|mut v| v.pop())
                .unwrap_or_default();
            let top = y + above;
            let text_h = if display.is_empty() && !line_images.is_empty() {
                0.
            } else {
                (wrapped.wrap_boundaries.len() + 1) as f32 * lh
            };
            let mut images = Vec::new();
            let mut iy = top + text_h + if text_h > 0. { 6. } else { 0. };
            let max_w = col_w - indent;
            for (_, img) in &line_images {
                let s = img.size(0);
                let (w, h) = (s.width.0 as f32, s.height.0 as f32);
                if w <= 0. || h <= 0. {
                    continue;
                }
                let k = (max_w / w).min(IMAGE_MAX_H / h).min(1.);
                let b = Bounds {
                    origin: point(col_x + indent, iy),
                    size: size(w * k, h * k),
                };
                iy += b.size.height + 8.;
                images.push(((*img).clone(), b));
            }
            let rows_h = if images.is_empty() { text_h } else { iy - top };
            y = top + rows_h + below;
            lines.push(LineBox {
                buf: buf.clone(),
                segs,
                kind: *kind,
                x: col_x + indent,
                top,
                lh,
                rows_h,
                pad_bottom: below,
                wrapped,
                bullet,
                check: None,
                images,
            });
        }
        let content_h = y + PAD_TOP;
        let mut layout = Layout {
            bounds,
            scroll: ed.scroll_y,
            col_x,
            col_w,
            content_h,
            lines,
        };

        if ed.autoscroll
            && let Some((p, lh)) = layout.position(ed.buf.cursor())
        {
            let margin = (view_h * 0.15).min(80.);
            if p.y - layout.scroll < margin {
                layout.scroll = p.y - margin;
            } else if p.y + lh - layout.scroll > view_h - margin {
                layout.scroll = p.y + lh - view_h + margin;
            }
        }
        // Allow scrolling the last line up to mid-screen for comfortable writing.
        let max_scroll = (content_h - view_h * 0.5).max(0.);
        layout.scroll = layout.scroll.clamp(0., max_scroll);
        Some(layout)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        layout: &mut Option<Layout>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(mut layout) = layout.take() else {
            return;
        };
        let pal = *cx.global::<Palette>();
        let (focus, sel, cursor, caret) = {
            let ed = self.0.read(cx);
            (ed.focus.clone(), ed.buf.sel(), ed.buf.cursor(), ed.caret)
        };
        let focused = focus.is_focused(window);
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.0.clone()), cx);

        let sy = layout.scroll;
        let view_h = f32::from(bounds.size.height);
        let at = |x: f32, y: f32| point(bounds.left() + px(x), bounds.top() + px(y - sy));
        let first = layout.lines.partition_point(|l| l.bottom() < sy);

        // Caret target + glide (disabled under reduced motion).
        let target = layout.position(cursor);
        let mut next_caret = target.map(|(p, _)| p);
        if let (Some((t, lh)), Some(cur)) = (target, caret)
            && !cx.reduce_motion()
            && (t.y - cur.y).abs() < lh * 2.
        {
            let nx = cur.x + (t.x - cur.x) * 0.5;
            let ny = cur.y + (t.y - cur.y) * 0.5;
            if (t.x - nx).abs() > 0.5 || (t.y - ny).abs() > 0.5 {
                next_caret = Some(point(nx, ny));
                window.request_animation_frame();
            }
        }

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for line in &mut layout.lines[first..] {
                if line.top - sy > view_h {
                    break;
                }
                match line.kind {
                    Kind::Code => window.paint_quad(fill(
                        Bounds::new(
                            at(layout.col_x - 14., line.top),
                            size(px(layout.col_w + 28.), px(line.rows_h)),
                        ),
                        hsla(pal.code_bg),
                    )),
                    Kind::Quote => window.paint_quad(fill(
                        Bounds::new(
                            at(layout.col_x + 2., line.top),
                            size(px(2.), px(line.rows_h)),
                        ),
                        hsla(pal.rule),
                    )),
                    Kind::Rule if line.wrapped.len() == 0 => window.paint_quad(fill(
                        Bounds::new(
                            at(layout.col_x, line.top + line.lh / 2.),
                            size(px(layout.col_w), px(1.)),
                        ),
                        hsla(pal.rule),
                    )),
                    _ => {}
                }

                // Selection, including a sliver for the selected line break.
                let s = sel.start.max(line.buf.start);
                let e = sel.end.min(line.buf.end);
                let spans_break = sel.end > line.buf.end && sel.start <= line.buf.end;
                if s < e || spans_break {
                    let pos = |off: usize| {
                        let di = line.to_display(off).min(line.wrapped.len());
                        line.wrapped
                            .position_for_index(di, px(line.lh))
                            .unwrap_or_default()
                    };
                    let (ps, pe) = (pos(s), pos(e));
                    let (r0, r1) = (
                        (f32::from(ps.y) / line.lh).round() as usize,
                        (f32::from(pe.y) / line.lh).round() as usize,
                    );
                    let full = layout.col_w - (line.x - layout.col_x);
                    for r in r0..=r1 {
                        let x0 = if r == r0 { f32::from(ps.x) } else { 0. };
                        let mut x1 = if r == r1 { f32::from(pe.x) } else { full };
                        if r == r1 && spans_break {
                            x1 += 6.;
                        }
                        window.paint_quad(fill(
                            Bounds::new(
                                at(line.x + x0, line.top + r as f32 * line.lh),
                                size(px((x1 - x0).max(0.)), px(line.lh)),
                            ),
                            rgba(pal.selection),
                        ));
                    }
                }

                // List bullets where the marker is concealed.
                if let Some((bullet, depth)) = line.bullet {
                    match bullet {
                        md::Bullet::Dot => {
                            let cxp = line.x - 13.;
                            let cyp = line.top + line.lh / 2.;
                            let filled = depth == 0;
                            window.paint_quad(quad(
                                Bounds::new(at(cxp - 2.5, cyp - 2.5), size(px(5.), px(5.))),
                                px(2.5),
                                if filled {
                                    hsla(pal.dim)
                                } else {
                                    transparent_black()
                                },
                                if filled { px(0.) } else { px(1.5) },
                                hsla(pal.dim),
                                BorderStyle::Solid,
                            ));
                        }
                        md::Bullet::Task { checked } => {
                            let sq = Bounds {
                                origin: point(line.x - 21., line.top + line.lh / 2. - 7.5),
                                size: size(15., 15.),
                            };
                            line.check = Some(sq);
                            let px_sq =
                                Bounds::new(at(sq.origin.x, sq.origin.y), size(px(15.), px(15.)));
                            if checked {
                                window.paint_quad(quad(
                                    px_sq,
                                    px(4.),
                                    hsla(pal.head),
                                    px(0.),
                                    transparent_black(),
                                    BorderStyle::Solid,
                                ));
                                window
                                    .paint_svg(
                                        Bounds::new(
                                            at(sq.origin.x + 2., sq.origin.y + 2.),
                                            size(px(11.), px(11.)),
                                        ),
                                        "icons/check.svg".into(),
                                        None,
                                        TransformationMatrix::unit(),
                                        hsla(pal.bg),
                                        cx,
                                    )
                                    .ok();
                            } else {
                                window.paint_quad(quad(
                                    px_sq,
                                    px(4.),
                                    transparent_black(),
                                    px(1.5),
                                    hsla(pal.muted),
                                    BorderStyle::Solid,
                                ));
                            }
                        }
                        md::Bullet::Ordered => {}
                    }
                }

                // `paint` draws glyphs/decorations only; backgrounds (inline
                // code) are a separate pass underneath.
                let origin = at(line.x, line.top);
                line.wrapped
                    .paint_background(origin, px(line.lh), TextAlign::Left, None, window, cx)
                    .ok();
                line.wrapped
                    .paint(origin, px(line.lh), TextAlign::Left, None, window, cx)
                    .ok();
                for (img, b) in &line.images {
                    let r = Bounds::new(
                        at(b.origin.x, b.origin.y),
                        size(px(b.size.width), px(b.size.height)),
                    );
                    window
                        .paint_image(r, r, Corners::all(px(6.)), img.clone(), 0, false)
                        .ok();
                }
            }

            if focused && let (Some(p), Some((_, lh))) = (next_caret, target) {
                let h = (lh * 0.72).max(16.);
                window.paint_quad(fill(
                    Bounds::new(at(p.x, p.y + (lh - h) / 2.), size(px(2.), px(h))),
                    hsla(pal.caret),
                ));
            }
        });

        self.0.update(cx, |ed, _| {
            ed.scroll_y = layout.scroll;
            ed.autoscroll = false;
            ed.caret = next_caret;
            ed.layout = Some(layout);
        });
    }
}
