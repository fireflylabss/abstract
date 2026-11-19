//! Markdown → HTML for "Export as HTML…" and "Copy as HTML". Block structure
//! comes from a fresh tree-sitter parse; inline styling reuses the editor's
//! `md::Analysis` flags and conceals, which already cover `==mark==`,
//! `%%comment%%`, callouts, footnotes, wiki-links and images. Pure — no
//! gpui — so it is testable headless.

use std::ops::Range;
use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::core::attach;
use crate::core::md::{self, Analysis, Analyzer};
use crate::core::vault;

/// Where `[[wiki-links]]` and `![[embeds]]` resolve from. `note_dir` is the
/// folder the note lives in — the save dialog opens there, so relative
/// hrefs keep working when the file lands next to the note.
pub(crate) struct Context<'a> {
    pub note_dir: Option<&'a Path>,
    /// Space root, for `![[…]]` embed resolution.
    pub root: Option<&'a Path>,
    /// Vault tree (wiki-link targets).
    pub tree: Option<&'a [vault::Node]>,
}

pub(crate) const STYLE: u16 =
    md::BOLD | md::ITALIC | md::UNDERLINE | md::STRIKE | md::HIGHLIGHT | md::CODE;

/// A whole `.html` document: embedded CSS, `<title>` from the note.
pub(crate) fn export(text: &str, title: &str, ctx: &Context) -> String {
    let a = Analyzer::new().analyze(text);
    let mut body = String::new();
    Emitter::new(text, &a, 0..text.len(), ctx).render(&mut body);
    let lang = match crate::core::i18n::current() {
        crate::core::i18n::Lang::En => "en",
        crate::core::i18n::Lang::PtBr => "pt-BR",
    };
    let mut out = String::with_capacity(body.len() + CSS.len() + title.len() + 320);
    out.push_str("<!doctype html>\n<html lang=\"");
    out.push_str(lang);
    out.push_str("\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>");
    escape(&mut out, title);
    out.push_str("</title>\n<style>\n");
    out.push_str(CSS);
    out.push_str("</style>\n</head>\n<body>\n");
    out.push_str(&body);
    out.push_str("</body>\n</html>\n");
    out
}

/// A body fragment over `sel` — what "Copy as HTML" puts on the clipboard.
pub(crate) fn fragment(text: &str, a: &Analysis, sel: Range<usize>, ctx: &Context) -> String {
    let mut out = String::new();
    Emitter::new(text, a, sel, ctx).render(&mut out);
    out
}

