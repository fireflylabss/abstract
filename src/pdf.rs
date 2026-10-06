//! Markdown → PDF for "Export as PDF…".
//!
//! Rendering is pure Rust: `genpdf` (over `printpdf`) lays out the document —
//! no browser, no `wkhtmltopdf`, nothing that might be missing on the user's
//! machine. Blocks come from a fresh tree-sitter parse and inline styling
//! reuses the editor's `md::Analysis` plus `html.rs`'s helpers, so both
//! exports walk the same document; the app's bundled Noto fonts are embedded
//! in the file.
//!
//! Anything the format can't express degrades to styled or plain text —
//! links print colored but aren't clickable, `==highlight==` colors the text
//! instead of painting a background, and HTML blocks print literally.
//! Failures return `Err`; the caller writes the file only on success, so a
//! broken export never leaves a corrupt PDF behind.

use std::ops::Range;
use std::path::{Path, PathBuf};

use genpdf::elements::{
    Break, FrameCellDecorator, Image, LinearLayout, PaddedElement, Paragraph, TableLayout, Text,
};
use genpdf::error::{Error, ErrorKind};
use genpdf::fonts::{Font, FontData, FontFamily};
use genpdf::render::Area;
use genpdf::style::{Color, Style, StyledString};
use genpdf::{Alignment, Element, Margins, Mm, PageDecorator, Position, RenderResult, Scale, Size};
use tree_sitter::{Node, Parser};

use crate::attach;
use crate::html::{Context, STYLE, at, autolink, clip, comments, heading_level, trim};
use crate::md::{self, Analysis, Analyzer};

const FONT_SANS: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const FONT_SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
const FONT_SANS_ITALIC: &[u8] = include_bytes!("../assets/fonts/NotoSans-Italic.ttf");
// Noto Sans has no bold-italic face — SemiBold Italic is the closest.
const FONT_SANS_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/NotoSans-SemiBoldItalic.ttf");
const FONT_MONO: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf");
const FONT_MONO_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf");

const INK: Color = Color::Rgb(29, 29, 31);
const MUTED: Color = Color::Rgb(85, 85, 90);
const FAINT: Color = Color::Rgb(130, 130, 136);
const LINK: Color = Color::Rgb(55, 105, 190);
const CODE_INK: Color = Color::Rgb(40, 46, 54);
/// Rules, quote bars, table grid and code-frame borders.
const LINE: Color = Color::Rgb(208, 208, 214);

const BODY_PT: u8 = 10;
const CODE_PT: u8 = 9;
const CELL_PT: u8 = 9;
const FOOTNOTE_PT: u8 = 9;
const FOOTNOTE_REF_PT: u8 = 7;
const HEADING_PT: [u8; 6] = [20, 16, 13, 12, 11, 10];

/// Gap between blocks, in body line-heights.
const BLOCK_GAP: f64 = 0.7;
/// Extra air above a heading.
const HEAD_GAP: f64 = 0.5;
/// Text column width: A4 (210 mm) minus the page margins.
const TEXT_W: f64 = 166.0;
/// Images never grow beyond the text column or past ~70% of a page.
const MAX_IMG_H: f64 = 200.0;
/// Hard-wrap inside code blocks — Noto Sans Mono at 9 pt is ~1.9 mm per char,
/// so ~82 chars fit the padded frame.
const CODE_COLS: usize = 82;
/// Left inset for list items and quote bars.
const INDENT: f64 = 6.0;
/// Space between a bullet's right edge and the item text.
const BULLET_SPACE: f64 = 2.0;
/// Page number position, measured up from the bottom edge.
const FOOTER_Y: f64 = 12.0;

fn mm(v: f64) -> Mm {
    Mm::from(v as f32)
}

fn line_style() -> Style {
    Style::new().with_color(LINE)
}

/// `==…==` has no background fill in genpdf — tints print as text color.
fn tint_color(tint: Option<md::Tint>) -> Color {
    match tint.unwrap_or(md::Tint::Yellow) {
        md::Tint::Red => Color::Rgb(185, 60, 52),
        md::Tint::Orange => Color::Rgb(184, 96, 0),
        md::Tint::Yellow => Color::Rgb(140, 108, 0),
        md::Tint::Green => Color::Rgb(30, 140, 70),
        md::Tint::Blue => Color::Rgb(42, 95, 190),
        md::Tint::Purple => Color::Rgb(115, 70, 190),
    }
}

fn tone_color(tone: md::Tone) -> Color {
    match tone {
        md::Tone::Note => Color::Rgb(74, 144, 217),
        md::Tone::Tip => Color::Rgb(63, 163, 77),
        md::Tone::Warning => Color::Rgb(207, 142, 0),
        md::Tone::Danger => Color::Rgb(214, 69, 69),
    }
}

