use super::*;
impl<'a> Emitter<'a> {
    pub(super) fn new(
        text: &'a str,
        a: &'a Analysis,
        sel: Range<usize>,
        ctx: &'a Context<'a>,
        mono: Family,
        math: Family,
    ) -> Self {
        let mut hidden: Vec<Range<usize>> = a.conceals.iter().map(|c| c.hidden.clone()).collect();
        hidden.extend(comments(text, a));
        hidden.sort_by_key(|r| r.start);
        Self {
            text,
            a,
            ctx,
            sel,
            hidden,
            defs: Vec::new(),
            link_defs: link_defs(text),
            mono,
            math,
            ink: INK,
            link_depth: 0,
        }
    }

    pub(super) fn render(&mut self, out: &mut LinearLayout) {
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

    /// Block children with a `Break` between them (not after the last one).
    pub(super) fn children(&mut self, out: &mut LinearLayout, node: Node) {
        let mut first = true;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let len = self.text.len();
            if clip(
                child.start_byte().min(len)..child.end_byte().min(len),
                &self.sel,
            )
            .is_empty()
            {
                continue;
            }
            if first {
                first = false;
            } else {
                out.push(Break::new(BLOCK_GAP));
            }
            self.block(out, child);
        }
    }

    pub(super) fn block(&mut self, out: &mut LinearLayout, node: Node) {
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
                self.heading(out, node, heading_level(node));
            }
            "paragraph" => {
                if self.def_bodies(node, &nb) {
                    return;
                }
                self.paragraph(out, node);
            }
            "block_quote" => self.quote(out, node),
            "list" => self.list(out, node),
            "list_item" => self.list_item(out, node, None),
            "thematic_break" => out.push(Rule.styled(line_style())),
            "fenced_code_block" => self.code(out, node, true),
            "indented_code_block" => self.code(out, node, false),
            "pipe_table" => self.table(out, node),
            "link_reference_definition" => {
                self.def_bodies(node, &nb);
            }
            "minus_metadata" | "plus_metadata" => {}
            "html_block" => out.push(Paragraph::new(StyledString::new(
                self.text.get(nb).unwrap_or_default(),
                Style::new().with_color(self.ink),
            ))),
            _ => self.children(out, node),
        }
    }

    /// The `inline` children of a node (paragraph, heading).
    pub(super) fn inline_nodes(&mut self, flow: &mut Flow, node: Node) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "inline" {
                let len = self.text.len();
                self.inline(
                    flow,
                    clip(
                        child.start_byte().min(len)..child.end_byte().min(len),
                        &self.sel,
                    ),
                );
            }
        }
    }

    pub(super) fn heading(&mut self, out: &mut LinearLayout, node: Node, level: u8) {
        let mut flow = Flow::runs();
        self.inline_nodes(&mut flow, node);
        if flow.runs.is_empty() {
            return;
        }
        let size = HEADING_PT[level.saturating_sub(1).min(5) as usize];
        out.push(Break::new(HEAD_GAP));
        out.push(Paragraph::from(
            flow.runs
                .into_iter()
                .map(|s| {
                    StyledString::new(s.s, s.style.and(Style::new().with_font_size(size).bold()))
                })
                .collect::<Vec<_>>(),
        ));
    }

    pub(super) fn paragraph(&mut self, out: &mut LinearLayout, node: Node) {
        let mut flow = Flow {
            runs: Vec::new(),
            layout: Some(out),
        };
        self.inline_nodes(&mut flow, node);
        flow.flush();
    }

    pub(super) fn quote(&mut self, out: &mut LinearLayout, node: Node) {
        let len = self.text.len();
        let first = self.a.line_of(node.start_byte().min(len));
        let Some(callout) = self.a.callouts.iter().find(|c| c.lines.start == first) else {
            let mut inner = LinearLayout::vertical();
            let ink = std::mem::replace(&mut self.ink, MUTED);
            self.children(&mut inner, node);
            self.ink = ink;
            out.push(Block::bar(inner).styled(line_style()));
            return;
        };
        let tone = tone_color(callout.tone);
        // The title is the first line of the first paragraph; the same
        // paragraph node continues into the callout body.
        let title_end = self
            .a
            .lines
            .get(callout.lines.start)
            .map(|l| l.0.end)
            .unwrap_or(len);
        let mut inner = LinearLayout::vertical();
        let mut titled = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !titled && child.kind() == "paragraph" {
                titled = true;
                let mut c = child.walk();
                for inner_node in child.children(&mut c) {
                    if inner_node.kind() != "inline" {
                        continue;
                    }
                    let r = inner_node.start_byte().min(len)..inner_node.end_byte().min(len);
                    let mut tf = Flow::runs();
                    self.inline(&mut tf, clip(r.start..r.end.min(title_end), &self.sel));
                    let title: Vec<StyledString> = tf
                        .runs
                        .into_iter()
                        .map(|s| {
                            StyledString::new(
                                s.s,
                                s.style.and(Style::new().with_color(tone).bold()),
                            )
                        })
                        .collect();
                    if !title.is_empty() {
                        inner.push(Paragraph::from(title));
                        inner.push(Break::new(0.3));
                    }
                    let mut bf = Flow {
                        runs: Vec::new(),
                        layout: Some(&mut inner),
                    };
                    self.inline(&mut bf, trim(self.text, clip(title_end..r.end, &self.sel)));
                    bf.flush();
                }
                continue;
            }
            self.block(&mut inner, child);
        }
        out.push(Block::bar(inner).styled(Style::new().with_color(tone)));
    }

    pub(super) fn list(&mut self, out: &mut LinearLayout, node: Node) {
        let mut cursor = node.walk();
        let ordered = node.children(&mut cursor).any(|item| {
            let mut c = item.walk();
            item.children(&mut c)
                .any(|k| matches!(k.kind(), "list_marker_dot" | "list_marker_parenthesis"))
        });
        let mut n = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "list_item" {
                n += 1;
                self.list_item(out, child, ordered.then_some(n));
            } else {
                self.block(out, child);
            }
        }
    }

    pub(super) fn list_item(&mut self, out: &mut LinearLayout, node: Node, num: Option<usize>) {
        let mut cursor = node.walk();
        let kids: Vec<Node> = node.children(&mut cursor).collect();
        let checked = kids.iter().find_map(|k| match k.kind() {
            "task_list_marker_checked" => Some(true),
            "task_list_marker_unchecked" => Some(false),
            _ => None,
        });
        let mut inner = LinearLayout::vertical();
        let mut first_para = true;
        for child in kids {
            match child.kind() {
                "paragraph" => {
                    if first_para {
                        first_para = false;
                    } else {
                        inner.push(Break::new(0.3));
                    }
                    let mut flow = Flow {
                        runs: Vec::new(),
                        layout: Some(&mut inner),
                    };
                    self.inline_nodes(&mut flow, child);
                    flow.flush();
                }
                "task_list_marker_checked" | "task_list_marker_unchecked" => {}
                _ => self.block(&mut inner, child),
            }
        }
        // ☐/■ live only in Noto Sans Mono — the bullet takes its own style.
        let (bullet, style) = match (checked, num) {
            (Some(true), _) => ("\u{25A0}".to_string(), self.mono_style()),
            (Some(false), _) => ("\u{25A1}".to_string(), self.mono_style()),
            (None, Some(n)) => (format!("{n}."), Style::new().with_color(self.ink)),
            (None, None) => ("\u{2022}".to_string(), Style::new().with_color(self.ink)),
        };
        out.push(Block::bullet(inner, bullet, style));
    }

    pub(super) fn code(&mut self, out: &mut LinearLayout, node: Node, fenced: bool) {
        let len = self.text.len();
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
            if child.kind() == "code_fence_content" {
                let r = child.start_byte().min(len)..child.end_byte().min(len);
                content = Some(clip(r, &self.sel));
            }
        }
        let mut rows: Vec<String> = Vec::new();
        if let Some(r) = content
            && let Some(s) = self.text.get(r)
        {
            for line in s.lines() {
                let line = if fenced {
                    line
                } else {
                    let indent = line.len() - line.trim_start_matches(' ').len();
                    &line[indent.min(4)..]
                };
                wrap_code(&mut rows, &line.replace('\t', "    "));
            }
        }
        let mut lines = LinearLayout::vertical();
        for l in rows {
            lines.push(Paragraph::new(StyledString::new(l, self.mono_style())).preformatted());
        }
        out.push(
            Block::padded(lines, Margins::trbl(1.6, 2.2, 1.6, 2.2))
                .framed()
                .styled(line_style()),
        );
    }

    pub(super) fn table(&mut self, out: &mut LinearLayout, node: Node) {
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
                    if let Some(s) = self.text.get(r) {
                        aligns = crate::table::aligns(s);
                    }
                }
                _ => {}
            }
        }
        let cells_of = |this: &mut Self, row: Node| -> Vec<Vec<StyledString>> {
            let mut cells = Vec::new();
            let mut c = row.walk();
            for cell in row.children(&mut c) {
                if cell.kind() != "pipe_table_cell" {
                    continue;
                }
                let r = clip(
                    cell.start_byte().min(len)..cell.end_byte().min(len),
                    &this.sel,
                );
                let mut flow = Flow::runs();
                this.inline(&mut flow, trim(this.text, r));
                cells.push(flow.runs);
            }
            cells
        };
        let mut grid: Vec<(bool, Vec<Vec<StyledString>>)> = Vec::new();
        if let Some(h) = header {
            grid.push((true, cells_of(self, h)));
        }
        for row in rows {
            grid.push((false, cells_of(self, row)));
        }
        let ncols = grid.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
        if ncols == 0 {
            return;
        }
        // Column weights follow the widest cell so prose gets more room.
        let mut weights = vec![1usize; ncols];
        for (_, cells) in &grid {
            for (i, cell) in cells.iter().enumerate() {
                let n: usize = cell.iter().map(|s| s.s.chars().count()).sum();
                weights[i] = weights[i].max((n / 8 + 1).min(12));
            }
        }
        let mut table = TableLayout::new(weights);
        for (is_header, cells) in grid {
            let mut row: Vec<Block> = Vec::with_capacity(ncols);
            for (i, cell) in cells.into_iter().enumerate() {
                let extra = if is_header {
                    Style::new().bold().with_font_size(CELL_PT)
                } else {
                    Style::new().with_font_size(CELL_PT)
                };
                let runs: Vec<StyledString> = cell
                    .into_iter()
                    .map(|s| StyledString::new(s.s, s.style.and(extra)))
                    .collect();
                let alignment = match aligns.get(i).copied().unwrap_or(crate::table::Align::None) {
                    crate::table::Align::None | crate::table::Align::Left => Alignment::Left,
                    crate::table::Align::Center => Alignment::Center,
                    crate::table::Align::Right => Alignment::Right,
                };
                row.push(Block::padded(
                    Paragraph::from(runs).aligned(alignment),
                    Margins::all(1.2f32),
                ));
            }
            while row.len() < ncols {
                row.push(Paragraph::new("").into());
            }
            row.truncate(ncols);
            table.push_row(row);
        }
        out.push(table.styled(line_style()));
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

    pub(super) fn footnote_section(&mut self, out: &mut LinearLayout) {
        let defs = std::mem::take(&mut self.defs);
        if defs.is_empty() {
            return;
        }
        out.push(Break::new(0.4));
        out.push(Rule.styled(line_style()));
        out.push(Break::new(0.4));
        for (label, body) in defs {
            let l = self.text.get(label).unwrap_or_default();
            let mut runs = vec![StyledString::new(
                format!("[{l}]"),
                Style::new()
                    .bold()
                    .with_color(FAINT)
                    .with_font_size(FOOTNOTE_PT),
            )];
            let mut flow = Flow::runs();
            self.inline(&mut flow, body);
            if !flow.runs.is_empty() {
                runs.push(StyledString::new(
                    " ",
                    Style::new().with_font_size(FOOTNOTE_PT),
                ));
                for s in flow.runs {
                    runs.push(StyledString::new(
                        s.s,
                        s.style
                            .and(Style::new().with_color(MUTED).with_font_size(FOOTNOTE_PT)),
                    ));
                }
            }
            out.push(Paragraph::from(runs));
            out.push(Break::new(0.2));
        }
    }

    // ── Inline emission (flag-driven) ────────────────────────────────────
}
