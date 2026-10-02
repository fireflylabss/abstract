use super::*;

impl AbstractApp {
    /// Flattened visible rows plus the synthetic pending-note/new-folder
    /// rows, the PINNED group on top, and the active tag filter.
    pub(crate) fn flat_rows(&self) -> Vec<vault::Row> {
        let mut rows = vault::flatten(&self.tree, &self.expanded);
        // Tag filter: keep tagged notes plus the folders containing them.
        if let Some(set) = self.tag_filter.as_ref().and_then(|t| self.tag_paths.get(t)) {
            let mut keep: Vec<bool> = rows
                .iter()
                .map(|r| r.kind == NodeKind::Note && set.contains(&r.path))
                .collect();
            for i in (0..rows.len()).rev() {
                if rows[i].kind != NodeKind::Folder || keep[i] {
                    continue;
                }
                let d = rows[i].depth;
                let mut j = i + 1;
                while j < rows.len() && rows[j].depth > d {
                    if keep[j] {
                        keep[i] = true;
                        break;
                    }
                    j += 1;
                }
            }
            let mut it = keep.into_iter();
            rows.retain(|_| it.next().unwrap_or(false));
        }
        let insert = |rows: &mut Vec<vault::Row>, parent: &Path| -> usize {
            let (ix, depth) = match rows.iter().position(|r| r.path == parent) {
                Some(i) => (i + 1, rows[i].depth + 1),
                None => (0, 0),
            };
            rows.insert(
                ix,
                vault::Row {
                    path: PathBuf::new(),
                    name: String::new(),
                    kind: NodeKind::Note,
                    depth,
                    expanded: false,
                },
            );
            ix
        };
        if let Some(ed) = &self.editing
            && ed.create
        {
            let ix = insert(&mut rows, &ed.target);
            rows[ix].path = ed.target.join(NEW_FOLDER_ROW);
            rows[ix].kind = NodeKind::Folder;
        }
        for p in self.tabs.iter().filter(|t| t.pending).map(NoteTab::path) {
            if rows.iter().any(|r| r.path == p) {
                continue;
            }
            if let Some(parent) = p.parent() {
                let ix = insert(&mut rows, parent);
                rows[ix].path = p.clone();
                rows[ix].name = t(Key::Untitled).into();
            }
        }
        // Pinned aliases on top (skipped while a tag filter is active): the
        // filtered view is already narrowed to what the user asked for.
        if self.tag_filter.is_none() && !self.pins.is_empty() {
            let mut top = vec![vault::Row {
                path: PathBuf::from(PINNED_ROW),
                name: t(Key::Pinned).into(),
                kind: NodeKind::Note,
                depth: 0,
                expanded: false,
            }];
            top.extend(
                self.pins
                    .iter()
                    .filter(|p| rows.iter().any(|r| r.path == **p))
                    .map(|p| vault::Row {
                        path: p.clone(),
                        name: stem_of(p),
                        kind: NodeKind::Note,
                        depth: 0,
                        expanded: false,
                    }),
            );
            if top.len() > 1 {
                rows.splice(0..0, top);
            }
        }
        rows
    }

    pub(crate) fn note_count(nodes: &[vault::Node]) -> usize {
        nodes
            .iter()
            .map(|n| {
                if n.is_folder() {
                    Self::note_count(&n.children)
                } else {
                    1
                }
            })
            .sum()
    }

    pub(crate) fn render_row(
        &self,
        ix: usize,
        row: &vault::Row,
        current: Option<&Path>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let pal = cx.palette();
        // The PINNED group header: a quiet label, not a note row.
        if row.path.as_path() == Path::new(PINNED_ROW) {
            return div()
                .id(("row", ix))
                .h(px(30.))
                .pl(px(10.))
                .flex()
                .items_center()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(pal.faint))
                .child(icon("icons/pin.svg", pal.faint).size(px(12.)).mr(px(6.)))
                .child(row.name.clone());
        }
        let active = current == Some(row.path.as_path());
        let folder = row.kind == NodeKind::Folder;
        let pinned = !folder && self.pins.contains(&row.path);
        let editing = self.editing.as_ref().is_some_and(|e| {
            (!e.create && e.target == row.path) || (e.create && row.path.ends_with(NEW_FOLDER_ROW))
        });
        let path = row.path.clone();
        let kind = row.kind;

