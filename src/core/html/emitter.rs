use super::*;
impl<'a> Emitter<'a> {
    pub(super) fn new(text: &'a str, a: &'a Analysis, sel: Range<usize>, ctx: &'a Context<'a>) -> Self {
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

    pub(super) fn render(&mut self, out: &mut String) {
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

    pub(super) fn children(&mut self, out: &mut String, node: Node) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.block(out, child);
        }
    }

    pub(super) fn block(&mut self, out: &mut String, node: Node) {
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
    pub(super) fn inline_nodes(&mut self, out: &mut String, node: Node) {
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

    pub(super) fn quote(&mut self, out: &mut String, node: Node) {
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

    pub(super) fn list(&mut self, out: &mut String, node: Node) {
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

    pub(super) fn list_item(&mut self, out: &mut String, node: Node) {
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

    pub(super) fn code(&mut self, out: &mut String, node: Node, fenced: bool) {
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

    pub(super) fn table(&mut self, out: &mut String, node: Node) {
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
                    aligns = crate::core::table::aligns(&self.text[r.clone()]);
                }
                _ => {}
            }
        }
        let style = |col: usize| match aligns
            .get(col)
            .copied()
            .unwrap_or(crate::core::table::Align::None)
        {
            crate::core::table::Align::None => String::new(),
            crate::core::table::Align::Left => " style=\"text-align:left\"".to_string(),
            crate::core::table::Align::Center => " style=\"text-align:center\"".to_string(),
            crate::core::table::Align::Right => " style=\"text-align:right\"".to_string(),
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
    pub(super) fn def_bodies(&mut self, node: Node, nb: &Range<usize>) -> bool {
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

    pub(super) fn footnote_section(&mut self, out: &mut String) {
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

    pub(super) fn inline(&mut self, out: &mut String, range: Range<usize>) {
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
            styled(out, &self.text[i..j], sf, self.a.tint_at(i));
            i = j;
        }
    }

    /// The `i`-positioned image / wiki-link / footnote constructs.
    pub(super) fn special(&mut self, out: &mut String, i: usize) -> Option<usize> {
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
        if let Some(m) = at(&self.a.maths, i, |x| &x.range) {
            let (range, content, block) = (m.range.clone(), m.content.clone(), m.block);
            mathml(out, &self.text[content], block);
            return Some(range.end);
        }
        None
    }

    pub(super) fn special_start(&self, i: usize) -> bool {
        at(&self.a.images, i, |x| &x.range).is_some()
            || at(&self.a.wiki_links, i, |x| &x.range).is_some()
            || at(&self.a.footnotes, i, |x| &x.range).is_some()
            || at(&self.a.maths, i, |x| &x.range).is_some()
    }

    pub(super) fn hidden_end(&self, i: usize) -> Option<usize> {
        let idx = self.hidden.partition_point(|r| r.end <= i);
        self.hidden.get(idx).filter(|r| r.start <= i).map(|r| r.end)
    }

    /// `![alt](src)` / `![[src]]` — relative sources stay relative.
    pub(super) fn image(&mut self, out: &mut String, range: Range<usize>, src: Range<usize>, embed: bool) {
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
    pub(super) fn wiki(&mut self, out: &mut String, target: Range<usize>, alias: Option<Range<usize>>) {
        let visible = alias.unwrap_or_else(|| target.clone());
        let href = self
            .ctx
            .tree
            .and_then(|t| crate::core::links::resolve(t, &self.text[target.clone()]))
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
    pub(super) fn link(&mut self, out: &mut String, range: Range<usize>) {
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
    pub(super) fn styled_runs(&mut self, out: &mut String, range: Range<usize>) {
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
            styled(out, &self.text[i..j], sf, self.a.tint_at(i));
            i = j;
        }
    }

    /// The destination and display range of the link ending at `range`:
    /// inline `[t](u)`, reference `[t][id]` / `[t][]`, shortcut `[t]` or
    /// `<autolink>` (whose `<>` never show in the text).
    pub(super) fn dest(&mut self, range: &Range<usize>) -> Option<(String, Range<usize>)> {
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

    pub(super) fn def_lookup(&mut self, label: &str) -> Option<String> {
        let l = label.trim().to_lowercase();
        self.link_defs()
            .iter()
            .find(|(k, _)| *k == l)
            .map(|(_, d)| d.clone())
    }

    /// `[label]: dest` definitions anywhere in the note.
    pub(super) fn link_defs(&mut self) -> &Vec<(String, String)> {
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
