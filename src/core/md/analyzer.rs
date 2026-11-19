use super::*;
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
            marks: Vec::new(),
            maths: Vec::new(),
            metadata: Vec::new(),
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
                        for (r, flag) in crate::core::code::highlight(&lang, &text[c.clone()]) {
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
                "minus_metadata" | "plus_metadata" => {
                    out.metadata.push(range.clone());
                    out.mark(range, MUTED);
                    false
                }
                "html_block" | "link_reference_definition" | "pipe_table_delimiter_row" => {
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
                "html_tag" => {
                    out.mark(range, MUTED);
                    return false;
                }
                // `latex_block` falls through: math gets its delimiters and
                // flags from the math pass in `analyze`, which applies the
                // money guards the inline grammar lacks.
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
        out.wiki_links = crate::core::links::parse(text, |o| {
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
        out.footnotes = crate::core::footnote::parse(text, |o| {
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
        for r in comments {
            let (open, close) = (r.start..r.start + 2, r.end - 2..r.end);
            out.mark(open.end..close.start, MUTED | ITALIC);
            out.mark(open.clone(), MARK);
            out.mark(close.clone(), MARK);
            out.conceal(r.clone(), open);
            out.conceal(r, close);
        }
        for r in marks {
            let (mut open, close) = (r.start..r.start + 2, r.end - 2..r.end);
            // `=={red}x==`: the `{color}` conceals with the opener.
            let (tint, prefix) = tint_prefix(&text[open.end..close.start]);
            open.end += prefix;
            out.mark(open.end..close.start, HIGHLIGHT);
            out.mark(open.clone(), MARK);
            out.mark(close.clone(), MARK);
            out.conceal(r.clone(), open);
            out.conceal(r.clone(), close);
            out.marks.push(MarkSpan { range: r, tint });
        }

        // tree-sitter-markdown cuts a table short when its last row has
        // only empty cells, which is what Tab in the last cell appends.
        for t in &mut out.tables {
            while let Some((line, kind)) = out.lines.get(t.end) {
                let row = text[line.clone()].trim();
                if *kind != Kind::Body || !row.starts_with('|') {
                    break;
                }
                out.lines[t.end].1 = Kind::Table;
                t.end += 1;
            }
        }

        // Table cells are padded to their source width, so an escaped
        // pipe's backslash must stay visible to keep columns aligned.
        let mut escapes = Vec::new();
        for t in out.tables.clone() {
            for ix in t {
                let line = out.lines[ix].0.clone();
                let b = text.as_bytes();
                let mut i = line.start;
                while i < line.end {
                    if b[i] == b'\\' {
                        if b.get(i + 1) == Some(&b'|') && out.flags[i] & CODE == 0 {
                            escapes.push(i..i + 1);
                        }
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
        drop_overlapping(&mut out.conceals, &escapes);

        // Math: `$…$` inline, `$$…$$` blocks. Delimiters conceal by the
        // same owner/hidden rule as every other construct; earlier syntax
        // (comments, marks, links) loses its conceals inside a math span.
        let mskip = |o: usize| {
            out.flags[o] & CODE != 0
                || matches!(out.lines[out.line_of(o)].1, Kind::Code | Kind::Table)
        };
        let maths = math_spans(text, &mskip);
        let spans: Vec<Range<usize>> = maths.iter().map(|m| m.range.clone()).collect();
        drop_overlapping(&mut out.conceals, &spans);
        for m in maths {
            for f in &mut out.flags[m.range.clone()] {
                *f = 0;
            }
            let open = m.range.start..m.content.start;
            let close = m.content.end..m.range.end;
            out.mark(open.clone(), MARK);
            out.mark(close.clone(), MARK);
            out.conceal(m.range.clone(), open);
            out.conceal(m.range.clone(), close);
            out.maths.push(m);
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
