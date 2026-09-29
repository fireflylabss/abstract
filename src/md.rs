//! Markdown analysis for the live editor: per-byte inline flags, per-line block
//! kinds, and concealable syntax (delimiters hidden unless the selection
//! touches the construct that owns them).

use std::ops::Range;

use tree_sitter::{Node, Parser, Range as TsRange, TreeCursor};

pub const BOLD: u16 = 1;
pub const ITALIC: u16 = 2;
pub const UNDERLINE: u16 = 4;
pub const STRIKE: u16 = 8;
pub const CODE: u16 = 16;
pub const LINK: u16 = 32;
pub const MARK: u16 = 64;
pub const MUTED: u16 = 128;
pub const TASK: u16 = 256;
pub const DONE: u16 = 512;
pub const KEYWORD: u16 = 1024;
pub const STRING: u16 = 2048;
pub const COMMENT: u16 = 4096;
pub const NUMBER: u16 = 8192;
/// `==highlighted==` text.
pub const HIGHLIGHT: u16 = 16384;
/// A callout's title (or its type word when it has no title).
pub const CALLOUT: u16 = 32768;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Body,
    Heading(u8),
    Code,
    Quote,
    Rule,
    Table,
}

/// A `- [ ]` / `- [x]` list-item marker; clicking toggles the task.
pub struct Task {
    pub marker: Range<usize>,
    pub checked: bool,
}

/// `owner` reveals `hidden` when the selection touches it (inclusive ends).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conceal {
    pub owner: Range<usize>,
    pub hidden: Range<usize>,
}

/// What a list-item marker renders as: a painted dot (concealed `-`/`+`/`*`),
/// the literal `1.`/`1)` text, or a clickable checkbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bullet {
    Dot,
    Ordered,
    Task { checked: bool },
}

/// A list item line: `marker` is the concealed range (marker + trailing
/// spaces; the raw node range for ordered items).
pub struct Item {
    pub line: usize,
    pub depth: u8,
    pub marker: Range<usize>,
    pub bullet: Bullet,
}

/// `![alt](src)` or an Obsidian-style `![[src]]` embed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    /// Whole construct, `!` included.
    pub range: Range<usize>,
    /// The raw destination text (may be `<…>`-wrapped or percent-encoded).
    pub src: Range<usize>,
    /// `![[…]]`: resolved against the space as well as the note's folder.
    pub embed: bool,
}

/// Callout colour family; unknown `[!types]` read as `Note`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Note,
    Tip,
    Warning,
    Danger,
}

/// A `> [!type] Title` block quote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callout {
    /// Line indices, end exclusive.
    pub lines: Range<usize>,
    pub tone: Tone,
}

/// The `[!type]` head of a callout line, offsets relative to the line.
struct CalloutHead {
    tone: Tone,
    /// `[!type]` plus an optional `+`/`-` fold sign.
    marker: Range<usize>,
    word: Range<usize>,
    /// Where the title (or the rest of the line) starts.
    body: usize,
}

fn callout_head(line: &str) -> Option<CalloutHead> {
    let start = line.len() - line.trim_start_matches([' ', '>']).len();
    if !line[..start].contains('>') {
        return None;
    }
    let rest = line[start..].strip_prefix("[!")?;
    let close = rest.find(']')?;
    let word = &rest[..close];
    if word.is_empty() || !word.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return None;
    }
    let tone = match word.to_lowercase().as_str() {
        "tip" | "hint" | "success" | "check" | "done" | "important" => Tone::Tip,
        "warning" | "caution" | "attention" | "question" | "help" | "faq" => Tone::Warning,
        "danger" | "error" | "bug" | "failure" | "fail" | "missing" => Tone::Danger,
        _ => Tone::Note,
    };
    let word = start + 2..start + 2 + close;
    let mut end = word.end + 1;
    if matches!(line.as_bytes().get(end), Some(b'+' | b'-')) {
        end += 1;
    }
    let body = end + (line.len() - end - line[end..].trim_start_matches(' ').len());
    Some(CalloutHead {
        tone,
        marker: start..end,
        word,
        body,
    })
}

#[derive(Default)]
pub struct Analysis {
    pub flags: Vec<u16>,
    /// Buffer lines without their `\n`, in order; always at least one.
    pub lines: Vec<(Range<usize>, Kind)>,
    /// Sorted by `hidden.start`.
    pub conceals: Vec<Conceal>,
    /// `[[wiki-links]]` found outside code spans/blocks.
    pub wiki_links: Vec<crate::links::WikiLink>,
    /// Footnote references and definitions, in buffer order.
    pub footnotes: Vec<crate::footnote::Footnote>,
    /// Task-list markers, in buffer order.
    pub tasks: Vec<Task>,
    /// List items, in line order.
    pub items: Vec<Item>,
    /// Images, in buffer order.
    pub images: Vec<ImageRef>,
    /// Callout blocks, in line order.
    pub callouts: Vec<Callout>,
    /// Pipe tables as line indices (end exclusive), in line order.
    pub tables: Vec<Range<usize>>,
}

pub struct Analyzer {
    block: Parser,
    inline: Parser,
}