/// The whole note as `.pdf` bytes (the caller writes them out).
pub(crate) fn export(text: &str, title: &str, ctx: &Context) -> Result<Vec<u8>, Error> {
    let build = || -> Result<Vec<u8>, Error> {
        let mut doc = genpdf::Document::new(FontFamily {
            regular: FontData::new(FONT_SANS.to_vec(), None)?,
            bold: FontData::new(FONT_SANS_BOLD.to_vec(), None)?,
            italic: FontData::new(FONT_SANS_ITALIC.to_vec(), None)?,
            bold_italic: FontData::new(FONT_SANS_BOLD_ITALIC.to_vec(), None)?,
        });
        doc.set_title(title);
        doc.set_font_size(BODY_PT);
        doc.set_line_spacing(1.35);
        doc.set_paper_size(genpdf::PaperSize::A4);
        let mono = doc.add_font_family(FontFamily {
            regular: FontData::new(FONT_MONO.to_vec(), None)?,
            bold: FontData::new(FONT_MONO_BOLD.to_vec(), None)?,
            italic: FontData::new(FONT_MONO.to_vec(), None)?,
            bold_italic: FontData::new(FONT_MONO_BOLD.to_vec(), None)?,
        });
        doc.set_page_decorator(Footer::default());
        let a = Analyzer::new().analyze(text);
        let mut body = LinearLayout::vertical();
        Emitter::new(text, &a, 0..text.len(), ctx, mono).render(&mut body);
        doc.push(body);
        let mut out = Vec::new();
        doc.render(&mut out)?;
        Ok(out)
    };
    // Debug builds may still panic (release aborts) — never propagate one.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(build))
        .unwrap_or_else(|_| Err(Error::new("PDF export panicked", ErrorKind::Internal)))
}

/// Margins plus a centered page number.
#[derive(Default)]
struct Footer {
    page: usize,
}

impl PageDecorator for Footer {
    fn decorate_page<'a>(
        &mut self,
        context: &genpdf::Context,
        mut area: Area<'a>,
        style: Style,
    ) -> Result<Area<'a>, Error> {
        self.page += 1;
        let num = self.page.to_string();
        let s = style.and(Style::new().with_color(FAINT).with_font_size(8));
        let w = s.str_width(&context.font_cache, &num);
        let size = area.size();
        let _ = area.print_str(
            &context.font_cache,
            Position::new((size.width - w) / 2.0, size.height - mm(FOOTER_Y)),
            s,
            num,
        )?;
        area.add_margins(Margins::trbl(18., 22., 20., 22.));
        Ok(area)
    }
}

/// Full-width rule (`---` and the footnote separator).
struct Rule;

impl Element for Rule {
    fn render(
        &mut self,
        _context: &genpdf::Context,
        area: Area<'_>,
        style: Style,
    ) -> Result<RenderResult, Error> {
        let w = area.size().width;
        area.draw_line(
            vec![Position::new(mm(0.0), mm(0.5)), Position::new(w, mm(0.5))],
            style,
        );
        Ok(RenderResult {
            size: Size::new(w, mm(1.0)),
            has_more: false,
        })
    }
}

/// Children with a colored bar on the left — block quotes and callouts.
struct Bar<E: Element> {
    inner: E,
}

impl<E: Element> Bar<E> {
    fn new(inner: E) -> Self {
        Self { inner }
    }
}

impl<E: Element> Element for Bar<E> {
    fn render(
        &mut self,
        context: &genpdf::Context,
        area: Area<'_>,
        style: Style,
    ) -> Result<RenderResult, Error> {
        let width = area.size().width;
        let mut inner_area = area.clone();
        inner_area.add_offset(Position::new(mm(INDENT), mm(0.0)));
        let mut result = self.inner.render(context, inner_area, style)?;
        if result.size.height > mm(0.0) {
            area.draw_line(
                vec![
                    Position::new(mm(1.2), mm(0.0)),
                    Position::new(mm(1.2), result.size.height),
                ],
                style,
            );
        }
        result.size.width = width;
        Ok(result)
    }
}

/// A list row: `bullet` drawn left of `inner` in `bullet_style` (the ☐/■
/// task glyphs only exist in the monospace face).
struct Bullet<E: Element> {
    inner: E,
    bullet: String,
    bullet_style: Style,
    rendered: bool,
}

impl<E: Element> Bullet<E> {
    fn new(inner: E, bullet: String, bullet_style: Style) -> Self {
        Self {
            inner,
            bullet,
            bullet_style,
            rendered: false,
        }
    }
}

impl<E: Element> Element for Bullet<E> {
    fn render(
        &mut self,
        context: &genpdf::Context,
        area: Area<'_>,
        style: Style,
    ) -> Result<RenderResult, Error> {
        let mut inner_area = area.clone();
        inner_area.add_offset(Position::new(mm(INDENT), mm(0.0)));
        let mut result = self.inner.render(context, inner_area, style)?;
        result.size.width += mm(INDENT);
        if !self.rendered {
            let bullet_width = self
                .bullet_style
                .str_width(&context.font_cache, &self.bullet);
            let x = (mm(INDENT) - bullet_width - mm(BULLET_SPACE)).max(mm(0.0));
            area.print_str(
                &context.font_cache,
                Position::new(x, mm(0.0)),
                self.bullet_style,
                self.bullet.as_str(),
            )?;
            self.rendered = true;
        }
        Ok(result)
    }
}

