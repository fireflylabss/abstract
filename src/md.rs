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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Body,
    Heading(u8),
    Code,
    Quote,
    Rule,
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

#[derive(Default)]
pub struct Analysis {
    pub flags: Vec<u16>,
    /// Buffer lines without their `\n`, in order; always at least one.
    pub lines: Vec<(Range<usize>, Kind)>,
    /// Sorted by `hidden.start`.
    pub conceals: Vec<Conceal>,
    /// `[[wiki-links]]` found outside code spans/blocks.
    pub wiki_links: Vec<crate::links::WikiLink>,
    /// Task-list markers, in buffer order.
    pub tasks: Vec<Task>,
    /// List items, in line order.
    pub items: Vec<Item>,
    /// Images, in buffer order.
    pub images: Vec<ImageRef>,
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
            tasks: Vec::new(),
            items: Vec::new(),
            images: Vec::new(),
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

        if !inline_ranges.is_empty()
            && self.inline.set_included_ranges(&inline_ranges).is_ok()
            && let Some(tree) = self.inline.parse(source, None)
        {
            walk(&mut tree.walk(), |node| {
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
            });
        }

        // Wiki-links last: zero prior flags, drop overlapping conceals
        // (tree-sitter may have read `[[x]]` as a shortcut_link), then mark
        // the visible part LINK and conceal the bracket syntax around it.
        out.wiki_links = crate::links::parse(text, |o| {
            out.flags[o] & CODE != 0 || matches!(out.lines[out.line_of(o)].1, Kind::Code)
        });
        for l in std::mem::take(&mut out.wiki_links) {
            for f in &mut out.flags[l.range.clone()] {
                *f = 0;
            }
            out.conceals
                .retain(|c| c.hidden.end <= l.range.start || c.hidden.start >= l.range.end);
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

        out.conceals.retain(|c| !c.hidden.is_empty());
        out.conceals.sort_by_key(|c| c.hidden.start);
        out.images.sort_by_key(|i| i.range.start);
        out
    }
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
