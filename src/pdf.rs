//! Markdown blocks and styled runs for PDF export with shaped text and bundled fonts.

use crate::attach;
use crate::html::{Context, STYLE, at, autolink, clip, comments, heading_level, trim};
use crate::md::{self, Analysis, Analyzer};
use std::ops::Range;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};
mod model;
mod writer;
use model::*;

const FONT_SANS: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const FONT_SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
const FONT_SANS_ITALIC: &[u8] = include_bytes!("../assets/fonts/NotoSans-Italic.ttf");
// Noto Sans has no bold-italic face — SemiBold Italic is the closest.
const FONT_SANS_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/NotoSans-SemiBoldItalic.ttf");
const FONT_MONO: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf");
const FONT_MONO_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf");
// Noto Sans has no math coverage (∑, ∫, arrows, super-/subscripts come
// out as .notdef boxes), so math prints in a DejaVu Sans subset instead —
// italic keeps it set apart from the prose face.
const FONT_MATH: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-math-subset.ttf");
const FONT_MATH_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSans-Oblique-math-subset.ttf");

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

fn line_style() -> Style {
    Style::new().with_color(LINE)
}

/// Highlights print as text color.
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

pub(crate) fn export(text: &str, title: &str, ctx: &Context) -> std::io::Result<Vec<u8>> {
    let a = Analyzer::new().analyze(text);
    let mut body = LinearLayout::vertical();
    Emitter::new(text, &a, 0..text.len(), ctx, Family::Mono, Family::Math).render(&mut body);
    writer::export(body, title)
}

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
        // Soft line breaks in a paragraph render as spaces, like HTML.
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

    fn element(&mut self, el: impl Into<Block>) {
        self.flush();
        if let Some(layout) = self.layout.as_mut() {
            layout.push(el);
        }
    }
}

/// The same walk `html.rs` does — blocks to writer-independent elements, inline flags
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
    mono: Family,
    /// DejaVu face for `$…$`/`$$…$$` math (the Noto faces lack the glyphs).
    math: Family,
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