/// Styled text accumulated for one wrapping unit (paragraph, heading, table
/// cell). Images can't live inside a paragraph: a block-level `flow` flushes
/// `runs` into a `Paragraph` and pushes the image into `layout`; contexts
/// without one degrade the image to its alt text.
struct Flow<'x> {
    runs: Vec<StyledString>,
    layout: Option<&'x mut LinearLayout>,
}

impl Flow<'_> {
    /// A run collector with no block sink (heading, cell, callout title).
    fn runs() -> Flow<'static> {
        Flow {
            runs: Vec::new(),
            layout: None,
        }
    }

    fn run(&mut self, mut s: StyledString) {
        // Soft line breaks stay literal in the source clip and genpdf draws
        // them as missing-glyph boxes — swap them for a space, like HTML.
        if s.s.contains('\n') {
            s.s = s.s.replace('\n', " ");
        }
        if !s.s.is_empty() {
            self.runs.push(s);
        }
    }

    /// Ends the paragraph under construction (called before a block element).
    fn flush(&mut self) {
        if let Some(layout) = self.layout.as_mut()
            && !self.runs.is_empty()
        {
            layout.push(Paragraph::from(std::mem::take(&mut self.runs)));
        }
    }

    fn element(&mut self, el: impl Element + 'static) {
        self.flush();
        if let Some(layout) = self.layout.as_mut() {
            layout.push(el);
        }
    }
}

/// The same walk `html.rs` does — blocks to `genpdf` elements, inline flags
/// to `StyledString` runs.
struct Emitter<'a> {
    text: &'a str,
    a: &'a Analysis,
    ctx: &'a Context<'a>,
    sel: Range<usize>,
    /// Concealed syntax + `%%…%%` spans, merged and sorted.
    hidden: Vec<Range<usize>>,
    /// Footnote definitions: `(label, body)` ranges, in document order.
    defs: Vec<(Range<usize>, Range<usize>)>,
    /// `[label]: dest` definitions for reference-style links.
    link_defs: Vec<(String, String)>,
    /// Monospace face for `code` and task bullets.
    mono: FontFamily<Font>,
    /// Body text color — gray inside plain block quotes.
    ink: Color,
    /// Inside a link run (links print colored; they aren't clickable).
    link_depth: usize,
}

impl<'a> Emitter<'a> {
    fn new(
        text: &'a str,
        a: &'a Analysis,
        sel: Range<usize>,
        ctx: &'a Context<'a>,
        mono: FontFamily<Font>,
    ) -> Self {
        let mut hidden: Vec<Range<usize>> = a.conceals.iter().map(|c| c.hidden.clone()).collect();
        hidden.extend(comments(text, a));
        hidden.sort_by_key(|r| r.start);
        Self {
            text,
            a,
            ctx,
            sel,
            hidden,
            defs: Vec::new(),
            link_defs: link_defs(text),
            mono,
            ink: INK,
            link_depth: 0,
        }
    }

    fn render(&mut self, out: &mut LinearLayout) {
        // tree-sitter-md errors on a last line without a line ending.
        let owned;
        let source: &str = if self.text.ends_with('\n') {
            self.text
        } else {
            owned = format!("{}\n", self.text);
            &owned
        };
        let mut parser = Parser::new();
        if parser
            .set_language(&tree_sitter_md::LANGUAGE.into())
            .is_err()
        {
            return;
        }
        if let Some(tree) = parser.parse(source, None) {
            self.children(out, tree.root_node());
        }
        self.footnote_section(out);
    }