impl Analyzer {
    pub fn new() -> Self {
        let mut block = Parser::new();
        block
            .set_language(&tree_sitter_md::LANGUAGE.into())
            .expect("tree-sitter-md block grammar");
        let mut inline = Parser::new();
        inline
            .set_language(&tree_sitter_md::INLINE_LANGUAGE.into())
            .expect("tree-sitter-md inline grammar");
        Self { block, inline }
    }

    pub fn analyze(&mut self, text: &str) -> Analysis {
        let len = text.len();
        let mut lines = Vec::new();
        let mut start = 0;
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                lines.push((start..i, Kind::Body));
                start = i + 1;
            }
        }
        lines.push((start..len, Kind::Body));

        let mut out = Analysis {
            flags: vec![0; len],
            lines,
            conceals: Vec::new(),
            wiki_links: Vec::new(),
            footnotes: Vec::new(),
            tasks: Vec::new(),
            items: Vec::new(),
            images: Vec::new(),
            callouts: Vec::new(),
            tables: Vec::new(),
        };
        if len == 0 {
            return out;
        }
        // tree-sitter-md errors on a last line without a line ending (the line
        // being typed at EOF), so parse a newline-terminated copy.
        let owned;
        let source: &str = if text.ends_with('\n') {
            text
        } else {
            owned = format!("{text}\n");
            &owned
        };
        let Some(tree) = self.block.parse(source, None) else {
            return out;
        };

        let mut inline_ranges: Vec<TsRange> = Vec::new();
        walk(&mut tree.walk(), |node| {
            let range = node.start_byte().min(len)..node.end_byte().min(len);
            match node.kind() {
                "atx_heading" => {
                    let level = heading_level(node);
                    let line = out.line_of(range.start);
                    out.lines[line].1 = Kind::Heading(level);
                    let line_range = out.lines[line].0.clone();
                    let mut cursor = node.walk();
                    let marker_start = range.start;
                    let content_start = node
                        .children(&mut cursor)
                        .find(|c| c.kind() == "inline")
                        .map_or(line_range.end, |c| c.start_byte().min(len));
                    out.mark(marker_start..content_start, MARK);
                    out.conceal(line_range, marker_start..content_start);
                    true
                }
                "setext_heading" => {
                    let level = heading_level(node);
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        let r = child.start_byte().min(len)..child.end_byte().min(len);
                        if child.kind() == "paragraph" {
                            out.set_kind(r, Kind::Heading(level));
                        } else {
                            out.mark(r, MUTED);
                        }
                    }
                    true
                }
                "fenced_code_block" | "indented_code_block" => {
                    out.set_kind(range.clone(), Kind::Code);
                    let mut cursor = node.walk();
                    let mut lang = String::new();
                    let mut content: Option<Range<usize>> = None;
                    for child in node.children(&mut cursor) {
                        let r = child.start_byte().min(len)..child.end_byte().min(len);
                        match child.kind() {
                            "info_string" => {
                                lang = text[r.clone()]
                                    .split_whitespace()
                                    .next()
                                    .unwrap_or_default()
                                    .to_lowercase();
                                out.mark(r.clone(), MUTED);
                                out.conceal(range.clone(), r);
                            }
                            "fenced_code_block_delimiter" => {
                                out.mark(r.clone(), MUTED);
                                // Fence text disappears (the line stays as block
                                // padding) until the selection enters the block.
                                out.conceal(range.clone(), r);
                            }
                            "code_fence_content" => {
                                content = Some(r);
                            }
                            _ => {}
                        }
                    }
                    if let Some(c) = content
                        && node.kind() == "fenced_code_block"
                        && c.end > c.start
                    {
                        for (r, flag) in crate::code::highlight(&lang, &text[c.clone()]) {
                            out.mark(c.start + r.start..(c.start + r.end).min(len), flag);
                        }
                    }
                    false
                }
                "block_quote" => {
                    let first = out.line_of(range.start);
                    let last = out.line_of(range.end.saturating_sub(1).max(range.start));
                    let nested = out
                        .callouts
                        .last()
                        .is_some_and(|c| c.lines.contains(&first));
                    if !nested && let Some(h) = callout_head(&text[out.lines[first].0.clone()]) {
                        out.callouts.push(Callout {
                            lines: first..last + 1,
                            tone: h.tone,
                        });
                    }
                    // Only the first line gets a `block_quote_marker` node;
                    // later lines carry their `>` in a continuation.
                    let outer = std::iter::successors(node.parent(), |p| p.parent())
                        .all(|p| p.kind() != "block_quote");
                    for line in (first + 1..=last).filter(|_| outer) {
                        let r = out.lines[line].0.clone();
                        let lead = text[r.clone()].len()
                            - text[r.clone()].trim_start_matches([' ', '>']).len();
                        if text[r.start..r.start + lead].contains('>') {
                            out.mark(r.start..r.start + lead, MARK);
                            out.conceal(r.clone(), r.start..r.start + lead);
                            if out.lines[line].1 == Kind::Body {
                                out.lines[line].1 = Kind::Quote;
                            }
                        }
                    }
                    true
                }
                "block_quote_marker" => {
                    let line = out.line_of(range.start);
                    if out.lines[line].1 == Kind::Body {
                        out.lines[line].1 = Kind::Quote;
                    }
                    out.mark(range.clone(), MARK);
                    out.conceal(out.lines[line].0.clone(), range);
                    false
                }
                "list_marker_minus"
                | "list_marker_plus"
                | "list_marker_star"
                | "list_marker_dot"
                | "list_marker_parenthesis" => {
                    let ordered =
                        matches!(node.kind(), "list_marker_dot" | "list_marker_parenthesis");
                    let line = out.line_of(range.start);
                    let line_range = out.lines[line].0.clone();
                    let depth = ((range.start - line_range.start) / 2).min(6) as u8;
                    // The node covers the marker only; swallow trailing spaces
                    // so the painted bullet doesn't leave a gap.
                    let mut marker = range.clone();
                    while text.as_bytes().get(marker.end) == Some(&b' ') {
                        marker.end += 1;
                    }
                    out.mark(range, MARK);
                    if !ordered {
                        out.conceal(line_range, marker.clone());
                    }
                    out.items.push(Item {
                        line,
                        depth,
                        marker,
                        bullet: if ordered {
                            Bullet::Ordered
                        } else {
                            Bullet::Dot
                        },
                    });
                    false
                }
                "task_list_marker_checked" | "task_list_marker_unchecked" => {
                    let checked = node.kind() == "task_list_marker_checked";
                    let line = out.line_of(range.start);
                    let line_range = out.lines[line].0.clone();
                    let mut marker = range.clone();
                    while text.as_bytes().get(marker.end) == Some(&b' ') {
                        marker.end += 1;
                    }
                    out.mark(range.clone(), MARK | TASK);
                    out.conceal(line_range.clone(), marker.clone());
                    out.tasks.push(Task {
                        marker: range.clone(),
                        checked,
                    });
                    match out.items.last_mut() {
                        Some(i) if i.line == line => i.bullet = Bullet::Task { checked },
                        _ => {
                            let ws = text[line_range].bytes().take_while(|b| *b == b' ').count();
                            out.items.push(Item {
                                line,
                                depth: (ws / 2).min(6) as u8,
                                marker,
                                bullet: Bullet::Task { checked },
                            });
                        }
                    }
                    if checked {
                        let end = out.lines[out.line_of(range.start)].0.end;
                        out.mark(range.end..end, DONE);
                    }
                    false
                }
                "thematic_break" => {
                    out.set_kind(range.clone(), Kind::Rule);
                    out.mark(range.clone(), MUTED);
                    let line = out.lines[out.line_of(range.start)].0.clone();
                    out.conceal(line.clone(), line);
                    false
                }
                "pipe_table" => {
                    out.set_kind(range.clone(), Kind::Table);
                    let first = out.line_of(range.start);
                    let last = out.line_of(range.end.saturating_sub(1).max(range.start));
                    out.tables.push(first..last + 1);
                    true
                }
                "html_block"
                | "minus_metadata"
                | "plus_metadata"
                | "link_reference_definition"
                | "pipe_table_delimiter_row" => {
                    out.mark(range, MUTED);
                    false
                }
                "pipe_table_cell" => {
                    if node
                        .parent()
                        .is_some_and(|p| p.kind() == "pipe_table_header")
                    {
                        out.mark(range, BOLD);
                    }
                    inline_ranges.push(node.range());
                    false
                }
                "inline" => {
                    inline_ranges.push(node.range());
                    false
                }
                _ => true,
            }
        });

        // One parse per inline node: a single parse over thousands of
        // included ranges scales poorly with the range count.
        let mut visit = |node: Node<'_>| -> bool {
            let range = node.start_byte().min(len)..node.end_byte().min(len);
            match node.kind() {
                "emphasis" => out.mark(range, ITALIC),
                "strong_emphasis" => {
                    // `__x__` underlines; `**x**` bolds.
                    let flag = if text.as_bytes().get(range.start) == Some(&b'_') {
                        UNDERLINE
                    } else {
                        BOLD
                    };
                    out.mark(range, flag);
                }
                "strikethrough" => out.mark(range, STRIKE),
                "emphasis_delimiter" => {
                    let owner = node.parent().map_or(range.clone(), outermost_same_kind);
                    out.mark(range.clone(), MARK);
                    out.conceal(owner, range);
                    return false;
                }
                "code_span" => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "code_span_delimiter" {
                            let r = child.start_byte().min(len)..child.end_byte().min(len);
                            out.conceal(range.clone(), r);
                        }
                    }
                    out.mark(range, CODE);
                    return false;
                }
                "inline_link"
                | "full_reference_link"
                | "collapsed_reference_link"
                | "shortcut_link"
                | "image" => {
                    let keep = if node.kind() == "image" {
                        ("image_description", MUTED | ITALIC)
                    } else {
                        ("link_text", LINK)
                    };
                    let mut pos = range.start;
                    let mut cursor = node.walk();
                    let mut visible = None;
                    let mut dest = None;
                    for child in node.children(&mut cursor) {
                        let r = child.start_byte().min(len)..child.end_byte().min(len);
                        if visible.is_none()
                            && (child.kind() == keep.0 || child.kind() == "link_label")
                        {
                            visible = Some(r);
                        } else if child.kind() == "link_destination" {
                            dest = Some(r);
                        }
                    }
                    if node.kind() == "image"
                        && let Some(src) = dest
                    {
                        out.images.push(ImageRef {
                            range: range.clone(),
                            src,
                            embed: false,
                        });
                    }
                    match visible {
                        Some(v) => {
                            out.mark(v.clone(), keep.1);
                            out.mark(pos..v.start, MARK);
                            out.conceal(range.clone(), pos..v.start);
                            pos = v.end;
                            out.mark(pos..range.end, MARK);
                            out.conceal(range.clone(), pos..range.end);
                        }
                        None => out.mark(range, MUTED),
                    }
                    return false;
                }
                "uri_autolink" | "email_autolink" => {
                    out.mark(range, LINK);
                    return false;
                }
                "backslash_escape" => {
                    out.mark(range.start..range.start + 1, MARK);
                    out.conceal(range.clone(), range.start..range.start + 1);
                    return false;
                }
                "html_tag" | "latex_block" => {
                    out.mark(range, MUTED);
                    return false;
                }
                _ => {}
            }
            true
        };
        for r in &inline_ranges {
            if self
                .inline
                .set_included_ranges(std::slice::from_ref(r))
                .is_err()
            {
                continue;
            }
            if let Some(tree) = self.inline.parse(source, None) {
                walk(&mut tree.walk(), &mut visit);
            }
        }

        // Wiki-links last: zero prior flags, drop overlapping conceals
        // (tree-sitter may have read `[[x]]` as a shortcut_link), then mark
        // the visible part LINK and conceal the bracket syntax around it.
        out.wiki_links = crate::links::parse(text, |o| {
            out.flags[o] & CODE != 0 || matches!(out.lines[out.line_of(o)].1, Kind::Code)
        });
        let spans: Vec<Range<usize>> = out.wiki_links.iter().map(|l| l.range.clone()).collect();
        drop_overlapping(&mut out.conceals, &spans);
        for l in std::mem::take(&mut out.wiki_links) {
            for f in &mut out.flags[l.range.clone()] {
                *f = 0;
            }
            if l.range.start > 0
                && text.as_bytes()[l.range.start - 1] == b'!'
                && is_image_path(&text[l.target.clone()])
            {
                let range = l.range.start - 1..l.range.end;
                out.mark(l.target.clone(), MUTED | ITALIC);
                out.mark(range.start..l.target.start, MARK);
                out.conceal(range.clone(), range.start..l.target.start);
                out.mark(l.target.end..range.end, MARK);
                out.conceal(range.clone(), l.target.end..range.end);
                out.images.push(ImageRef {
                    range,
                    src: l.target.clone(),
                    embed: true,
                });
                continue;
            }
            let visible = l.alias.clone().unwrap_or_else(|| l.target.clone());
            out.mark(visible.clone(), LINK);
            let mut pre = l.range.start..visible.start;
            if l.alias.is_some() {
                pre = l.target.end..visible.start;
                out.mark(l.range.start..l.target.end, MARK);
                out.conceal(l.range.clone(), l.range.start..l.target.end);
            }
            out.mark(pre.clone(), MARK);
            out.conceal(l.range.clone(), pre);
            out.mark(visible.end..l.range.end, MARK);
            out.conceal(l.range.clone(), visible.end..l.range.end);
            out.wiki_links.push(l);
        }

        // Footnotes show as `[label]`: the `^` is concealed, the label is a
        // LINK. A one-word definition parses as a link reference definition,
        // which mutes its whole line.
        out.footnotes = crate::footnote::parse(text, |o| {
            out.flags[o] & CODE != 0 || matches!(out.lines[out.line_of(o)].1, Kind::Code)
        });
        let spans: Vec<Range<usize>> = out.footnotes.iter().map(|f| f.range.clone()).collect();
        drop_overlapping(&mut out.conceals, &spans);
        for f in out.footnotes.clone() {
            for fl in &mut out.flags[f.range.clone()] {
                *fl = 0;
            }
            if f.def {
                let line = out.lines[out.line_of(f.range.start)].0.clone();
                if out.flags.get(f.range.end).is_some_and(|fl| fl & MUTED != 0) {
                    for fl in &mut out.flags[f.range.end..line.end] {
                        *fl &= !MUTED;
                    }
                }
            }
            let caret = f.range.start + 1..f.label.start;
            out.mark(f.range.start..f.label.start, MARK);
            out.conceal(f.range.clone(), caret);
            out.mark(f.label.clone(), LINK);
            out.mark(f.label.end..f.range.end, MARK);
        }

        let skip = |o: usize| out.flags[o] & CODE != 0 || out.lines[out.line_of(o)].1 == Kind::Code;
        let comments = delimited(text, b'%', true, &skip);
        let marks = delimited(text, b'=', false, &skip);
        for (r, flag) in comments
            .into_iter()
            .map(|r| (r, MUTED | ITALIC))
            .chain(marks.into_iter().map(|r| (r, HIGHLIGHT)))
        {
            let (open, close) = (r.start..r.start + 2, r.end - 2..r.end);
            out.mark(open.end..close.start, flag);
            out.mark(open.clone(), MARK);
            out.mark(close.clone(), MARK);
            out.conceal(r.clone(), open);
            out.conceal(r, close);
        }

        for t in out.tables.clone() {
            for ix in t {
                let line = out.lines[ix].0.clone();
                let b = text.as_bytes();
                let mut i = line.start;
                while i < line.end {
                    if b[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if b[i] == b'|' && out.flags[i] & CODE == 0 {
                        out.flags[i] |= MUTED;
                    }
                    i += 1;
                }
            }
        }

        let heads: Vec<(Range<usize>, CalloutHead)> = out
            .callouts
            .iter()
            .filter_map(|c| {
                let line = out.lines[c.lines.start].0.clone();
                callout_head(&text[line.clone()]).map(|h| (line, h))
            })
            .collect();
        let markers: Vec<Range<usize>> = heads
            .iter()
            .map(|(line, h)| line.start + h.marker.start..line.start + h.marker.end)
            .collect();
        // tree-sitter may have read `[!type]` as a shortcut link.
        drop_overlapping(&mut out.conceals, &markers);
        for (line, h) in heads {
            let at = |r: Range<usize>| line.start + r.start..line.start + r.end;
            let marker = at(h.marker.clone());
            for f in &mut out.flags[marker.clone()] {
                *f = 0;
            }
            if h.body < line.len() {
                let hidden = marker.start..line.start + h.body;
                out.mark(hidden.clone(), MARK);
                out.conceal(line.clone(), hidden);
                out.mark(line.start + h.body..line.end, CALLOUT | BOLD);
            } else {
                let word = at(h.word);
                out.mark(marker.start..word.start, MARK);
                out.conceal(line.clone(), marker.start..word.start);
                out.mark(word.end..marker.end, MARK);
                out.conceal(line.clone(), word.end..marker.end);
                out.mark(word, CALLOUT | BOLD);
            }
        }

        out.conceals.retain(|c| !c.hidden.is_empty());
        out.conceals.sort_by_key(|c| c.hidden.start);
        out.images.sort_by_key(|i| i.range.start);
        out
    }
}

