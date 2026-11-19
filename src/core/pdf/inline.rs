use super::*;
impl Emitter<'_> {
    pub(super) fn inline(&mut self, flow: &mut Flow, range: Range<usize>) {
        let mut i = range.start;
        while i < range.end {
            if let Some(end) = self.special(flow, i) {
                i = end;
                continue;
            }
            if let Some(end) = self.hidden_end(i) {
                i = end;
                continue;
            }
            let f = self.a.flags.get(i).copied().unwrap_or(0);
            if f & md::MARK != 0 {
                i += 1;
                continue;
            }
            if f & md::LINK != 0 {
                let mut j = i + 1;
                while j < range.end
                    && self.flags_at(j) & md::LINK != 0
                    && self.flags_at(j) & md::MARK == 0
                    && self.hidden_end(j).is_none()
                    && !self.special_start(j)
                {
                    j += 1;
                }
                self.link(flow, i..j);
                i = j;
                continue;
            }
            let sf = f & STYLE;
            let mut j = i + 1;
            while j < range.end
                && self.flags_at(j) & STYLE == sf
                && self.flags_at(j) & (md::LINK | md::MARK) == 0
                && self.hidden_end(j).is_none()
                && !self.special_start(j)
            {
                j += 1;
            }
            flow.run(self.styled(
                self.text.get(i..j).unwrap_or_default(),
                sf,
                self.a.tint_at(i),
            ));
            i = j;
        }
    }

    pub(super) fn flags_at(&self, i: usize) -> u16 {
        self.a.flags.get(i).copied().unwrap_or(0)
    }

    /// The `i`-positioned image / wiki-link / footnote constructs.
    pub(super) fn special(&mut self, flow: &mut Flow, i: usize) -> Option<usize> {
        if let Some(im) = at(&self.a.images, i, |x| &x.range) {
            let (range, src, embed) = (im.range.clone(), im.src.clone(), im.embed);
            let end = range.end;
            self.image(flow, range, src, embed);
            return Some(end);
        }
        if let Some(w) = at(&self.a.wiki_links, i, |x| &x.range) {
            let (range, target, alias) = (w.range.clone(), w.target.clone(), w.alias.clone());
            self.wiki(flow, target, alias);
            return Some(range.end);
        }
        if let Some(f) = at(&self.a.footnotes, i, |x| &x.range) {
            let (range, label, def) = (f.range.clone(), f.label.clone(), f.def);
            if !def {
                let l = self.text.get(label).unwrap_or_default();
                flow.run(StyledString::new(
                    format!("[{l}]"),
                    Style::new()
                        .with_color(FAINT)
                        .with_font_size(FOOTNOTE_REF_PT),
                ));
            }
            return Some(range.end);
        }
        if let Some(m) = at(&self.a.maths, i, |x| &x.range) {
            let (range, content, block) = (m.range.clone(), m.content.clone(), m.block);
            self.math(flow, content, block);
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

    pub(super) fn ink_style(&self) -> Style {
        Style::new().with_color(self.ink)
    }

    pub(super) fn mono_style(&self) -> Style {
        let mut s = Style::new().with_color(CODE_INK);
        s.set_font_family(self.mono);
        s.set_font_size(CODE_PT);
        s
    }

    /// Italic DejaVu — math set apart from the sans prose.
    pub(super) fn math_style(&self) -> Style {
        let mut s = Style::new().with_color(self.ink).italic();
        s.set_font_family(self.math);
        s
    }

    /// `$…$`/`$$…$$`: the editor's Unicode approximation in the math face.
    /// A construct `math.rs` can't read — or that converts to a glyph the
    /// face lacks — keeps its LaTeX source, printed in mono so it reads as
    /// source instead of a broken glyph. `$$` blocks center on their own
    /// line when the flow can hold a block element.
    pub(super) fn math(&mut self, flow: &mut Flow, content: Range<usize>, block: bool) {
        let Some(src) = self.text.get(clip(content, &self.sel)) else {
            return;
        };
        let mut runs: Vec<StyledString> = Vec::new();
        let mut at = 0;
        for p in crate::core::math::pieces(src) {
            if p.range.start > at {
                runs.push(StyledString::new(
                    src[at..p.range.start].to_string(),
                    self.math_style(),
                ));
            }
            let show_source = match &p.text {
                Some(t) => t.chars().any(|c| MATH_TOFU.contains(c)),
                None => true,
            };
            runs.push(if show_source {
                StyledString::new(src[p.range.clone()].to_string(), self.mono_style())
            } else {
                StyledString::new(p.text.unwrap_or_default(), self.math_style())
            });
            at = p.range.end.max(at);
        }
        if at < src.len() {
            runs.push(StyledString::new(src[at..].to_string(), self.math_style()));
        }
        if block && flow.layout.is_some() {
            flow.element(Paragraph::from(runs).aligned(Alignment::Center));
        } else {
            for r in runs {
                flow.run(r);
            }
        }
    }

    /// A run of text under `sf` — underline/strike degrade to plain text,
    /// highlight degrades to a tinted color, links to blue.
    pub(super) fn styled(&self, s: &str, f: u16, tint: Option<md::Tint>) -> StyledString {
        let mut style = self.ink_style();
        if f & md::BOLD != 0 {
            style.set_bold();
        }
        if f & md::ITALIC != 0 {
            style.set_italic();
        }
        if f & md::CODE != 0 {
            style.set_font_family(self.mono);
            style.set_font_size(CODE_PT);
            style.set_color(CODE_INK);
        }
        if f & md::HIGHLIGHT != 0 {
            style.set_color(tint_color(tint));
        }
        if self.link_depth > 0 {
            style.set_color(LINK);
        }
        StyledString::new(s, style)
    }

    /// `![alt](src)` / `![[src]]` — scaled to the text column, or `[alt]`
    /// when the file can't be loaded (or the context can't hold an image).
    pub(super) fn image(&mut self, flow: &mut Flow, range: Range<usize>, src: Range<usize>, embed: bool) {
        let raw = self.text.get(src.clone()).unwrap_or_default();
        let bare = raw
            .strip_prefix('<')
            .and_then(|r| r.strip_suffix('>'))
            .unwrap_or(raw);
        let alt = if embed {
            bare
        } else {
            // The description sits between `![` and `](`.
            self.text
                .get(range.start + 2..src.start.saturating_sub(2))
                .unwrap_or_default()
        };
        if flow.layout.is_some()
            && let Some(img) = self.image_el(bare, embed)
        {
            flow.element(img);
            return;
        }
        flow.run(StyledString::new(
            if alt.is_empty() {
                "[image]".to_string()
            } else {
                format!("[{alt}]")
            },
            Style::new().italic().with_color(FAINT),
        ));
    }

    /// The `![alt](path)`/`![[path]]` image scaled to the text column.
    pub(super) fn image_el(&self, bare: &str, embed: bool) -> Option<Image> {
        let p = self.image_path(bare, embed)?;
        let mut img = Image::from_path(&p).ok()?;
        let (w, h) = image::image_dimensions(&p).ok()?;
        // genpdf draws at 300 dpi by default — px→mm is px * 25.4/300.
        let to_mm = |px: u32| px as f64 * 25.4 / 300.0;
        let s = (TEXT_W / to_mm(w)).min(MAX_IMG_H / to_mm(h)).min(1.0);
        if s < 1.0 {
            img.set_scale(Scale::new(s, s));
        }
        img.set_alignment(Alignment::Center);
        Some(img)
    }

    /// Resolves an image source to an existing local file — absolute paths,
    /// `attachments/` embeds searched like the in-app resolver, plain
    /// relative paths under the note's folder.
    pub(super) fn image_path(&self, bare: &str, embed: bool) -> Option<PathBuf> {
        if bare.contains("://") {
            return None;
        }
        let dec = attach::decode(bare);
        if Path::new(&dec).is_absolute() {
            return Path::new(&dec).exists().then(|| PathBuf::from(&dec));
        }
        if embed && let (Some(dir), Some(root)) = (self.ctx.note_dir, self.ctx.root) {
            for cand in attach::candidates(bare, true, dir, root) {
                if cand.exists() {
                    return Some(cand);
                }
            }
            return None;
        }
        let dir = self.ctx.note_dir?;
        let p = dir.join(&dec);
        p.exists().then_some(p)
    }

    /// `[[target]]` / `[[target|alias]]` — colored when it resolves.
    pub(super) fn wiki(&mut self, flow: &mut Flow, target: Range<usize>, alias: Option<Range<usize>>) {
        let visible = alias.unwrap_or_else(|| target.clone());
        let resolved = self
            .ctx
            .tree
            .and_then(|t| {
                crate::core::links::resolve(t, self.text.get(target.clone()).unwrap_or_default())
            })
            .is_some();
        let mut style = self.ink_style();
        if resolved {
            style.set_color(LINK);
        }
        flow.run(StyledString::new(
            self.text.get(visible).unwrap_or_default(),
            style,
        ));
    }

    /// A LINK-flagged run: colored text (PDF links aren't clickable).
    pub(super) fn link(&mut self, flow: &mut Flow, range: Range<usize>) {
        let Some((_d, inner)) = self.dest(&range) else {
            self.styled_runs(flow, range);
            return;
        };
        self.link_depth += 1;
        self.styled_runs(flow, inner);
        self.link_depth -= 1;
    }

    /// Text inside a link run: the same flag segmentation minus the LINK bit.
    pub(super) fn styled_runs(&mut self, flow: &mut Flow, range: Range<usize>) {
        let mut i = range.start;
        while i < range.end {
            if let Some(end) = self.hidden_end(i) {
                i = end;
                continue;
            }
            let f = self.flags_at(i);
            if f & md::MARK != 0 {
                i += 1;
                continue;
            }
            let sf = f & STYLE;
            let mut j = i + 1;
            while j < range.end
                && self.flags_at(j) & STYLE == sf
                && self.flags_at(j) & md::MARK == 0
                && self.hidden_end(j).is_none()
            {
                j += 1;
            }
            flow.run(self.styled(
                self.text.get(i..j).unwrap_or_default(),
                sf,
                self.a.tint_at(i),
            ));
            i = j;
        }
    }

    /// The destination and display range of the link ending at `range`:
    /// inline `[t](u)`, reference `[t][id]` / `[t][]`, shortcut `[t]` or
    /// `<autolink>` (whose `<>` never show in the text).
    pub(super) fn dest(&self, range: &Range<usize>) -> Option<(String, Range<usize>)> {
        let rest = self
            .text
            .get(range.end.min(self.text.len())..)
            .unwrap_or_default();
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
                self.text.get(range.clone()).unwrap_or_default().to_string()
            } else {
                r[..n].to_string()
            };
            return self.def_lookup(&label).map(|d| (d, range.clone()));
        }
        self.def_lookup(self.text.get(range.clone()).unwrap_or_default())
            .map(|d| (d, range.clone()))
            .or_else(|| {
                let inner = self.text.get(range.clone()).unwrap_or_default();
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

    pub(super) fn def_lookup(&self, label: &str) -> Option<String> {
        let l = label.trim().to_lowercase();
        self.link_defs
            .iter()
            .find(|(k, _)| *k == l)
            .map(|(_, d)| d.clone())
    }
}
