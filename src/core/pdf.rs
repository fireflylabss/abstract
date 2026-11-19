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

use crate::core::attach;
use crate::core::html::{Context, STYLE, at, autolink, clip, comments, heading_level, trim};
use crate::core::md::{self, Analysis, Analyzer};

const FONT_SANS: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const FONT_SANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Bold.ttf");
const FONT_SANS_ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Italic.ttf");
// Noto Sans has no bold-italic face — SemiBold Italic is the closest.
const FONT_SANS_BOLD_ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSans-SemiBoldItalic.ttf");
const FONT_MONO: &[u8] = include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf");
const FONT_MONO_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSansMono-Bold.ttf");
// Noto Sans has no math coverage (∑, ∫, arrows, super-/subscripts come
// out as .notdef boxes), so math prints in a DejaVu Sans subset instead —
// italic keeps it set apart from the prose face.
const FONT_MATH: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-math-subset.ttf");
const FONT_MATH_ITALIC: &[u8] =
    include_bytes!("../../assets/fonts/DejaVuSans-Oblique-math-subset.ttf");

/// Glyphs `math.rs` can emit that DejaVu lacks — a run holding one falls
/// back to its LaTeX source rather than drawing a .notdef box.
const MATH_TOFU: &str = "\u{2322}\u{2323}\u{2a06}\u{2a3f}";

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
        let (mono, math) = add_fonts(&mut doc)?;
        doc.set_page_decorator(Footer::default());
        let a = Analyzer::new().analyze(text);
        let mut body = LinearLayout::vertical();
        Emitter::new(text, &a, 0..text.len(), ctx, mono, math).render(&mut body);
        doc.push(body);
        let mut out = Vec::new();
        doc.render(&mut out)?;
        Ok(out)
    };
    // Debug builds may still panic (release aborts) — never propagate one.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(build))
        .unwrap_or_else(|_| Err(Error::new("PDF export panicked", ErrorKind::Internal)))
}

/// The extra font families an `Emitter` needs: `(mono, math)`.
fn add_fonts(doc: &mut genpdf::Document) -> Result<(FontFamily<Font>, FontFamily<Font>), Error> {
    let mono = doc.add_font_family(FontFamily {
        regular: FontData::new(FONT_MONO.to_vec(), None)?,
        bold: FontData::new(FONT_MONO_BOLD.to_vec(), None)?,
        italic: FontData::new(FONT_MONO.to_vec(), None)?,
        bold_italic: FontData::new(FONT_MONO_BOLD.to_vec(), None)?,
    });
    // The math face is always set italic; bold maps reuse the same pair.
    let math = doc.add_font_family(FontFamily {
        regular: FontData::new(FONT_MATH.to_vec(), None)?,
        bold: FontData::new(FONT_MATH_ITALIC.to_vec(), None)?,
        italic: FontData::new(FONT_MATH_ITALIC.to_vec(), None)?,
        bold_italic: FontData::new(FONT_MATH_ITALIC.to_vec(), None)?,
    });
    Ok((mono, math))
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
    /// DejaVu face for `$…$`/`$$…$$` math (the Noto faces lack the glyphs).
    math: FontFamily<Font>,
    /// Body text color — gray inside plain block quotes.
    ink: Color,
    /// Inside a link run (links print colored; they aren't clickable).
    link_depth: usize,
}

mod blocks;
mod inline;

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
mod tests;