/// Remove conceals whose hidden range overlaps any of `spans`, which are
/// sorted and disjoint.
fn drop_overlapping(conceals: &mut Vec<Conceal>, spans: &[Range<usize>]) {
    if spans.is_empty() {
        return;
    }
    conceals.retain(|c| {
        let i = spans.partition_point(|s| s.end <= c.hidden.start);
        spans.get(i).is_none_or(|s| s.start >= c.hidden.end)
    });
}

pub const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tif", "tiff",
];

/// Newlines to insert at the end of `before` so a new block starts after a
/// blank line (or at the top of the document).
pub fn block_lead(before: &str) -> &'static str {
    if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    }
}

/// `line` with its heading/list/quote prefix replaced by `prefix` (indent
/// kept). Applying the prefix a line already has removes it.
pub fn set_block_prefix(line: &str, prefix: &str) -> String {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let (pad, rest) = line.split_at(indent);
    let hashes = rest.bytes().take_while(|&b| b == b'#').count();
    let ordered = rest.bytes().take_while(u8::is_ascii_digit).count();
    let old = if (1..=6).contains(&hashes) && rest[hashes..].starts_with(' ') {
        hashes + 1
    } else if ordered > 0
        && (rest[ordered..].starts_with(". ") || rest[ordered..].starts_with(") "))
    {
        ordered + 2
    } else {
        [
            "- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "- ", "* ", "+ ", "> ",
        ]
        .iter()
        .find(|p| rest.starts_with(**p))
        .map_or(0, |p| p.len())
    };
    let (had, body) = rest.split_at(old);
    let same = had == prefix || (prefix == "1. " && ordered > 0 && old == ordered + 2);
    let new = if same { "" } else { prefix };
    format!("{pad}{new}{body}")
}

