//! Markdown → HTML for "Export as HTML…" and "Copy as HTML". Block structure
//! comes from a fresh tree-sitter parse; inline styling reuses the editor's
//! `md::Analysis` flags and conceals, which already cover `==mark==`,
//! `%%comment%%`, callouts, footnotes, wiki-links and images. Pure — no
//! gpui — so it is testable headless.

use std::ops::Range;
use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::attach;
use crate::md::{self, Analysis, Analyzer};
use crate::vault;

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

const STYLE: u16 = md::BOLD | md::ITALIC | md::UNDERLINE | md::STRIKE | md::HIGHLIGHT | md::CODE;

/// A whole `.html` document: embedded CSS, `<title>` from the note.
pub(crate) fn export(text: &str, title: &str, ctx: &Context) -> String {
    let a = Analyzer::new().analyze(text);
    let mut body = String::new();
    Emitter::new(text, &a, 0..text.len(), ctx).render(&mut body);
    let lang = match crate::i18n::current() {
        crate::i18n::Lang::En => "en",
        crate::i18n::Lang::PtBr => "pt-BR",
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
fn comments(text: &str, a: &Analysis) -> Vec<Range<usize>> {
    md::comment_spans(text, &|o| {
        a.flags[o] & md::CODE != 0 || a.lines[a.line_of(o)].1 == md::Kind::Code
    })
}

fn clip(r: Range<usize>, sel: &Range<usize>) -> Range<usize> {
    r.start.max(sel.start)..r.end.min(sel.end)
}

fn trim(text: &str, mut r: Range<usize>) -> Range<usize> {
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

/// Text with the app's inline style flags mapped to tags.
fn styled(out: &mut String, s: &str, f: u16) {
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
            out.push_str(open);
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

impl<'a> Emitter<'a> {
    fn new(text: &'a str, a: &'a Analysis, sel: Range<usize>, ctx: &'a Context<'a>) -> Self {
        let mut hidden: Vec<Range<usize>> = a.conceals.iter().map(|c| c.hidden.clone()).collect();
        let comments = comments(text, a);
        hidden.extend(comments);
        hidden.sort_by_key(|r| r.start);
        Self {
            text,
            a,
            ctx,
            sel,
            hidden,
            defs: Vec::new(),
            link_defs: None,
        }
    }

    fn render(&mut self, out: &mut String) {
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

    fn children(&mut self, out: &mut String, node: Node) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.block(out, child);
        }
    }

    fn block(&mut self, out: &mut String, node: Node) {
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
                let level = heading_level(node);
                out.push_str(&format!("<h{level}>"));
                self.inline_nodes(out, node);
                out.push_str(&format!("</h{level}>\n"));
            }
            "paragraph" => {
                if self.def_bodies(node, &nb) {
                    return;
                }
                let start = out.len();
                out.push_str("<p>");
                self.inline_nodes(out, node);
                if out.len() == start + 3 {
                    out.truncate(start);
                } else {
                    out.push_str("</p>\n");
                }
            }
            "block_quote" => self.quote(out, node),
            "list" => self.list(out, node),
            "list_item" => self.list_item(out, node),
            "thematic_break" => out.push_str("<hr>\n"),
            "fenced_code_block" => self.code(out, node, true),
            "indented_code_block" => self.code(out, node, false),
            "pipe_table" => self.table(out, node),
            "link_reference_definition" => {
                self.def_bodies(node, &nb);
            }
            "minus_metadata" | "plus_metadata" => {}
            "html_block" => {
                out.push_str("<p>");
                escape(&mut *out, &self.text[nb]);
                out.push_str("</p>\n");
            }
            _ => self.children(out, node),
        }
    }

    /// The `inline` children of a node (paragraph, heading).
    fn inline_nodes(&mut self, out: &mut String, node: Node) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "inline" {
                let len = self.text.len();
                self.inline(
                    out,
                    clip(
                        child.start_byte().min(len)..child.end_byte().min(len),
                        &self.sel,
                    ),
                );
            }
        }
    }

    fn quote(&mut self, out: &mut String, node: Node) {
        let first = self.a.line_of(node.start_byte().min(self.text.len()));
        let Some(callout) = self.a.callouts.iter().find(|c| c.lines.start == first) else {
            out.push_str("<blockquote>\n");
            self.children(out, node);
            out.push_str("</blockquote>\n");
            return;
        };
        let tone = match callout.tone {
            md::Tone::Note => "note",
            md::Tone::Tip => "tip",
            md::Tone::Warning => "warning",
            md::Tone::Danger => "danger",
        };
        out.push_str(&format!("<div class=\"callout callout-{tone}\">\n"));
        // The title is the first line of the first paragraph; the same
        // paragraph node continues into the callout body.
        let title_end = self.a.lines[callout.lines.start].0.end;
        let mut titled = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !titled && child.kind() == "paragraph" {
                titled = true;
                let mut c = child.walk();
                for inner in child.children(&mut c) {
                    if inner.kind() != "inline" {
                        continue;
                    }
                    let len = self.text.len();
                    let r = inner.start_byte().min(len)..inner.end_byte().min(len);
                    out.push_str("<p class=\"callout-title\">");
                    self.inline(out, clip(r.start..r.end.min(title_end), &self.sel));
                    out.push_str("</p>\n");
                    let start = out.len();
                    out.push_str("<p>");
                    self.inline(out, trim(self.text, clip(title_end..r.end, &self.sel)));
                    if out.len() == start + 3 {
                        out.truncate(start);
                    } else {
                        out.push_str("</p>\n");
                    }
                }
                continue;
            }
            self.block(out, child);
        }
        out.push_str("</div>\n");
    }

    fn list(&mut self, out: &mut String, node: Node) {
        let mut cursor = node.walk();
        let ordered = node.children(&mut cursor).any(|item| {
            let mut c = item.walk();
            item.children(&mut c)
                .any(|k| matches!(k.kind(), "list_marker_dot" | "list_marker_parenthesis"))
        });
        out.push_str(if ordered { "<ol>\n" } else { "<ul>\n" });
        self.children(out, node);
        out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
    }

    fn list_item(&mut self, out: &mut String, node: Node) {
        let mut cursor = node.walk();
        let kids: Vec<Node> = node.children(&mut cursor).collect();
        let checked = kids.iter().find_map(|k| match k.kind() {
            "task_list_marker_checked" => Some(true),
            "task_list_marker_unchecked" => Some(false),
            _ => None,
        });
        let paras = kids.iter().filter(|k| k.kind() == "paragraph").count();
        out.push_str(if checked.is_some() {
            "<li class=\"task\">"
        } else {
            "<li>"
        });
        if let Some(done) = checked {
            out.push_str(if done {
                "<input type=\"checkbox\" checked disabled> "
            } else {
                "<input type=\"checkbox\" disabled> "
            });
        }
        let mut seen_para = false;
        for child in kids {
            match child.kind() {
                "paragraph" => {
                    let wrap = paras > 1 || seen_para;
                    seen_para = true;
                    if wrap {
                        out.push_str("<p>");
                    }
                    self.inline_nodes(out, child);
                    if wrap {
                        out.push_str("</p>\n");
                    }
                }
                _ => self.block(out, child),
            }
        }
        out.push_str("</li>\n");
    }

    fn code(&mut self, out: &mut String, node: Node, fenced: bool) {
        let len = self.text.len();
        let mut lang = String::new();
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
            let r = child.start_byte().min(len)..child.end_byte().min(len);
            match child.kind() {
                "info_string" => {
                    lang = self.text[r.clone()]
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_lowercase();
                }
                "code_fence_content" => content = Some(clip(r, &self.sel)),
                _ => {}
            }
        }
        out.push_str("<pre><code");
        if !lang.is_empty() {
            out.push_str(" class=\"language-");
            escape_attr(&mut *out, &lang);
            out.push('"');
        }
        out.push('>');
        if let Some(r) = content {
            let s = &self.text[r];
            if fenced {
                escape(&mut *out, s);
            } else {
                for (i, line) in s.split('\n').enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    let indent = line.len() - line.trim_start_matches(' ').len();
                    escape(&mut *out, &line[indent.min(4)..]);
                }
            }
        }
        out.push_str("</code></pre>\n");
    }

    fn table(&mut self, out: &mut String, node: Node) {
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
                    aligns = crate::table::aligns(&self.text[r.clone()]);
                }
                _ => {}
            }
        }
        let style = |col: usize| match aligns
            .get(col)
            .copied()
            .unwrap_or(crate::table::Align::None)
        {
            crate::table::Align::None => String::new(),
            crate::table::Align::Left => " style=\"text-align:left\"".to_string(),
            crate::table::Align::Center => " style=\"text-align:center\"".to_string(),
            crate::table::Align::Right => " style=\"text-align:right\"".to_string(),
        };
        let cells = |this: &mut Self, out: &mut String, row: Node, tag: &str| {
            out.push_str("<tr>");
            let mut c = row.walk();
            let mut col = 0;
            for cell in row.children(&mut c) {
                if cell.kind() != "pipe_table_cell" {
                    continue;
                }
                let r = clip(
                    cell.start_byte().min(this.text.len())..cell.end_byte().min(this.text.len()),
                    &this.sel,
                );
                out.push('<');
                out.push_str(tag);
                out.push_str(&style(col));
                out.push('>');
                this.inline(out, trim(this.text, r));
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
                col += 1;
            }
            out.push_str("</tr>\n");
        };
        out.push_str("<table>\n");
        if let Some(h) = header {
            out.push_str("<thead>\n");
            cells(self, out, h, "th");
            out.push_str("</thead>\n");
        }
        if !rows.is_empty() {
            out.push_str("<tbody>\n");
            for row in rows {
                cells(self, out, row, "td");
            }
            out.push_str("</tbody>\n");
        }
        out.push_str("</table>\n");
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

    fn footnote_section(&mut self, out: &mut String) {
        let defs = std::mem::take(&mut self.defs);
        if defs.is_empty() {
            return;
        }
        out.push_str("<section class=\"footnotes\">\n<hr>\n<ul>\n");
        for (label, body) in defs {
            let l = &self.text[label];
            out.push_str("<li id=\"fn-");
            escape_attr(&mut *out, l);
            out.push_str("\"><sup>");
            escape(&mut *out, l);
            out.push_str("</sup> ");
            self.inline(out, body);
            out.push_str(" <a class=\"footnote-backref\" href=\"#fnref-");
            escape_attr(&mut *out, l);
            out.push_str("\">&#8617;</a></li>\n");
        }
        out.push_str("</ul>\n</section>\n");
    }

    // ── Inline emission (flag-driven) ────────────────────────────────────

    fn inline(&mut self, out: &mut String, range: Range<usize>) {
        let mut i = range.start;
        while i < range.end {
            if let Some(end) = self.special(out, i) {
                i = end;
                continue;
            }
            if let Some(end) = self.hidden_end(i) {
                i = end;
                continue;
            }
            let f = self.a.flags[i];
            if f & md::MARK != 0 {
                i += 1;
                continue;
            }
            if f & md::LINK != 0 {
                let mut j = i + 1;
                while j < range.end
                    && self.a.flags[j] & md::LINK != 0
                    && self.a.flags[j] & md::MARK == 0
                    && self.hidden_end(j).is_none()
                    && !self.special_start(j)
                {
                    j += 1;
                }
                self.link(out, i..j);
                i = j;
                continue;
            }
            let sf = f & STYLE;
            let mut j = i + 1;
            while j < range.end
                && self.a.flags[j] & STYLE == sf
                && self.a.flags[j] & (md::LINK | md::MARK) == 0
                && self.hidden_end(j).is_none()
                && !self.special_start(j)
            {
                j += 1;
            }
            styled(out, &self.text[i..j], sf);
            i = j;
        }
    }

    /// The `i`-positioned image / wiki-link / footnote constructs.
    fn special(&mut self, out: &mut String, i: usize) -> Option<usize> {
        if let Some(im) = at(&self.a.images, i, |x| &x.range) {
            let (range, src, embed) = (im.range.clone(), im.src.clone(), im.embed);
            let end = range.end;
            self.image(out, range, src, embed);
            return Some(end);
        }
        if let Some(w) = at(&self.a.wiki_links, i, |x| &x.range) {
            let (range, target, alias) = (w.range.clone(), w.target.clone(), w.alias.clone());
            self.wiki(out, target, alias);
            return Some(range.end);
        }
        if let Some(f) = at(&self.a.footnotes, i, |x| &x.range) {
            let (range, label, def) = (f.range.clone(), f.label.clone(), f.def);
            if !def {
                let l = &self.text[label];
                out.push_str("<sup class=\"footnote-ref\" id=\"fnref-");
                escape_attr(&mut *out, l);
                out.push_str("\"><a href=\"#fn-");
                escape_attr(&mut *out, l);
                out.push_str("\">");
                escape(&mut *out, l);
                out.push_str("</a></sup>");
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

    /// `![alt](src)` / `![[src]]` — relative sources stay relative.
    fn image(&mut self, out: &mut String, range: Range<usize>, src: Range<usize>, embed: bool) {
        let raw = &self.text[src.clone()];
        let bare = raw
            .strip_prefix('<')
            .and_then(|r| r.strip_suffix('>'))
            .unwrap_or(raw);
        let mut href = bare.to_string();
        if embed && let (Some(dir), Some(root)) = (self.ctx.note_dir, self.ctx.root) {
            for cand in attach::candidates(bare, true, dir, root) {
                if cand.exists() {
                    href = attach::encode(&attach::relative(dir, &cand));
                    break;
                }
            }
        }
        let alt = if embed {
            bare
        } else {
            // The description sits between `![` and `](`.
            self.text
                .get(range.start + 2..src.start.saturating_sub(2))
                .unwrap_or_default()
        };
        out.push_str("<img src=\"");
        escape_attr(&mut *out, &href);
        out.push_str("\" alt=\"");
        escape_attr(&mut *out, alt);
        out.push_str("\">");
    }

    /// `[[target]]` / `[[target|alias]]` → `<a>` to the other .md file when it
    /// resolves, styled text otherwise.
    fn wiki(&mut self, out: &mut String, target: Range<usize>, alias: Option<Range<usize>>) {
        let visible = alias.unwrap_or_else(|| target.clone());
        let href = self
            .ctx
            .tree
            .and_then(|t| crate::links::resolve(t, &self.text[target.clone()]))
            .zip(self.ctx.note_dir)
            .map(|(p, dir)| attach::encode(&attach::relative(dir, &p)));
        match href {
            Some(h) => {
                out.push_str("<a href=\"");
                escape_attr(&mut *out, &h);
                out.push_str("\">");
                escape(&mut *out, &self.text[visible]);
                out.push_str("</a>");
            }
            None => {
                out.push_str("<span class=\"wikilink\">");
                escape(&mut *out, &self.text[visible]);
                out.push_str("</span>");
            }
        }
    }

    /// A LINK-flagged run: `<a href>` plus inner styling.
    fn link(&mut self, out: &mut String, range: Range<usize>) {
        let Some((d, inner)) = self.dest(&range) else {
            self.styled_runs(out, range);
            return;
        };
        out.push_str("<a href=\"");
        escape_attr(&mut *out, &d);
        out.push_str("\">");
        self.styled_runs(out, inner);
        out.push_str("</a>");
    }

    /// Text inside a link run: the same flag segmentation minus the LINK bit.
    fn styled_runs(&mut self, out: &mut String, range: Range<usize>) {
        let mut i = range.start;
        while i < range.end {
            if let Some(end) = self.hidden_end(i) {
                i = end;
                continue;
            }
            let f = self.a.flags[i];
            if f & md::MARK != 0 {
                i += 1;
                continue;
            }
            let sf = f & STYLE;
            let mut j = i + 1;
            while j < range.end
                && self.a.flags[j] & STYLE == sf
                && self.a.flags[j] & md::MARK == 0
                && self.hidden_end(j).is_none()
            {
                j += 1;
            }
            styled(out, &self.text[i..j], sf);
            i = j;
        }
    }

    /// The destination and display range of the link ending at `range`:
    /// inline `[t](u)`, reference `[t][id]` / `[t][]`, shortcut `[t]` or
    /// `<autolink>` (whose `<>` never show in the text).
    fn dest(&mut self, range: &Range<usize>) -> Option<(String, Range<usize>)> {
        let rest = &self.text[range.end..];
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
                self.text[range.clone()].to_string()
            } else {
                r[..n].to_string()
            };
            return self.def_lookup(&label).map(|d| (d, range.clone()));
        }
        self.def_lookup(&self.text[range.clone()])
            .map(|d| (d, range.clone()))
            .or_else(|| {
                let inner = &self.text[range.clone()];
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

    fn def_lookup(&mut self, label: &str) -> Option<String> {
        let l = label.trim().to_lowercase();
        self.link_defs()
            .iter()
            .find(|(k, _)| *k == l)
            .map(|(_, d)| d.clone())
    }

    /// `[label]: dest` definitions anywhere in the note.
    fn link_defs(&mut self) -> &Vec<(String, String)> {
        if self.link_defs.is_none() {
            let mut defs = Vec::new();
            for line in self.text.lines() {
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
            self.link_defs = Some(defs);
        }
        self.link_defs.as_ref().unwrap()
    }
}

/// Entry with `range().start == i` in a sorted vec.
fn at<T>(v: &[T], i: usize, r: impl Fn(&T) -> &Range<usize>) -> Option<&T> {
    let idx = v.partition_point(|x| r(x).start < i);
    match v.get(idx) {
        Some(x) if r(x).start == i => Some(x),
        _ => None,
    }
}

/// `http://x`, `www.x` or `name@host` — the uri/email autolink cases.
fn autolink(inner: &str) -> Option<String> {
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

fn heading_level(heading: Node) -> u8 {
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
mod tests {
    use super::*;
    use crate::links::parse;

    fn ctx() -> Context<'static> {
        Context {
            note_dir: None,
            root: None,
            tree: None,
        }
    }

    fn html(text: &str) -> String {
        let a = Analyzer::new().analyze(text);
        fragment(text, &a, 0..text.len(), &ctx())
    }

    #[test]
    fn headings_and_inline_styles() {
        let out = html("# Title\n\nA **bold** *it* __u__ ~~del~~ `co` ==mk==.\n");
        assert!(out.contains("<h1>Title</h1>"));
        assert!(out.contains("<strong>bold</strong>"));
        assert!(out.contains("<em>it</em>"));
        assert!(out.contains("<u>u</u>"));
        assert!(out.contains("<del>del</del>"));
        assert!(out.contains("<code>co</code>"));
        assert!(out.contains("<mark>mk</mark>"));
    }

    #[test]
    fn escapes_text() {
        let out = html("a < b & c > d \"e\"\n");
        assert!(out.contains("a &lt; b &amp; c &gt; d \"e\""));
    }

    #[test]
    fn comments_are_omitted() {
        let out = html("keep %%private%% this\n");
        assert_eq!(out, "<p>keep  this</p>\n");
        let out = html("%%whole line%%\n\nshown\n");
        assert!(!out.contains("whole line"));
        assert!(out.contains("shown"));
        // Inside code spans `%%` is literal.
        let out = html("`%%x%%`\n");
        assert!(out.contains("<code>%%x%%</code>"));
    }

    #[test]
    fn links_render_with_destinations() {
        let out = html("[site](https://x.dev) <https://y.dev>\n\n[ref][id]\n\n[id]: /docs\n");
        assert!(out.contains("<a href=\"https://x.dev\">site</a>"));
        assert!(out.contains("<a href=\"https://y.dev\">https://y.dev</a>"));
        assert!(out.contains("<a href=\"/docs\">ref</a>"));
        // The definition itself stays hidden.
        assert!(!out.contains("[id]:"));
    }

    #[test]
    fn wiki_links() {
        let out = html("see [[Target|the target]] and [[Missing]]\n");
        assert!(out.contains("<span class=\"wikilink\">the target</span>"));
        assert!(out.contains("<span class=\"wikilink\">Missing</span>"));
    }

    #[test]
    fn tasks_and_lists() {
        let out = html("- [ ] todo\n- [x] done\n\n1. one\n2. two\n");
        assert!(out.contains("<input type=\"checkbox\" disabled> todo"));
        assert!(out.contains("<input type=\"checkbox\" checked disabled> done"));
        assert!(out.contains("<ol>\n<li>one</li>\n<li>two</li>\n</ol>"));
    }

    #[test]
    fn code_blocks() {
        let out = html("```rust\nlet a = 1 < 2;\n```\n");
        assert!(
            out.contains("<pre><code class=\"language-rust\">let a = 1 &lt; 2;\n</code></pre>")
        );
    }

    #[test]
    fn tables() {
        let out = html("| a | b |\n|:-:|--:|\n| 1 | 2 |\n");
        assert!(out.contains("<th style=\"text-align:center\"><strong>a</strong></th>"));
        assert!(out.contains("<th style=\"text-align:right\"><strong>b</strong></th>"));
        assert!(out.contains("<td style=\"text-align:center\">1</td>"));
    }

    #[test]
    fn callouts() {
        let out = html("> [!warning] Careful\n> body text\n");
        assert!(out.contains("class=\"callout callout-warning\""));
        assert!(out.contains("<p class=\"callout-title\"><strong>Careful</strong></p>"));
        assert!(out.contains("<p>body text</p>"));
        assert!(out.contains("body text"));
        // A plain quote stays a blockquote.
        let out = html("> just a quote\n");
        assert!(out.contains("<blockquote>"));
    }

    #[test]
    fn footnotes() {
        // Adjacent defs: each `<li>` ends at its own body.
        let out =
            html("Text[^1] and[^note].\n\n[^1]: First footnote.\n[^note]: Second **bold**.\n");
        assert!(
            out.contains(
                "<sup class=\"footnote-ref\" id=\"fnref-1\"><a href=\"#fn-1\">1</a></sup>"
            )
        );
        assert!(out.contains("class=\"footnotes\""));
        assert!(out.contains("<li id=\"fn-1\"><sup>1</sup> First footnote. <a"));
        assert!(!out.contains("First footnote.\n Second"));
        assert!(out.contains("<li id=\"fn-note\"><sup>note</sup> Second <strong>bold</strong>."));
        assert!(out.contains("href=\"#fnref-note\""));
    }

    #[test]
    fn images() {
        let out = html("![cat](pic%20one.png)\n");
        assert!(out.contains("<img src=\"pic%20one.png\" alt=\"cat\">"));
    }

    #[test]
    fn selection_clips() {
        let text = "first\n\nsecond para\n\nthird\n";
        let a = Analyzer::new().analyze(text);
        let sel = text.find("second").unwrap()..text.find("para").unwrap() + 4;
        let out = fragment(text, &a, sel.clone(), &ctx());
        assert_eq!(out, "<p>second para</p>\n");
        // A selection inside a paragraph clips to it.
        let sel = text.find("eco").unwrap()..text.find("eco").unwrap() + 3;
        let out = fragment(text, &a, sel, &ctx());
        assert_eq!(out, "<p>eco</p>\n");
    }

    #[test]
    fn plain_text_fallback() {
        let text = "**bold** `code` %%secret%% [[Link]]\n";
        let a = Analyzer::new().analyze(text);
        let p = plain(text, &a, 0..text.len());
        assert_eq!(p, "bold code  Link");
    }

    #[test]
    fn export_document() {
        let out = export("# T\n\nbody %%hidden%%\n", "T", &ctx());
        assert!(out.starts_with("<!doctype html>"));
        assert!(out.contains("<title>T</title>"));
        assert!(out.contains("<style>"));
        assert!(out.contains("<h1>T</h1>"));
        assert!(!out.contains("hidden"));
    }

    #[test]
    fn wiki_link_targets_parse() {
        let links = parse("[[A]] [[B|x]]", |_| false);
        assert_eq!(links.len(), 2);
    }
}