    /// Block children with a `Break` between them (not after the last one).
    fn children(&mut self, out: &mut LinearLayout, node: Node) {
        let mut first = true;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let len = self.text.len();
            if clip(
                child.start_byte().min(len)..child.end_byte().min(len),
                &self.sel,
            )
            .is_empty()
            {
                continue;
            }
            if first {
                first = false;
            } else {
                out.push(Break::new(BLOCK_GAP));
            }
            self.block(out, child);
        }
    }

    fn block(&mut self, out: &mut LinearLayout, node: Node) {
        let len = self.text.len();
        let nb = clip(
            node.start_byte().min(len)..node.end_byte().min(len),
            &self.sel,
        );
        if nb.is_empty() {
            return;
        }
        match node.kind() {
            "document" | "section" => self.children(out, node),
            "atx_heading" | "setext_heading" => {
                self.heading(out, node, heading_level(node));
            }
            "paragraph" => {
                if self.def_bodies(node, &nb) {
                    return;
                }
                self.paragraph(out, node);
            }
            "block_quote" => self.quote(out, node),
            "list" => self.list(out, node),
            "list_item" => self.list_item(out, node, None),
            "thematic_break" => out.push(Rule.styled(line_style())),
            "fenced_code_block" => self.code(out, node, true),
            "indented_code_block" => self.code(out, node, false),
            "pipe_table" => self.table(out, node),
            "link_reference_definition" => {
                self.def_bodies(node, &nb);
            }
            "minus_metadata" | "plus_metadata" => {}
            "html_block" => out.push(Paragraph::new(StyledString::new(
                self.text.get(nb).unwrap_or_default(),
                Style::new().with_color(self.ink),
            ))),
            _ => self.children(out, node),
        }
    }

    /// The `inline` children of a node (paragraph, heading).
    fn inline_nodes(&mut self, flow: &mut Flow, node: Node) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "inline" {
                let len = self.text.len();
                self.inline(
                    flow,
                    clip(
                        child.start_byte().min(len)..child.end_byte().min(len),
                        &self.sel,
                    ),
                );
            }
        }
    }

    fn heading(&mut self, out: &mut LinearLayout, node: Node, level: u8) {
        let mut flow = Flow::runs();
        self.inline_nodes(&mut flow, node);
        if flow.runs.is_empty() {
            return;
        }
        let size = HEADING_PT[level.saturating_sub(1).min(5) as usize];
        out.push(Break::new(HEAD_GAP));
        out.push(Paragraph::from(
            flow.runs
                .into_iter()
                .map(|s| {
                    StyledString::new(s.s, s.style.and(Style::new().with_font_size(size).bold()))
                })
                .collect::<Vec<_>>(),
        ));
    }

    fn paragraph(&mut self, out: &mut LinearLayout, node: Node) {
        let mut flow = Flow {
            runs: Vec::new(),
            layout: Some(out),
        };
        self.inline_nodes(&mut flow, node);
        flow.flush();
    }

    fn quote(&mut self, out: &mut LinearLayout, node: Node) {
        let len = self.text.len();
        let first = self.a.line_of(node.start_byte().min(len));
        let Some(callout) = self.a.callouts.iter().find(|c| c.lines.start == first) else {
            let mut inner = LinearLayout::vertical();
            let ink = std::mem::replace(&mut self.ink, MUTED);
            self.children(&mut inner, node);
            self.ink = ink;
            out.push(Bar::new(inner).styled(line_style()));
            return;
        };
        let tone = tone_color(callout.tone);
        // The title is the first line of the first paragraph; the same
        // paragraph node continues into the callout body.
        let title_end = self
            .a
            .lines
            .get(callout.lines.start)
            .map(|l| l.0.end)
            .unwrap_or(len);
        let mut inner = LinearLayout::vertical();
        let mut titled = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !titled && child.kind() == "paragraph" {
                titled = true;
                let mut c = child.walk();
                for inner_node in child.children(&mut c) {
                    if inner_node.kind() != "inline" {
                        continue;
                    }
                    let r = inner_node.start_byte().min(len)..inner_node.end_byte().min(len);
                    let mut tf = Flow::runs();
                    self.inline(&mut tf, clip(r.start..r.end.min(title_end), &self.sel));
                    let title: Vec<StyledString> = tf
                        .runs
                        .into_iter()
                        .map(|s| {
                            StyledString::new(
                                s.s,
                                s.style.and(Style::new().with_color(tone).bold()),
                            )
                        })
                        .collect();
                    if !title.is_empty() {
                        inner.push(Paragraph::from(title));
                        inner.push(Break::new(0.3));
                    }
                    let mut bf = Flow {
                        runs: Vec::new(),
                        layout: Some(&mut inner),
                    };
                    self.inline(&mut bf, trim(self.text, clip(title_end..r.end, &self.sel)));
                    bf.flush();
                }
                continue;
            }
            self.block(&mut inner, child);
        }
        out.push(Bar::new(inner).styled(Style::new().with_color(tone)));
    }

    fn list(&mut self, out: &mut LinearLayout, node: Node) {
        let mut cursor = node.walk();
        let ordered = node.children(&mut cursor).any(|item| {
            let mut c = item.walk();
            item.children(&mut c)
                .any(|k| matches!(k.kind(), "list_marker_dot" | "list_marker_parenthesis"))
        });
        let mut n = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "list_item" {
                n += 1;
                self.list_item(out, child, ordered.then_some(n));
            } else {
                self.block(out, child);
            }
        }
    }

    fn list_item(&mut self, out: &mut LinearLayout, node: Node, num: Option<usize>) {
        let mut cursor = node.walk();
        let kids: Vec<Node> = node.children(&mut cursor).collect();
        let checked = kids.iter().find_map(|k| match k.kind() {
            "task_list_marker_checked" => Some(true),
            "task_list_marker_unchecked" => Some(false),
            _ => None,
        });
        let mut inner = LinearLayout::vertical();
        let mut first_para = true;
        for child in kids {
            match child.kind() {
                "paragraph" => {
                    if first_para {
                        first_para = false;
                    } else {
                        inner.push(Break::new(0.3));
                    }
                    let mut flow = Flow {
                        runs: Vec::new(),
                        layout: Some(&mut inner),
                    };
                    self.inline_nodes(&mut flow, child);
                    flow.flush();
                }
                "task_list_marker_checked" | "task_list_marker_unchecked" => {}
                _ => self.block(&mut inner, child),
            }
        }
        // ☐/■ live only in Noto Sans Mono — the bullet takes its own style.
        let (bullet, style) = match (checked, num) {
            (Some(true), _) => ("\u{25A0}".to_string(), self.mono_style()),
            (Some(false), _) => ("\u{25A1}".to_string(), self.mono_style()),
            (None, Some(n)) => (format!("{n}."), Style::new().with_color(self.ink)),
            (None, None) => ("\u{2022}".to_string(), Style::new().with_color(self.ink)),
        };
        out.push(Bullet::new(inner, bullet, style));
    }

    fn code(&mut self, out: &mut LinearLayout, node: Node, fenced: bool) {
        let len = self.text.len();
        let mut content = if fenced {
            None
        } else {
            Some(clip(
                node.start_byte().min(len)..node.end_byte().min(len),
                &self.sel,
            ))
        };
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "code_fence_content" {
                let r = child.start_byte().min(len)..child.end_byte().min(len);
                content = Some(clip(r, &self.sel));
            }
        }
        let mut rows: Vec<String> = Vec::new();
        if let Some(r) = content
            && let Some(s) = self.text.get(r)
        {
            for line in s.split('\n') {
                let line = if fenced {
                    line
                } else {
                    let indent = line.len() - line.trim_start_matches(' ').len();
                    &line[indent.min(4)..]
                };
                wrap_code(&mut rows, line);
            }
        }
        while matches!(rows.last(), Some(l) if l.is_empty()) {
            rows.pop();
        }
        let mut lines = LinearLayout::vertical();
        for l in rows {
            lines.push(Text::new(StyledString::new(l, self.mono_style())));
        }
        out.push(
            PaddedElement::new(lines, Margins::trbl(1.6, 2.2, 1.6, 2.2))
                .framed()
                .styled(line_style()),
        );
    }

    fn table(&mut self, out: &mut LinearLayout, node: Node) {
        let len = self.text.len();
        let mut aligns = Vec::new();
        let mut header = None;
        let mut rows = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let r = child.start_byte().min(len)..child.end_byte().min(len);
            match child.kind() {
                "pipe_table_header" => header = Some(child),
                "pipe_table_row" => rows.push(child),
                "pipe_table_delimiter_row" => {
                    if let Some(s) = self.text.get(r) {
                        aligns = crate::table::aligns(s);
                    }
                }
                _ => {}
            }
        }
        let cells_of = |this: &mut Self, row: Node| -> Vec<Vec<StyledString>> {
            let mut cells = Vec::new();
            let mut c = row.walk();
            for cell in row.children(&mut c) {
                if cell.kind() != "pipe_table_cell" {
                    continue;
                }
                let r = clip(
                    cell.start_byte().min(len)..cell.end_byte().min(len),
                    &this.sel,
                );
                let mut flow = Flow::runs();
                this.inline(&mut flow, trim(this.text, r));
                cells.push(flow.runs);
            }
            cells
        };
        let mut grid: Vec<(bool, Vec<Vec<StyledString>>)> = Vec::new();
        if let Some(h) = header {
            grid.push((true, cells_of(self, h)));
        }
        for row in rows {
            grid.push((false, cells_of(self, row)));
        }
        let ncols = grid.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
        if ncols == 0 {
            return;
        }
        // Column weights follow the widest cell so prose gets more room.
        let mut weights = vec![1usize; ncols];
        for (_, cells) in &grid {
            for (i, cell) in cells.iter().enumerate() {
                let n: usize = cell.iter().map(|s| s.s.chars().count()).sum();
                weights[i] = weights[i].max((n / 8 + 1).min(12));
            }
        }
        let mut table = TableLayout::new(weights);
        table.set_cell_decorator(FrameCellDecorator::new(true, true, false));
        for (is_header, cells) in grid {
            let mut row: Vec<Box<dyn Element>> = Vec::with_capacity(ncols);
            for (i, cell) in cells.into_iter().enumerate() {
                let extra = if is_header {
                    Style::new().bold().with_font_size(CELL_PT)
                } else {
                    Style::new().with_font_size(CELL_PT)
                };
                let runs: Vec<StyledString> = cell
                    .into_iter()
                    .map(|s| StyledString::new(s.s, s.style.and(extra)))
                    .collect();
                let alignment = match aligns.get(i).copied().unwrap_or(crate::table::Align::None) {
                    crate::table::Align::None | crate::table::Align::Left => Alignment::Left,
                    crate::table::Align::Center => Alignment::Center,
                    crate::table::Align::Right => Alignment::Right,
                };
                row.push(Box::new(PaddedElement::new(
                    Paragraph::from(runs).aligned(alignment),
                    Margins::all(1.2f32),
                )));
            }
            while row.len() < ncols {
                row.push(Box::new(Text::new("")));
            }
            row.truncate(ncols);
            let _ = table.push_row(row);
        }
        out.push(table.styled(line_style()));
    }

    /// Collects footnote definition bodies inside `node` (`[^label]: …` can
    /// parse as either a link reference definition or a paragraph). Returns
    /// whether the node held one and must not render in place.
    fn def_bodies(&mut self, node: Node, nb: &Range<usize>) -> bool {
        let len = self.text.len();
        let end = node.end_byte().min(len);
        // Adjacent `[^x]:` defs can share one paragraph node — each body ends
        // where the next def begins.
        let defs: Vec<(Range<usize>, Range<usize>)> = self
            .a
            .footnotes
            .iter()
            .filter(|f| f.def && f.range.start >= node.start_byte() && f.range.start < end)
            .map(|f| (f.range.clone(), f.label.clone()))
            .collect();
        if defs.is_empty() {
            return false;
        }
        for (i, (range, label)) in defs.iter().enumerate() {
            let next = defs.get(i + 1).map(|(r, _)| r.start).unwrap_or(end);
            let body = trim(self.text, clip(range.end..next, nb));
            if !body.is_empty() {
                self.defs.push((label.clone(), body));
            }
        }
        true
    }

    fn footnote_section(&mut self, out: &mut LinearLayout) {
        let defs = std::mem::take(&mut self.defs);
        if defs.is_empty() {
            return;
        }
        out.push(Break::new(0.4));
        out.push(Rule.styled(line_style()));
        out.push(Break::new(0.4));
        for (label, body) in defs {
            let l = self.text.get(label).unwrap_or_default();
            let mut runs = vec![StyledString::new(
                format!("[{l}]"),
                Style::new()
                    .bold()
                    .with_color(FAINT)
                    .with_font_size(FOOTNOTE_PT),
            )];
            let mut flow = Flow::runs();
            self.inline(&mut flow, body);
            if !flow.runs.is_empty() {
                runs.push(StyledString::new(
                    " ",
                    Style::new().with_font_size(FOOTNOTE_PT),
                ));
                for s in flow.runs {
                    runs.push(StyledString::new(
                        s.s,
                        s.style
                            .and(Style::new().with_color(MUTED).with_font_size(FOOTNOTE_PT)),
                    ));
                }
            }
            out.push(Paragraph::from(runs));
            out.push(Break::new(0.2));
        }
    }

    // ── Inline emission (flag-driven) ────────────────────────────────────

    fn inline(&mut self, flow: &mut Flow, range: Range<usize>) {
        let mut i = range.start;
        while i < range.end {
            if let Some(end) = self.special(flow, i) {
                i = end;
                continue;
            }
            if let Some(end) = self.hidden_end(i) {
                i = end;
                continue;
            }
            let f = self.a.flags.get(i).copied().unwrap_or(0);
            if f & md::MARK != 0 {
                i += 1;
                continue;
            }
            if f & md::LINK != 0 {
                let mut j = i + 1;
                while j < range.end
                    && self.flags_at(j) & md::LINK != 0
                    && self.flags_at(j) & md::MARK == 0
                    && self.hidden_end(j).is_none()
                    && !self.special_start(j)
                {
                    j += 1;
                }
                self.link(flow, i..j);
                i = j;
                continue;
            }
            let sf = f & STYLE;
            let mut j = i + 1;
            while j < range.end
                && self.flags_at(j) & STYLE == sf
                && self.flags_at(j) & (md::LINK | md::MARK) == 0
                && self.hidden_end(j).is_none()
                && !self.special_start(j)
            {
                j += 1;
            }
            flow.run(self.styled(
                self.text.get(i..j).unwrap_or_default(),
                sf,
                self.a.tint_at(i),
            ));
            i = j;
        }
    }

    fn flags_at(&self, i: usize) -> u16 {
        self.a.flags.get(i).copied().unwrap_or(0)
    }

    /// The `i`-positioned image / wiki-link / footnote constructs.
    fn special(&mut self, flow: &mut Flow, i: usize) -> Option<usize> {
        if let Some(im) = at(&self.a.images, i, |x| &x.range) {
            let (range, src, embed) = (im.range.clone(), im.src.clone(), im.embed);
            let end = range.end;
            self.image(flow, range, src, embed);
            return Some(end);
        }
        if let Some(w) = at(&self.a.wiki_links, i, |x| &x.range) {
            let (range, target, alias) = (w.range.clone(), w.target.clone(), w.alias.clone());
            self.wiki(flow, target, alias);
            return Some(range.end);
        }
        if let Some(f) = at(&self.a.footnotes, i, |x| &x.range) {
            let (range, label, def) = (f.range.clone(), f.label.clone(), f.def);
            if !def {
                let l = self.text.get(label).unwrap_or_default();
                flow.run(StyledString::new(
                    format!("[{l}]"),
                    Style::new()
                        .with_color(FAINT)
                        .with_font_size(FOOTNOTE_REF_PT),
                ));
            }
            return Some(range.end);
        }
        None
    }

    fn special_start(&self, i: usize) -> bool {
        at(&self.a.images, i, |x| &x.range).is_some()
            || at(&self.a.wiki_links, i, |x| &x.range).is_some()
            || at(&self.a.footnotes, i, |x| &x.range).is_some()
    }

    fn hidden_end(&self, i: usize) -> Option<usize> {
        let idx = self.hidden.partition_point(|r| r.end <= i);
        self.hidden.get(idx).filter(|r| r.start <= i).map(|r| r.end)
    }

    fn ink_style(&self) -> Style {
        Style::new().with_color(self.ink)
    }

    fn mono_style(&self) -> Style {
        let mut s = Style::new().with_color(CODE_INK);
        s.set_font_family(self.mono);
        s.set_font_size(CODE_PT);
        s
    }

    /// A run of text under `sf` — underline/strike degrade to plain text,
    /// highlight degrades to a tinted color, links to blue.
    fn styled(&self, s: &str, f: u16, tint: Option<md::Tint>) -> StyledString {
        let mut style = self.ink_style();
        if f & md::BOLD != 0 {
            style.set_bold();
        }
        if f & md::ITALIC != 0 {
            style.set_italic();
        }
        if f & md::CODE != 0 {
            style.set_font_family(self.mono);
            style.set_font_size(CODE_PT);
            style.set_color(CODE_INK);
        }
        if f & md::HIGHLIGHT != 0 {
            style.set_color(tint_color(tint));
        }
        if self.link_depth > 0 {
            style.set_color(LINK);
        }
        StyledString::new(s, style)
    }

    /// `![alt](src)` / `![[src]]` — scaled to the text column, or `[alt]`
    /// when the file can't be loaded (or the context can't hold an image).
    fn image(&mut self, flow: &mut Flow, range: Range<usize>, src: Range<usize>, embed: bool) {
        let raw = self.text.get(src.clone()).unwrap_or_default();
        let bare = raw
            .strip_prefix('<')
            .and_then(|r| r.strip_suffix('>'))
            .unwrap_or(raw);
        let alt = if embed {
            bare
        } else {
            // The description sits between `![` and `](`.
            self.text
                .get(range.start + 2..src.start.saturating_sub(2))
                .unwrap_or_default()
        };
        if flow.layout.is_some()
            && let Some(img) = self.image_el(bare, embed)
        {
            flow.element(img);
            return;
        }
        flow.run(StyledString::new(
            if alt.is_empty() {
                "[image]".to_string()
            } else {
                format!("[{alt}]")
            },
            Style::new().italic().with_color(FAINT),
        ));
    }

    /// The `![alt](path)`/`![[path]]` image scaled to the text column.
    fn image_el(&self, bare: &str, embed: bool) -> Option<Image> {
        let p = self.image_path(bare, embed)?;
        let mut img = Image::from_path(&p).ok()?;
        let (w, h) = image::image_dimensions(&p).ok()?;
        // genpdf draws at 300 dpi by default — px→mm is px * 25.4/300.
        let to_mm = |px: u32| px as f64 * 25.4 / 300.0;
        let s = (TEXT_W / to_mm(w)).min(MAX_IMG_H / to_mm(h)).min(1.0);
        if s < 1.0 {
            img.set_scale(Scale::new(s, s));
        }
        img.set_alignment(Alignment::Center);
        Some(img)
    }

    /// Resolves an image source to an existing local file — absolute paths,
    /// `attachments/` embeds searched like the in-app resolver, plain
    /// relative paths under the note's folder.
    fn image_path(&self, bare: &str, embed: bool) -> Option<PathBuf> {
        if bare.contains("://") {
            return None;
        }
        let dec = attach::decode(bare);
        if Path::new(&dec).is_absolute() {
            return Path::new(&dec).exists().then(|| PathBuf::from(&dec));
        }
        if embed && let (Some(dir), Some(root)) = (self.ctx.note_dir, self.ctx.root) {
            for cand in attach::candidates(bare, true, dir, root) {
                if cand.exists() {
                    return Some(cand);
                }
            }
            return None;
        }
        let dir = self.ctx.note_dir?;
        let p = dir.join(&dec);
        p.exists().then_some(p)
    }

    /// `[[target]]` / `[[target|alias]]` — colored when it resolves.
    fn wiki(&mut self, flow: &mut Flow, target: Range<usize>, alias: Option<Range<usize>>) {
        let visible = alias.unwrap_or_else(|| target.clone());
        let resolved = self
            .ctx
            .tree
            .and_then(|t| {
                crate::links::resolve(t, self.text.get(target.clone()).unwrap_or_default())
            })
            .is_some();
        let mut style = self.ink_style();
        if resolved {
            style.set_color(LINK);
        }
        flow.run(StyledString::new(
            self.text.get(visible).unwrap_or_default(),
            style,
        ));
    }

    /// A LINK-flagged run: colored text (PDF links aren't clickable).
    fn link(&mut self, flow: &mut Flow, range: Range<usize>) {
        let Some((_d, inner)) = self.dest(&range) else {
            self.styled_runs(flow, range);
            return;
        };
        self.link_depth += 1;
        self.styled_runs(flow, inner);
        self.link_depth -= 1;
    }

    /// Text inside a link run: the same flag segmentation minus the LINK bit.
    fn styled_runs(&mut self, flow: &mut Flow, range: Range<usize>) {
        let mut i = range.start;
        while i < range.end {
            if let Some(end) = self.hidden_end(i) {
                i = end;
                continue;
            }
            let f = self.flags_at(i);
            if f & md::MARK != 0 {
                i += 1;
                continue;
            }
            let sf = f & STYLE;
            let mut j = i + 1;
            while j < range.end
                && self.flags_at(j) & STYLE == sf
                && self.flags_at(j) & md::MARK == 0
                && self.hidden_end(j).is_none()
            {
                j += 1;
            }
            flow.run(self.styled(
                self.text.get(i..j).unwrap_or_default(),
                sf,
                self.a.tint_at(i),
            ));
            i = j;
        }
    }

    /// The destination and display range of the link ending at `range`:
    /// inline `[t](u)`, reference `[t][id]` / `[t][]`, shortcut `[t]` or
    /// `<autolink>` (whose `<>` never show in the text).
    fn dest(&self, range: &Range<usize>) -> Option<(String, Range<usize>)> {
        let rest = self
            .text
            .get(range.end.min(self.text.len())..)
            .unwrap_or_default();
        if let Some(r) = rest.strip_prefix("](") {
            let r = r.trim_start();
            let d = if let Some(r) = r.strip_prefix('<') {
                r.find('>').map(|n| r[..n].to_string())
            } else {
                let n = r
                    .find(|c: char| c == ')' || c.is_whitespace())
                    .unwrap_or(r.len());
                Some(r[..n].to_string())
            };
            return d.map(|d| (d, range.clone()));
        }
        if let Some(r) = rest.strip_prefix("][") {
            let n = r.find(']')?;
            let label = if n == 0 {
                self.text.get(range.clone()).unwrap_or_default().to_string()
            } else {
                r[..n].to_string()
            };
            return self.def_lookup(&label).map(|d| (d, range.clone()));
        }
        self.def_lookup(self.text.get(range.clone()).unwrap_or_default())
            .map(|d| (d, range.clone()))
            .or_else(|| {
                let inner = self.text.get(range.clone()).unwrap_or_default();
                autolink(inner).map(|d| {
                    let r = if inner.starts_with('<') && inner.ends_with('>') {
                        range.start + 1..range.end - 1
                    } else {
                        range.clone()
                    };
                    (d, r)
                })
            })
    }

    fn def_lookup(&self, label: &str) -> Option<String> {
        let l = label.trim().to_lowercase();
        self.link_defs
            .iter()
            .find(|(k, _)| *k == l)
            .map(|(_, d)| d.clone())
    }
}