/// `ch ch … ch ch` spans (`==mark==`, `%%comment%%`), delimiters included.
/// Runs of more than two `ch` are not delimiters; an opener needs text right
/// after it and a closer right before it. `multiline` spans may cross lines.
fn delimited(
    text: &str,
    ch: u8,
    multiline: bool,
    skip: &impl Fn(usize) -> bool,
) -> Vec<Range<usize>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\n' && !multiline {
            open = None;
        }
        if b[i] != ch {
            i += 1;
            continue;
        }
        let n = b[i..].iter().take_while(|&&c| c == ch).count();
        if n != 2 || skip(i) || (i > 0 && b[i - 1] == b'\\') {
            i += n;
            continue;
        }
        let blank = |c: Option<&u8>| matches!(c, None | Some(b' ' | b'\t' | b'\n'));
        match open {
            Some(o) if i > o + 2 && (multiline || !blank(b.get(i - 1))) => {
                out.push(o..i + 2);
                open = None;
            }
            None if multiline || !blank(b.get(i + 2)) => open = Some(i),
            _ => {}
        }
        i += 2;
    }
    out
}

/// Turn the lines of `block` into a `> [!note]` callout, or back into plain
/// lines when the first one already is a callout head.
pub fn toggle_callout(block: &str) -> String {
    let mut lines = block.split('\n');
    let first = lines.next().unwrap_or_default();
    let unquote = |l: &str| {
        let t = l.trim_start_matches(' ');
        t.strip_prefix("> ")
            .or_else(|| t.strip_prefix('>'))
            .unwrap_or(t)
            .to_string()
    };
    let quote = |l: &str| {
        if l.trim_start().starts_with('>') {
            l.to_string()
        } else {
            format!("> {l}")
        }
    };
    let head = callout_head(first);
    let lines = lines.map(|l| if head.is_some() { unquote(l) } else { quote(l) });
    let first = match &head {
        Some(h) => first[h.body..].to_string(),
        None => format!("> [!note] {}", unquote(first)),
    };
    std::iter::once(first)
        .chain(lines)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `path`'s extension is an image format the editor can display.
pub fn is_image_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| IMAGE_EXTS.contains(&x.to_ascii_lowercase().as_str()))
}

