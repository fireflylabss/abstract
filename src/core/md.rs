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

/// `=={tint}…==` marker colour; a bare `==…==` reads as `Yellow`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tint {
    Red,
    Orange,
    #[default]
    Yellow,
    Green,
    Blue,
    Purple,
}

impl Tint {
    /// Lowercase `{name}` source form, the inverse of `tint_prefix`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }
}

/// A `==…==` highlight span, delimiters included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkSpan {
    pub range: Range<usize>,
    pub tint: Tint,
}

/// A `$…$` or `$$…$$` math span; `content` is the TeX source between the
/// delimiters and `block` marks a `$$` span that owns its line(s).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathSpan {
    pub range: Range<usize>,
    pub content: Range<usize>,
    pub block: bool,
}

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
    pub wiki_links: Vec<crate::core::links::WikiLink>,
    /// Footnote references and definitions, in buffer order.
    pub footnotes: Vec<crate::core::footnote::Footnote>,
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
    /// `==…==` marks, in buffer order; `tint` comes from an optional `{color}`
    /// prefix right after the opener.
    pub marks: Vec<MarkSpan>,
    /// `$…$`/`$$…$$` math spans, in buffer order.
    pub maths: Vec<MathSpan>,
    /// YAML (`---`) / TOML (`+++`) frontmatter ranges, in buffer order.
    pub metadata: Vec<Range<usize>>,
}

pub struct Analyzer {
    block: Parser,
    inline: Parser,
}

mod analyzer;
mod helpers;
#[allow(unused_imports)]
pub use helpers::*;
#[cfg(test)]
mod tests;