/// The `text/plain` fallback for the clipboard: what the selection shows,
/// markup and `%%comments%%` stripped.
pub(crate) fn plain(text: &str, a: &Analysis, range: Range<usize>) -> String {
    let comments = comments(text, a);
    let far = usize::MAX..usize::MAX;
    let last = a.line_of(range.end.saturating_sub(1).max(range.start));
    let mut out = String::new();
    for l in a.line_of(range.start)..=last {
        for r in a.visible(a.lines[l].0.clone(), &far) {
            let r = clip(r, &range);
            let mut pos = r.start;
            for c in &comments {
                if c.end <= pos || c.start >= r.end {
                    continue;
                }
                let cut = c.start.max(pos).min(r.end);
                out.push_str(&text[pos..cut]);
                pos = c.end.min(r.end).max(pos);
            }
            out.push_str(&text[pos..r.end]);
        }
        out.push('\n');
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// `%%…%%` comment spans in `text`, skipped inside code — same scan the
/// analyzer runs for its MUTED|ITALIC marking.
pub(crate) fn comments(text: &str, a: &Analysis) -> Vec<Range<usize>> {
    md::comment_spans(text, &|o| {
        a.flags[o] & md::CODE != 0 || a.lines[a.line_of(o)].1 == md::Kind::Code
    })
}

pub(crate) fn clip(r: Range<usize>, sel: &Range<usize>) -> Range<usize> {
    r.start.max(sel.start)..r.end.min(sel.end)
}

pub(crate) fn trim(text: &str, mut r: Range<usize>) -> Range<usize> {
    let ws = |i: usize| text.as_bytes()[i].is_ascii_whitespace();
    while r.start < r.end && ws(r.start) {
        r.start += 1;
    }
    while r.end > r.start && ws(r.end - 1) {
        r.end -= 1;
    }
    r
}

fn escape(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn escape_attr(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

/// The `<mark>` open tag for a tint: `Yellow` keeps the plain `<mark>`
/// (stylesheet default); the rest carry an inline `background`.
fn mark_open(tint: Option<md::Tint>) -> &'static str {
    match tint.unwrap_or_default() {
        md::Tint::Red => "<mark style=\"background:#ffd7d5\">",
        md::Tint::Orange => "<mark style=\"background:#ffe0b3\">",
        md::Tint::Green => "<mark style=\"background:#d2f0d8\">",
        md::Tint::Blue => "<mark style=\"background:#d3e3fd\">",
        md::Tint::Purple => "<mark style=\"background:#e8d8fa\">",
        md::Tint::Yellow => "<mark>",
    }
}

/// Text with the app's inline style flags mapped to tags.
fn styled(out: &mut String, s: &str, f: u16, tint: Option<md::Tint>) {
    if s.is_empty() {
        return;
    }
    const TAGS: [(u16, &str, &str); 6] = [
        (md::BOLD, "<strong>", "</strong>"),
        (md::ITALIC, "<em>", "</em>"),
        (md::UNDERLINE, "<u>", "</u>"),
        (md::STRIKE, "<del>", "</del>"),
        (md::HIGHLIGHT, "<mark>", "</mark>"),
        (md::CODE, "<code>", "</code>"),
    ];
    for (flag, open, _) in TAGS {
        if f & flag != 0 {
            out.push_str(if flag == md::HIGHLIGHT {
                mark_open(tint)
            } else {
                open
            });
        }
    }
    escape(out, s);
    for (flag, _, close) in TAGS.iter().rev() {
        if f & flag != 0 {
            out.push_str(close);
        }
    }
}

struct Emitter<'a> {
    text: &'a str,
    a: &'a Analysis,
    ctx: &'a Context<'a>,
    /// Emission clip — the selection for "Copy as HTML", the whole note for
    /// export.
    sel: Range<usize>,
    /// Concealed syntax + `%%…%%` spans, merged and sorted.
    hidden: Vec<Range<usize>>,
    /// Footnote definitions: `(label, body)` ranges, in document order.
    defs: Vec<(Range<usize>, Range<usize>)>,
    /// `[label]: dest` definitions for reference-style links; built lazily.
    link_defs: Option<Vec<(String, String)>>,
}

mod emitter;

/// LaTeX `src` → MathML via pulldown-latex: real math in the export with
/// no JS and no CDN. `block` picks centered `display="block"` math; a span
/// the converter can't read degrades to `<merror>` for that part only.
fn mathml(out: &mut String, src: &str, block: bool) {
    let src = src.trim();
    let storage = pulldown_latex::Storage::new();
    let parser = pulldown_latex::Parser::new(src, &storage);
    let config = pulldown_latex::RenderConfig {
        display_mode: if block {
            pulldown_latex::config::DisplayMode::Block
        } else {
            pulldown_latex::config::DisplayMode::Inline
        },
        ..Default::default()
    };
    let mut body = String::new();
    if pulldown_latex::push_mathml(&mut body, parser, config).is_ok() {
        out.push_str(&body);
    } else {
        // The export never swallows the source: unrenderable math shows up
        // as literal code instead of a broken `<math>` fragment.
        out.push_str("<code class=\"math\">");
        escape(out, src);
        out.push_str("</code>");
    }
}

/// Entry with `range().start == i` in a sorted vec.
pub(crate) fn at<T>(v: &[T], i: usize, r: impl Fn(&T) -> &Range<usize>) -> Option<&T> {
    let idx = v.partition_point(|x| r(x).start < i);
    match v.get(idx) {
        Some(x) if r(x).start == i => Some(x),
        _ => None,
    }
}

/// `http://x`, `www.x` or `name@host` — the uri/email autolink cases.
pub(crate) fn autolink(inner: &str) -> Option<String> {
    let s = inner.trim();
    let s = s
        .strip_prefix('<')
        .and_then(|x| x.strip_suffix('>'))
        .unwrap_or(s);
    if s.contains("://") || s.starts_with("www.") || s.starts_with("mailto:") {
        Some(s.to_string())
    } else if s.contains('@') && !s.contains(char::is_whitespace) {
        Some(format!("mailto:{s}"))
    } else {
        None
    }
}

pub(crate) fn heading_level(heading: Node) -> u8 {
    let mut cursor = heading.walk();
    heading
        .children(&mut cursor)
        .find_map(|c| match c.kind() {
            "atx_h1_marker" | "setext_h1_underline" => Some(1),
            "atx_h2_marker" | "setext_h2_underline" => Some(2),
            "atx_h3_marker" => Some(3),
            "atx_h4_marker" => Some(4),
            "atx_h5_marker" => Some(5),
            "atx_h6_marker" => Some(6),
            _ => None,
        })
        .unwrap_or(1)
}

const CSS: &str = "\
body{font-family:'Noto Sans',system-ui,-apple-system,'Segoe UI',sans-serif;\
max-width:42rem;margin:2rem auto;padding:0 1.2rem;line-height:1.55;color:#1d1d1f}
code,pre{font-family:'Noto Sans Mono',ui-monospace,Menlo,monospace}
code{background:#f4f4f5;padding:.12em .3em;border-radius:4px;font-size:.92em}
pre{background:#f4f4f5;padding:.9em 1em;border-radius:8px;overflow-x:auto}
pre code{background:none;padding:0}
mark{background:#ffe58a;padding:0 .12em;border-radius:2px}
blockquote{margin:1em 0;padding-left:1em;border-left:3px solid #dcdce0;color:#555}
.callout{border:1px solid #e4e4e8;border-left:4px solid #8a8a92;border-radius:8px;\
padding:.5em 1em;margin:1em 0}
.callout-note{border-left-color:#4a90d9;background:#f5f9fe}
.callout-tip{border-left-color:#3fa34d;background:#f5fbf6}
.callout-warning{border-left-color:#dfa100;background:#fffaf0}
.callout-danger{border-left-color:#d64545;background:#fff5f5}
.callout-title{margin:.3em 0;font-weight:600}
table{border-collapse:collapse;margin:1em 0}
th,td{border:1px solid #dcdce0;padding:.3em .75em}
img{max-width:100%}
li.task{list-style:none;margin-left:-1.4em}
li.task>input{margin-right:.5em}
.footnotes{font-size:.9em;color:#555}
.footnotes li{margin:.3em 0}
a{color:#4a90d9}
hr{border:none;border-top:1px solid #dcdce0}
";

#[cfg(test)]
mod tests;