impl Analysis {
    pub fn line_of(&self, offset: usize) -> usize {
        self.lines
            .partition_point(|(r, _)| r.start <= offset)
            .saturating_sub(1)
    }

    fn mark(&mut self, range: Range<usize>, flag: u16) {
        for f in &mut self.flags[range] {
            *f |= flag;
        }
    }

    fn conceal(&mut self, owner: Range<usize>, hidden: Range<usize>) {
        self.conceals.push(Conceal { owner, hidden });
    }

    fn set_kind(&mut self, range: Range<usize>, kind: Kind) {
        let first = self.line_of(range.start);
        let last = self.line_of(range.end.saturating_sub(1).max(range.start));
        for line in &mut self.lines[first..=last] {
            line.1 = kind;
        }
    }

    /// Visible sub-ranges of `line` after concealing syntax the selection does
    /// not touch.
    pub fn visible(&self, line: Range<usize>, sel: &Range<usize>) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut pos = line.start;
        let first = self
            .conceals
            .partition_point(|c| c.hidden.end <= line.start);
        for c in &self.conceals[first..] {
            if c.hidden.start >= line.end {
                break;
            }
            if c.owner.start <= sel.end && sel.start <= c.owner.end {
                continue;
            }
            let start = c.hidden.start.max(pos);
            if start > pos {
                out.push(pos..start);
            }
            pos = pos.max(c.hidden.end.min(line.end));
        }
        if pos < line.end || out.is_empty() {
            out.push(pos..line.end.max(pos));
        }
        out
    }
}