        let mut pill = div()
            .id(("row", ix))
            .group("note-row")
            .role(Role::Button)
            .aria_label(row.name.clone())
            .h(px(30.))
            .pl(px(10. + row.depth as f32 * 14.))
            .pr(px(4.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(13.))
            .line_height(px(18.))
            .text_color(rgb(if active { pal.fg } else { pal.dim }))
            .when(active, |s| s.bg(rgb(pal.active)))
            .when(!active, |s| {
                s.hover(|s| s.bg(rgb(pal.hover)).text_color(rgb(pal.body)))
            })
            .active(|s| s.bg(rgb(pal.active)));
        if !editing {
            let dragged = files_menu::DraggedRow {
                path: path.clone(),
                kind,
                name: row.name.clone().into(),
            };
            pill = pill.on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()));
            if folder {
                pill = pill
                    .drag_over::<files_menu::DraggedRow>(move |s, _, _, _| {
                        s.bg(rgb(pal.active)).text_color(rgb(pal.fg))
                    })
                    .on_drop(cx.listener({
                        let path = path.clone();
                        move |this, d: &files_menu::DraggedRow, _, cx| {
                            this.move_into(d, path.clone(), cx)
                        }
                    }));
            }
            pill = pill.on_click(cx.listener({
                let path = path.clone();
                move |this, _, window, cx| match kind {
                    NodeKind::Folder => this.toggle_folder(path.clone(), cx),
                    NodeKind::Note => this.open_path(path.clone(), None, window, cx),
                }
            }));
        }
        if folder {
            pill = pill
                .child(
                    icon(
                        if row.expanded {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        },
                        pal.faint,
                    )
                    .size(px(12.)),
                )
                .child(icon("icons/folder.svg", pal.faint).size(px(15.)));
        } else {
            pill = pill.child(
                icon(
                    if pinned {
                        "icons/pin.svg"
                    } else {
                        "icons/note.svg"
                    },
                    if active { pal.fg } else { pal.faint },
                )
                .size(px(15.))
                .ml(px(18.)),
            );
        }
        if editing && let Some(ed) = &self.editing {
            let state = ed.state.clone();
            pill = pill.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .on_action(cx.listener(|this, _: &input::Escape, _, cx| this.cancel_edit(cx)))
                    .child(
                        Input::new(&state)
                            .appearance(false)
                            .bordered(false)
                            .h(px(24.))
                            .w_full()
                            .text_size(px(13.))
                            .text_color(rgb(pal.fg)),
                    ),
            );
        } else {
            pill = pill.child(div().flex_1().min_w_0().truncate().child(row.name.clone()));
            // Hover actions: rename, delete.
            pill = pill
                .child(
                    div()
                        .id(("row-rename", ix))
                        .role(Role::Button)
                        .aria_label(t(Key::Rename))
                        .size(px(20.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .invisible()
                        .group_hover("note-row", |s| s.visible())
                        .hover(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener({
                            let path = path.clone();
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.start_rename(path.clone(), kind, window, cx);
                            }
                        }))
                        .child(icon("icons/pencil.svg", pal.dim).size(px(12.))),
                )
                .child(
                    div()
                        .id(("row-delete", ix))
                        .role(Role::Button)
                        .aria_label(t(Key::MoveToTrash))
                        .size(px(20.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .invisible()
                        .group_hover("note-row", |s| s.visible())
                        .hover(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.delete_row(path.clone(), kind, window, cx);
                        }))
                        .child(icon("icons/delete.svg", pal.dim).size(px(12.))),
                );
        }
        pill
    }

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.flat_rows();
        let count = rows.len();
        let list = uniform_list(
            "notes",
            count,
            cx.processor(move |this, range: Range<usize>, _window, cx| {
                let rows = this.flat_rows();
                let current = this.current.as_ref().map(|c| c.path());
                range
                    .map(|ix| {
                        let row = &rows[ix.min(rows.len().saturating_sub(1))];
                        let pill = this.render_row(ix, row, current.as_deref(), cx);
                        let (path, kind) = (row.path.clone(), row.kind);
                        let pinned = kind == NodeKind::Note && this.pins.contains(&path);
                        let weak = cx.entity().downgrade();
                        let pill = if path.as_path() == Path::new(PINNED_ROW) {
                            div().size_full().child(pill)
                        } else {
                            div().size_full().child(pill.context_menu(move |m, _, _| {
                                Self::row_menu(&weak, &path, kind, pinned, m)
                            }))
                        };
                        div().h(px(32.)).px(px(8.)).pb(px(2.)).child(rise(
                            pill,
                            ("note-in", ix),
                            360,
                            (ix.min(12) as f32) * 0.05,
                            4.,
                        ))
                    })
                    .collect()
            }),
        )
        .flex_1()
        .min_h_0();

        let (from, to) = if self.sidebar_open {
            (0., SIDEBAR_W)
        } else {
            (SIDEBAR_W, 0.)
        };
        let pal = cx.palette();
        let space_name = SharedString::from(spaces::name_of(&self.dir));
        let notes_n = Self::note_count(&self.tree);
        let panel = div()
            .id("sidebar")
            .flex_none()
            .h_full()
            .overflow_hidden()
            .bg(rgb(pal.panel))
            .border_r_1()
            .border_color(rgb(pal.line))
            .child(
                div()
                    .relative()
                    .w(px(SIDEBAR_W))
                    .h_full()
                    .flex()
                    .flex_col()
                    // 48px header = toolbar height; drags the window.
                    .child(
                        drag_fallback(div().id("sidebar-head"))
                            .h(px(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .pl(px(chrome_left_pad(true)))
                            .pr(px(9.))
                            .child(
                                self.ring(
                                    0,
                                    div()
                                        .id("space-switcher")
                                        .role(Role::Button)
                                        .aria_label(tf(Key::SwitchSpace, &[]).as_str())
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(30.))
                                        .px(px(8.))
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .occlude()
                                        .when(self.spaces_open, |s| s.bg(rgb(pal.active)))
                                        .hover(|s| s.bg(rgb(pal.hover)))
                                        .active(|s| s.bg(rgb(pal.active)))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.toggle_spaces(cx)),
                                        )
                                        .child(icon("icons/folder.svg", pal.fg).size(px(15.)))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(px(13.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(pal.fg))
                                                .child(space_name),
                                        )
                                        .child(icon("icons/chevrons.svg", pal.faint).size(px(14.)))
                                        .when_some(
                                            self.mark(
                                                0,
                                                Anchor::TopLeft,
                                                point(px(0.), px(38.)),
                                                cx,
                                            ),
                                            |s, m| s.child(m),
                                        ),
                                    &pal,
                                ),
                            )
                            .child(
                                self.ring(
                                    1,
                                    icon_btn(
                                        "new-folder",
                                        "icons/folder-add.svg",
                                        t(Key::NewFolder).into(),
                                        false,
                                        &pal,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.new_folder(window, cx),
                                    )),
                                    &pal,
                                )
                                .when_some(
                                    self.mark(1, Anchor::TopRight, point(px(30.), px(38.)), cx),
                                    |s, m| s.child(m),
                                ),
                            )
                            .child(
                                icon_btn(
                                    "new",
                                    "icons/add.svg",
                                    tf(Key::NewNote, &[]).into(),
                                    false,
                                    &pal,
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.new_note(window, cx)),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .id("notes-head")
                            .h(px(28.))
                            .flex_none()
                            .drag_over::<files_menu::DraggedRow>(move |s, _, _, _| {
                                s.bg(rgb(pal.active)).text_color(rgb(pal.fg))
                            })
                            .on_drop(cx.listener(|this, d: &files_menu::DraggedRow, _, cx| {
                                this.move_into(d, this.dir.clone(), cx)
                            }))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(18.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(pal.faint))
                            .child(div().min_w_0().truncate().child(match &self.tag_filter {
                                Some(k) => SharedString::from(format!("#{}", self.tag_name(k))),
                                None => t(Key::Notes).into(),
                            }))
                            .child(match &self.tag_filter {
                                Some(k) => div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(self.tag_paths.get(k).map_or(0, |s| s.len()).to_string())
                                    .child(
                                        div()
                                            .id("clear-tag")
                                            .role(Role::Button)
                                            .aria_label(t(Key::ClearFilter))
                                            .size(px(16.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(pal.active)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.tag_filter = None;
                                                cx.notify();
                                            }))
                                            .child(icon("icons/close.svg", pal.dim).size(px(10.))),
                                    )
                                    .into_any_element(),
                                None => div().child(notes_n.to_string()).into_any_element(),
                            }),
                    )
                    .child(list)
                    .when_some(self.render_tags(cx), |s, el| s.child(el))
                    .child(self.render_settings_button(cx))
                    .when(self.spaces_open, |col| {
                        col.child(self.render_spaces_menu(cx))
                    }),
            );
        if self.sidebar_gen == 0 {
            return panel.w(px(to)).into_any_element();
        }
        panel
            .with_animation(
                ("sidebar-slide", self.sidebar_gen),
                Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint),
                move |el, d| el.w(px(from + (to - from) * d)),
            )
            .into_any_element()
    }

    /// Display casing for a lowercased tag key (falls back to the key).
    fn tag_name<'a>(&'a self, key: &'a str) -> &'a str {
        self.tags
            .iter()
            .find(|(d, _)| d.to_lowercase() == key)
            .map(|(d, _)| d.as_str())
            .unwrap_or(key)
    }

    /// Click a tag row: filter the note list to it (again to clear).
    pub(crate) fn filter_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        self.tag_filter = if self.tag_filter.as_deref() == Some(tag.as_str()) {
            None
        } else {
            Some(tag)
        };
        cx.notify();
    }

    pub(crate) fn toggle_tags(&mut self, cx: &mut Context<Self>) {
        self.tags_open = !self.tags_open;
        self.save_session(cx);
        cx.notify();
    }

    /// Recompute the tag index off-thread; applied only while the same space
    /// is still open. Files are re-read only when their mtime moved.
    pub(crate) fn refresh_tags(&mut self, cx: &mut Context<Self>) {
        let dir = self.dir.clone();
        if dir.as_os_str().is_empty() {
            return;
        }
        let index = self.tag_index.clone();
        let scanned = dir.clone();
        self._tags_task = Some(cx.spawn(async move |this, cx| {
            let (tags, paths) = cx
                .background_executor()
                .spawn(async move { guard(&index).refresh(&scanned) })
                .await;
            this.update(cx, |this, cx| {
                if this.dir != dir {
                    return;
                }
                this.tags = tags;
                this.tag_paths = paths;
                if let Some(f) = &this.tag_filter
                    && !this.tag_paths.contains_key(f)
                {
                    this.tag_filter = None;
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// Collapsible TAGS strip under the note list; hidden while a space has
    /// no tags at all.
    pub(crate) fn render_tags(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.tags.is_empty() {
            return None;
        }
        let pal = cx.palette();
        let mut section = div()
            .flex_none()
            .border_t_1()
            .border_color(rgb(pal.line))
            .child(
                div()
                    .id("tags-head")
                    .role(Role::Button)
                    .aria_expanded(self.tags_open)
                    .h(px(28.))
                    .px(px(18.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .cursor_pointer()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(pal.faint))
                    .hover(|s| s.text_color(rgb(pal.dim)))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_tags(cx)))
                    .child(
                        icon(
                            if self.tags_open {
                                "icons/chevron-down.svg"
                            } else {
                                "icons/chevron-right.svg"
                            },
                            pal.faint,
                        )
                        .size(px(11.)),
                    )
                    .child(t(Key::Tags)),
            );
        if self.tags_open {
            let mut rows = div().flex().flex_col().gap(px(1.)).px(px(8.)).pb(px(6.));
            for (ix, (name, count)) in self.tags.iter().enumerate() {
                let key = name.to_lowercase();
                let on = self.tag_filter.as_deref() == Some(key.as_str());
                rows = rows.child(
                    div()
                        .id(("tag", ix))
                        .role(Role::Button)
                        .aria_label(SharedString::from(format!("#{name}")))
                        .h(px(26.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.dim }))
                        .when(on, |s| s.bg(rgb(pal.active)))
                        .when(!on, |s| {
                            s.hover(|s| s.bg(rgb(pal.hover)).text_color(rgb(pal.body)))
                        })
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.filter_tag(key.clone(), cx)),
                        )
                        .child(
                            icon("icons/hash.svg", if on { pal.fg } else { pal.faint })
                                .size(px(13.)),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(11.))
                                .text_color(rgb(pal.faint))
                                .child(count.to_string()),
                        ),
                );
            }
            section = section.child(
                div()
                    .id("tags-list")
                    .max_h(px(160.))
                    .overflow_y_scroll()
                    .child(rows),
            );
        }
        Some(section.into_any_element())
    }
}