/// `[label]: dest` definitions anywhere in the note (reference-style links).
fn link_defs(text: &str) -> Vec<(String, String)> {
    let mut defs = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        let Some(r) = l.strip_prefix('[') else {
            continue;
        };
        let Some(close) = r.find("]:") else {
            continue;
        };
        let label = &r[..close];
        if label.is_empty() || label.starts_with('^') {
            continue;
        }
        let d = r[close + 2..].trim_start();
        let d = d
            .strip_prefix('<')
            .and_then(|s| s.find('>').map(|n| &s[..n]))
            .unwrap_or_else(|| d.split_whitespace().next().unwrap_or_default());
        if !d.is_empty() {
            defs.push((label.to_lowercase(), d.to_string()));
        }
    }
    defs
}

/// Splits `line` at `CODE_COLS` chars so over-long lines stay inside the
/// code frame instead of overflowing the page.
fn wrap_code(rows: &mut Vec<String>, line: &str) {
    let mut rest = line;
    loop {
        let Some(cut) = rest.char_indices().nth(CODE_COLS).map(|(i, _)| i) else {
            rows.push(rest.to_string());
            return;
        };
        rows.push(rest[..cut].to_string());
        rest = &rest[cut..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::Context;

    fn ctx() -> Context<'static> {
        Context {
            note_dir: None,
            root: None,
            tree: None,
        }
    }

    #[test]
    fn export_emits_a_pdf() {
        let bytes = export("# Título\n\nparágrafo **bold** e `code`\n", "T", &ctx()).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn weird_input_does_not_panic() {
        for text in [
            "", "# ", "\u{0}", "- \n", "[]()", "![](x)", "[^a]", "```", "> [!]",
        ] {
            let _ = export(text, "", &ctx()); // must not panic
        }
    }

    #[test]
    fn big_note_exports() {
        let mut text = String::new();
        for i in 0..8000 {
            text.push_str(&format!(
                "## Seção {i}\n\nparágrafo com **bold** e ==mark==\n\n- item\n- item\n\n"
            ));
        }
        let bytes = export(&text, "big", &ctx()).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}