fn walk(cursor: &mut TreeCursor<'_>, mut f: impl FnMut(Node<'_>) -> bool) {
    loop {
        if f(cursor.node()) && cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

fn heading_level(heading: Node<'_>) -> u8 {
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

/// `~~x~~` parses as nested strikethrough nodes; the outer one owns all four
/// tildes so they reveal together.
fn outermost_same_kind(mut node: Node<'_>) -> Range<usize> {
    while let Some(parent) = node.parent() {
        if parent.kind() != node.kind() {
            break;
        }
        node = parent;
    }
    node.byte_range()
}

/// Continuation for Enter on a list/quote line: `(prefix byte len, next prefix)`.
pub fn list_prefix(line: &str) -> Option<(usize, String)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let pad = &line[..indent];
    for task in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] "] {
        if rest.starts_with(task) {
            return Some((indent + task.len(), format!("{pad}{}[ ] ", &task[..2])));
        }
    }
    for marker in ["- ", "* ", "+ ", "> "] {
        if rest.starts_with(marker) {
            return Some((indent + 2, format!("{pad}{marker}")));
        }
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && rest[digits..].starts_with(". ") {
        let n: u64 = rest[..digits].parse().ok()?;
        return Some((indent + digits + 2, format!("{pad}{}. ", n + 1)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(text: &str, cursor: usize) -> String {
        let a = Analyzer::new().analyze(text);
        let sel = cursor..cursor;
        a.lines
            .iter()
            .map(|(r, _)| {
                a.visible(r.clone(), &sel)
                    .into_iter()
                    .map(|v| &text[v])
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn emphasis_stays_inside_its_paragraph() {
        let t = "*a\n\nb*\n\n**c** or *d*\n";
        let a = Analyzer::new().analyze(t);
        let at = |s: &str| t.find(s).unwrap();
        assert_eq!(a.flags[at("a")] & ITALIC, 0);
        assert_eq!(a.flags[at("b")] & ITALIC, 0);
        assert_ne!(a.flags[at("c")] & BOLD, 0);
        assert_ne!(a.flags[at("d")] & ITALIC, 0);
    }

    #[test]
    fn block_lead_leaves_a_blank_line() {
        assert_eq!(block_lead(""), "");
        assert_eq!(block_lead("alpha"), "\n\n");
        assert_eq!(block_lead("alpha\n"), "\n");
        assert_eq!(block_lead("alpha\n\n"), "");
    }

    #[test]
    fn block_prefix_replaces_and_toggles() {
        assert_eq!(set_block_prefix("hello", "# "), "# hello");
        assert_eq!(set_block_prefix("## hello", "# "), "# hello");
        assert_eq!(set_block_prefix("# hello", "# "), "hello");
        assert_eq!(set_block_prefix("  - [x] a", "> "), "  > a");
        assert_eq!(set_block_prefix("3. a", "1. "), "a");
        assert_eq!(set_block_prefix("- a", "1. "), "1. a");
        assert_eq!(set_block_prefix("#tag", ""), "#tag");
        assert_eq!(set_block_prefix("### a", ""), "a");
    }

    #[test]
    fn images_are_collected_and_concealed() {
        let t = "a ![cat](img/cat%20x.png) b\n![[shot.PNG]] and [[note]]\n![x](<a b.jpg>)";
        let a = Analyzer::new().analyze(t);
        let srcs: Vec<(&str, bool)> = a
            .images
            .iter()
            .map(|i| (&t[i.src.clone()], i.embed))
            .collect();
        assert_eq!(
            srcs,
            [
                ("img/cat%20x.png", false),
                ("shot.PNG", true),
                ("<a b.jpg>", false)
            ]
        );
        assert_eq!(&t[a.images[0].range.clone()], "![cat](img/cat%20x.png)");
        assert_eq!(&t[a.images[1].range.clone()], "![[shot.PNG]]");
        // Embeds are not wiki-links; the plain link stays one.
        assert_eq!(a.wiki_links.len(), 1);
        assert_eq!(
            shown(t, t.len()),
            "a cat b\nshot.PNG and note\n![x](<a b.jpg>)"
        );
        assert!(is_image_path("x.JPEG") && !is_image_path("x.md"));
    }

    #[test]
    fn syntax_hidden_until_cursor_enters() {
        let t = "# Title\nx *i* **b** __u__ ~~s~~ `c` [l](http://a)";
        assert_eq!(shown(t, 0), "# Title\nx i b u s c l");
        // Cursor on the heading line reveals its marker only.
        assert_eq!(shown(t, 3), "# Title\nx i b u s c l");
        // Cursor on line 2 far from constructs: heading marker hidden.
        assert_eq!(shown(t, 9), "Title\nx i b u s c l");
        // Cursor touching `**b**` reveals exactly that construct.
        let b = t.find("**b").unwrap();
        assert_eq!(shown(t, b + 3), "Title\nx i **b** u s c l");
        let s = t.find("~~").unwrap();
        assert_eq!(shown(t, s + 5), "Title\nx i b u ~~s~~ c l");
    }

    #[test]
    fn flags_and_kinds() {
        let t = "## H\n*i* **b** __u__ ~~s~~";
        let a = Analyzer::new().analyze(t);
        assert_eq!(a.lines[0].1, Kind::Heading(2));
        let at = |s: &str| a.flags[t.find(s).unwrap()];
        assert!(at("i*") & ITALIC != 0);
        assert!(at("b*") & BOLD != 0);
        assert!(at("u_") & UNDERLINE != 0 && at("u_") & BOLD == 0);
        assert!(at("s~") & STRIKE != 0);
    }

    #[test]
    fn wiki_link_conceals_brackets() {
        let t = "see [[Foo|bar]] ok";
        let a = Analyzer::new().analyze(t);
        assert_eq!(a.wiki_links.len(), 1);
        let bar = t.find("bar").unwrap();
        // Cursor elsewhere: only the alias shows.
        assert_eq!(shown(t, 0), "see bar ok");
        assert!(a.flags[bar] & LINK != 0);
        // Cursor inside the link reveals the full syntax.
        assert_eq!(shown(t, bar), "see [[Foo|bar]] ok");
        // Plain target link shows the target.
        assert_eq!(shown("x [[Foo]] y", 0), "x Foo y");
    }

    #[test]
    fn task_items_flag_marker_and_done_text() {
        let t = "- [ ] todo\n- [x] done";
        let a = Analyzer::new().analyze(t);
        assert_eq!(a.tasks.len(), 2);
        assert!(!a.tasks[0].checked);
        assert!(a.tasks[1].checked);
        let at = |s: &str| a.flags[t.find(s).unwrap()];
        assert!(at("[ ]") & TASK != 0 && at("[ ]") & MARK != 0);
        assert!(at("[x]") & TASK != 0);
        assert!(at("done") & DONE != 0);
        assert!(at("todo") & DONE == 0);
    }

    #[test]
    fn list_items_track_depth_bullet_and_conceal() {
        let t = "- a\n  - b\n1. c\n- [x] d\n- [ ] e\n";
        let a = Analyzer::new().analyze(t);
        let bullets: Vec<_> = a.items.iter().map(|i| i.bullet).collect();
        assert_eq!(
            bullets,
            [
                Bullet::Dot,
                Bullet::Dot,
                Bullet::Ordered,
                Bullet::Task { checked: true },
                Bullet::Task { checked: false },
            ]
        );
        let depths: Vec<_> = a.items.iter().map(|i| i.depth).collect();
        assert_eq!(depths, [0, 1, 0, 0, 0]);
        // Concealed ranges cover the dash markers (with trailing spaces)
        // and the task boxes; the ordered marker stays visible.
        let hidden: Vec<_> = a.conceals.iter().map(|c| c.hidden.clone()).collect();
        assert!(hidden.contains(&(0..2)), "{hidden:?}");
        assert!(hidden.contains(&(6..8)), "{hidden:?}");
        let bx = t.find("[x]").unwrap();
        assert!(hidden.iter().any(|r| *r == (bx..bx + 4)), "{hidden:?}");
        let bu = t.find("[ ]").unwrap();
        assert!(hidden.iter().any(|r| *r == (bu..bu + 4)), "{hidden:?}");
        // `- [x] d` text still flagged DONE.
        let at = |s: &str| a.flags[t.find(s).unwrap()];
        assert!(at("[x]") & TASK != 0);
        assert!(at("d\n") & DONE != 0);
    }

    #[test]
    fn highlight_and_comments_conceal_delimiters() {
        let t = "a ==hi== b %%note%% c `==x==` == d ==\n=== e";
        let a = Analyzer::new().analyze(t);
        let at = |s: &str| a.flags[t.find(s).unwrap()];
        assert!(at("hi") & HIGHLIGHT != 0);
        assert!(at("note") & MUTED != 0 && at("note") & ITALIC != 0);
        assert!(at("x==") & HIGHLIGHT == 0);
        assert!(at(" d") & HIGHLIGHT == 0);
        assert_eq!(shown(t, t.len()), "a hi b note c ==x== == d ==\n=== e");
        let m = "%%\nsecret\n%% after";
        let a = Analyzer::new().analyze(m);
        assert!(a.flags[m.find("secret").unwrap()] & MUTED != 0);
        assert!(a.flags[m.find("after").unwrap()] & MUTED == 0);
    }

    #[test]
    fn callouts_take_tone_and_title() {
        let t = "> [!WARNING] Careful\n> body\n\n> [!tip]\n> x\n\n> plain";
        let a = Analyzer::new().analyze(t);
        assert_eq!(
            a.callouts,
            [
                Callout {
                    lines: 0..2,
                    tone: Tone::Warning
                },
                Callout {
                    lines: 3..5,
                    tone: Tone::Tip
                }
            ]
        );
        assert!(a.flags[t.find("Careful").unwrap()] & CALLOUT != 0);
        assert!(a.flags[t.find("tip").unwrap()] & CALLOUT != 0);
        assert_eq!(shown(t, 0), "> [!WARNING] Careful\nbody\n\ntip\nx\n\nplain");
    }

    #[test]
    fn callout_toggles() {
        assert_eq!(toggle_callout("a\n- b"), "> [!note] a\n> - b");
        assert_eq!(toggle_callout("> q"), "> [!note] q");
        assert_eq!(toggle_callout("> [!note] a\n> - b"), "a\n- b");
        assert_eq!(toggle_callout("> [!tip]"), "");
    }

    #[test]
    fn pipe_tables_are_table_lines() {
        let text = "intro\n\n| a | b\\|c |\n| - | - |\n| 1 | 2 |\n\nafter";
        let a = Analyzer::new().analyze(text);
        assert_eq!(a.tables, vec![2..5_usize]);
        let kinds: Vec<Kind> = a.lines.iter().map(|l| l.1).collect();
        assert_eq!(kinds[2..5], [Kind::Table; 3]);
        assert_eq!(kinds[6], Kind::Body);
        let header = a.lines[2].0.start;
        assert_ne!(a.flags[header] & MUTED, 0);
        assert_ne!(a.flags[header + 2] & BOLD, 0);
        let escaped = header + text[header..].find("\\|").unwrap() + 1;
        assert_eq!(a.flags[escaped] & MUTED, 0);
    }

    #[test]
    fn footnotes_render_as_bracketed_links() {
        let t = "Hi[^1] `[^2]`.\n\n[^1]: Source.";
        let a = Analyzer::new().analyze(t);
        assert_eq!(a.footnotes.len(), 2);
        assert!(a.footnotes[1].def);
        assert_eq!(shown(t, t.len()), "Hi[1] [^2].\n\n[1]: Source.");
        let label = t.find('1').unwrap();
        assert_ne!(a.flags[label] & LINK, 0);
        let body = t.find("Source").unwrap();
        assert_eq!(a.flags[body] & MUTED, 0);
        // The caret on the reference reveals its `^`.
        assert!(shown(t, 3).starts_with("Hi[^1]"));
    }

    #[test]
    fn heading_typed_at_eof() {
        let a = Analyzer::new().analyze("# A");
        assert_eq!(a.lines[0].1, Kind::Heading(1));
    }

    #[test]
    fn list_continuation() {
        assert_eq!(list_prefix("- item"), Some((2, "- ".into())));
        assert_eq!(list_prefix("  9. x"), Some((5, "  10. ".into())));
        assert_eq!(list_prefix("- [x] done"), Some((6, "- [ ] ".into())));
        assert_eq!(list_prefix("plain"), None);
    }
}
