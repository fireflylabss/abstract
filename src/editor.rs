//! Live markdown editor: one entity owns the buffer, selection, IME state and
//! undo; a custom element shapes each logical line at its own size (headings
//! render large) and conceals markdown syntax the selection is not touching.

use std::ops::Range;

use gpui_kit::*;

use crate::buffer::Buffer;
use crate::md::{self, Analysis, Analyzer, Kind};
use crate::theme::Palette;

const SANS: &str = "Noto Sans";
const MONO: &str = "Noto Sans Mono";
const MAX_COL: f32 = 700.;
const PAD_X: f32 = 48.;
const PAD_TOP: f32 = 28.;
const PLACEHOLDER: &str = "Comece a escrever…";

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
        Tab,
        Undo,
        Redo,
        Bold,
        Italic,
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
    ]);
}

pub struct Changed;

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
}

impl EventEmitter<Changed> for LiveEditor {}

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
        }
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
        window.focus(&self.focus, cx);
        let Some(off) = self.offset_at(ev.position) else {
            return;
        };
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
            .aria_label("Editor Markdown")
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
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(false, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(true, false, cx)))
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
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    this.insert(&t.replace("\r\n", "\n"), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Enter, _, cx| {
                let line = this.line_range(this.buf.cursor());
                this.buf.enter(line);
                this.changed(cx);
            }))
            .on_action(cx.listener(|this, _: &Tab, _, cx| this.insert("  ", cx)))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.restore(true, cx)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.restore(false, cx)))
            .on_action(cx.listener(|this, _: &Bold, _, cx| this.wrap("**", cx)))
            .on_action(cx.listener(|this, _: &Italic, _, cx| this.wrap("*", cx)))
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
    let color = if flags & md::MARK != 0 {
        pal.mark
    } else if flags & md::MUTED != 0 {
        pal.muted
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
        (flags & md::STRIKE != 0 && flags & md::MARK == 0).then(|| StrikethroughStyle {
            thickness: px(1.),
            color: None,
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
        for (ix, (buf, kind)) in a.lines.iter().enumerate() {
            let (fs, lh, above, below) = metrics(*kind);
            let indent = if *kind == Kind::Quote { 18. } else { 0. };
            display.clear();
            runs.clear();
            let segs = if text.is_empty() && ix == 0 {
                display.push_str(PLACEHOLDER);
                runs.push(run(&pal, *kind, md::MARK, PLACEHOLDER.len()));
                std::iter::once(0..0).collect()
            } else {
                let segs = a.visible(buf.clone(), &reveal);
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
            let rows_h = (wrapped.wrap_boundaries.len() + 1) as f32 * lh;
            let top = y + above;
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
        let Some(layout) = layout.take() else { return };
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
            for line in &layout.lines[first..] {
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

                // `paint` draws glyphs/decorations only; backgrounds (inline
                // code) are a separate pass underneath.
                let origin = at(line.x, line.top);
                line.wrapped
                    .paint_background(origin, px(line.lh), TextAlign::Left, None, window, cx)
                    .ok();
                line.wrapped
                    .paint(origin, px(line.lh), TextAlign::Left, None, window, cx)
                    .ok();
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
