use super::*;

/// Remove conceals whose hidden range overlaps any of `spans`, which are
/// sorted and disjoint.
pub(super) fn drop_overlapping(conceals: &mut Vec<Conceal>, spans: &[Range<usize>]) {
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
pub(super) fn delimited(
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

/// `%%…%%` hidden comments (delimiters included) — the HTML emitter drops
/// them so private text never leaves the app.
pub(crate) fn comment_spans(text: &str, skip: &impl Fn(usize) -> bool) -> Vec<Range<usize>> {
    delimited(text, b'%', true, skip)
}

/// The `{color}` prefix of a mark's content: `(tint, byte len)`. `(Yellow, 0)`
/// when absent — unknown names are literal text, not a colour spec.
pub(crate) fn tint_prefix(content: &str) -> (Tint, usize) {
    let Some(rest) = content.strip_prefix('{') else {
        return (Tint::Yellow, 0);
    };
    let Some(close) = rest.find('}') else {
        return (Tint::Yellow, 0);
    };
    let tint = match rest[..close].to_ascii_lowercase().as_str() {
        "red" => Tint::Red,
        "orange" => Tint::Orange,
        "yellow" => Tint::Yellow,
        "green" => Tint::Green,
        "blue" => Tint::Blue,
        "purple" => Tint::Purple,
        _ => return (Tint::Yellow, 0),
    };
    // A bare `=={color}==` leaves `{color}` as literal content.
    if rest.len() == close + 1 {
        return (Tint::Yellow, 0);
    }
    (tint, close + 2)
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

    pub(super) fn mark(&mut self, range: Range<usize>, flag: u16) {
        for f in &mut self.flags[range] {
            *f |= flag;
        }
    }

    pub(super) fn conceal(&mut self, owner: Range<usize>, hidden: Range<usize>) {
        self.conceals.push(Conceal { owner, hidden });
    }

    pub(super) fn set_kind(&mut self, range: Range<usize>, kind: Kind) {
        let first = self.line_of(range.start);
        let last = self.line_of(range.end.saturating_sub(1).max(range.start));
        for line in &mut self.lines[first..=last] {
            line.1 = kind;
        }
    }

    /// The `=={tint}…==` colour covering `offset`, if any.
    pub fn tint_at(&self, offset: usize) -> Option<Tint> {
        let i = self.marks.partition_point(|m| m.range.end <= offset);
        match self.marks.get(i) {
            Some(m) if m.range.start <= offset => Some(m.tint),
            _ => None,
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

    /// The math span covering `off`, if any.
    pub fn math_at(&self, off: usize) -> Option<&MathSpan> {
        let i = self.maths.partition_point(|m| m.range.end <= off);
        self.maths.get(i).filter(|m| m.range.start <= off)
    }

    /// The math spans overlapping `r`, in buffer order.
    pub fn maths_overlapping(&self, r: &Range<usize>) -> &[MathSpan] {
        let s = self.maths.partition_point(|m| m.range.end <= r.start);
        let e = self.maths.partition_point(|m| m.range.start < r.end);
        &self.maths[s..e]
    }

    /// Buffer range of the block the cursor's line belongs to, for focus-mode
    /// dimming. A heading or a list item stands alone and also owns the
    /// non-blank continuation lines below it; other lines group into the
    /// non-blank run (paragraph) around the cursor. On a blank line the block
    /// is that line itself.
    pub fn focus_block(&self, text: &str, cursor: usize) -> Range<usize> {
        let blank = |ix: usize| text[self.lines[ix].0.clone()].trim().is_empty();
        let item_at = |ix: usize| {
            let i = self.items.partition_point(|it| it.line < ix);
            self.items.get(i).is_some_and(|it| it.line == ix)
        };
        let unit_start = |ix: usize| matches!(self.lines[ix].1, Kind::Heading(_)) || item_at(ix);
        let ix = self.line_of(cursor.min(text.len()));
        if blank(ix) {
            return self.lines[ix].0.clone();
        }
        let mut start = ix;
        while start > 0 && !blank(start - 1) && !unit_start(start) {
            start -= 1;
        }
        let mut end = ix;
        while end + 1 < self.lines.len() && !blank(end + 1) && !unit_start(end + 1) {
            end += 1;
        }
        self.lines[start].0.start..self.lines[end].0.end
    }
}

pub(super) fn walk(cursor: &mut TreeCursor<'_>, mut f: impl FnMut(Node<'_>) -> bool) {
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

pub(super) fn heading_level(heading: Node<'_>) -> u8 {
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
pub(super) fn outermost_same_kind(mut node: Node<'_>) -> Range<usize> {
    while let Some(parent) = node.parent() {
        if parent.kind() != node.kind() {
            break;
        }
        node = parent;
    }
    node.byte_range()
}

/// `$…$` and `$$…$$` spans, delimiters included in `range`. A single `$`
/// opens only before a non-blank, non-digit byte and closes only after a
/// non-blank one that is not followed by a digit — `R$ 10`, `$ 20` and
/// `$x$5` are money, not math. `$$` may open before a newline
/// (`$$\n…\n$$`). `\$` escapes, an unmatched `$` stays literal, inline
/// math never crosses a line break, and `$$` inside an open `$…$` (and
/// vice versa) is literal text, not a nested delimiter.
pub(super) fn math_spans(text: &str, skip: &impl Fn(usize) -> bool) -> Vec<MathSpan> {
    let b = text.as_bytes();
    let blank = |c: Option<&u8>| matches!(c, None | Some(b' ' | b'\t' | b'\n'));
    let mut out = Vec::new();
    let mut open: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\n' && matches!(open, Some((_, 1))) {
            open = None;
        }
        if b[i] != b'$' {
            i += 1;
            continue;
        }
        let n = b[i..].iter().take_while(|&&c| c == b'$').count();
        if (n != 1 && n != 2) || skip(i) || (i > 0 && b[i - 1] == b'\\') {
            i += n;
            continue;
        }
        match open {
            Some((o, w)) if w == n => {
                // A closer follows a non-blank byte; a single `$` also
                // refuses a digit right after it.
                if i > o + w
                    && !matches!(b[i - 1], b' ' | b'\t')
                    && (n == 2 || !b.get(i + 1).is_some_and(u8::is_ascii_digit))
                {
                    let end = i + n;
                    out.push(MathSpan {
                        range: o..end,
                        content: o + w..i,
                        block: n == 2 && block_math(text, o, end),
                    });
                    open = None;
                }
                i += n;
            }
            // A `$$` inside `$…$` (or a `$` inside `$$…$$`) is literal
            // text: math never nests.
            Some(_) => i += n,
            None => {
                let next = b.get(i + n);
                let opens = if n == 1 {
                    !blank(next) && !next.is_some_and(u8::is_ascii_digit)
                } else {
                    match next {
                        Some(b'\n') => true,
                        Some(c) => !matches!(c, b' ' | b'\t') && !c.is_ascii_digit(),
                        None => false,
                    }
                };
                if opens {
                    open = Some((i, n));
                }
                i += n;
            }
        }
    }
    out
}

/// `o..e` (a `$$` span) owns its lines: only a `>` quote prefix before,
/// only blanks after.
pub(super) fn block_math(text: &str, o: usize, e: usize) -> bool {
    let b = text.as_bytes();
    let ls = text[..o].rfind('\n').map_or(0, |p| p + 1);
    let le = text[e..].find('\n').map_or(b.len(), |p| e + p);
    b[ls..o].iter().all(|c| matches!(c, b' ' | b'\t' | b'>'))
        && b[e..le].iter().all(|c| matches!(c, b' ' | b'\t'))
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
