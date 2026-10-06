use super::*;

impl AbstractApp {
    /// Flattened visible rows plus the synthetic pending-note/new-folder rows.
    pub(crate) fn flat_rows(&self) -> Vec<vault::Row> {
        let mut rows = vault::flatten(&self.tree, &self.expanded);
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
        // Unsaved pending notes show as ghost rows where they'll land.
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let pal = cx.palette();
        let active = current == Some(row.path.as_path());
        let folder = row.kind == NodeKind::Folder;
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
            .h(z(30.))
            .pl(z(10. + row.depth as f32 * 14.))
            .pr(z(4.))
            .flex()
            .items_center()
            .gap(z(6.))
            .rounded(z(6.))
            .cursor_pointer()
            .text_size(z(13.))
            .line_height(z(18.))
            .text_color(rgb(if active { pal.fg } else { pal.dim }))
            .when(!active, |s| s.hover(|s| s.text_color(rgb(pal.body))))
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
                move |this, ev: &ClickEvent, window, cx| match kind {
                    NodeKind::Folder => this.toggle_folder(path.clone(), cx),
                    // Cmd/Ctrl+click keeps the active tab; a plain click
                    // retargets it.
                    NodeKind::Note => {
                        if ev.modifiers().secondary() {
                            this.open_path_tab(path.clone(), None, window, cx);
                        } else {
                            this.open_path(path.clone(), None, window, cx);
                        }
                    }
                }
            }));
            if kind == NodeKind::Note {
                let path = path.clone();
                pill = pill.on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.open_path_tab(path.clone(), None, window, cx);
                    }),
                );
            }
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
                    .size(z(12.)),
                )
                .child(icon("icons/folder.svg", pal.faint).size(z(15.)));
        } else {
            pill = pill.child(
                icon("icons/note.svg", if active { pal.fg } else { pal.faint })
                    .size(z(15.))
                    .ml(z(18.)),
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
                            .h(z(24.))
                            .w_full()
                            .text_size(z(13.))
                            .text_color(rgb(pal.fg)),
                    ),
            );
        } else {
            pill = pill.child(div().flex_1().min_w_0().truncate().child(row.name.clone()));
            // Hover actions: rename, delete.
            pill = pill
                .child(hover_bg(
                    div()
                        .id(("row-rename", ix))
                        .role(Role::Button)
                        .aria_label(t(Key::Rename))
                        .size(z(20.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(6.))
                        .invisible()
                        .group_hover("note-row", |s| s.visible())
                        .on_click(cx.listener({
                            let path = path.clone();
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.start_rename(path.clone(), kind, window, cx);
                            }
                        }))
                        .child(icon("icons/pencil.svg", pal.dim).size(z(12.))),
                    ("row-rename", ix),
                    None,
                    pal.active,
                    window,
                    cx,
                ))
                .child(hover_bg(
                    div()
                        .id(("row-delete", ix))
                        .role(Role::Button)
                        .aria_label(t(Key::MoveToTrash))
                        .size(z(20.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(6.))
                        .invisible()
                        .group_hover("note-row", |s| s.visible())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.delete_row(path.clone(), kind, window, cx);
                        }))
                        .child(icon("icons/delete.svg", pal.dim).size(z(12.))),
                    ("row-delete", ix),
                    None,
                    pal.active,
                    window,
                    cx,
                ));
        }
        if active {
            pill = pill.bg(rgb(pal.active));
        } else {
            pill = hover_bg(pill, ("row", ix), None, pal.hover, window, cx);
        }
        pill
    }

    pub(crate) fn render_sidebar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = self.flat_rows();
        let count = rows.len();
        let list = uniform_list(
            "notes",
            count,
            cx.processor(move |this, range: Range<usize>, window, cx| {
                let rows = this.flat_rows();
                let current = this.current.as_ref().map(|c| c.path());
                range
                    .map(|ix| {
                        let row = &rows[ix.min(rows.len().saturating_sub(1))];
                        let pill = this.render_row(ix, row, current.as_deref(), window, cx);
                        let (path, kind) = (row.path.clone(), row.kind);
                        // No menu on rows that are not a real file: the
                        // new-folder input, an unsaved pending note and the
                        // row being renamed.
                        let menuable = !path.ends_with(NEW_FOLDER_ROW)
                            && !this.tabs.iter().any(|t| t.pending && t.path() == path)
                            && !this
                                .editing
                                .as_ref()
                                .is_some_and(|e| !e.create && e.target == path);
                        let pill = div().size_full().child(if menuable {
                            let weak = cx.entity().downgrade();
                            let select = path.clone();
                            pill
                                // Right-click selects the row it landed on, so
                                // the menu acts on what was clicked.
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, _, w, cx| {
                                        if kind == NodeKind::Note {
                                            this.open_path(select.clone(), None, w, cx);
                                        }
                                    }),
                                )
                                .ctx_menu(move |m, _, cx| Self::row_menu(&weak, &path, kind, m, cx))
                                .into_any_element()
                        } else {
                            pill.into_any_element()
                        });
                        div().h(z(32.)).px(z(8.)).pb(z(2.)).child(rise(
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
                    .w(z(SIDEBAR_W))
                    .h_full()
                    .flex()
                    .flex_col()
                    // 48px header = toolbar height; drags the window.
                    .child(
                        drag_fallback(div().id("sidebar-head"))
                            .h(z(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(z(2.))
                            .pl(px(chrome_left_pad(true)))
                            .pr(z(9.))
                            .child(
                                self.ring(
                                    0,
                                    hover_bg(
                                        div()
                                            .id("space-switcher")
                                            .role(Role::Button)
                                            .aria_label(tf(Key::SwitchSpace, &[]).as_str())
                                            .flex_1()
                                            .min_w_0()
                                            .h(z(30.))
                                            .px(z(8.))
                                            .flex()
                                            .items_center()
                                            .gap(z(8.))
                                            .rounded(z(6.))
                                            .cursor_pointer()
                                            .occlude()
                                            .active(|s| s.bg(rgb(pal.active)))
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.toggle_spaces(cx)
                                                }),
                                            )
                                            .child(icon("icons/folder.svg", pal.fg).size(z(15.)))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(z(13.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(pal.fg))
                                                    .child(space_name),
                                            )
                                            .child(
                                                icon("icons/chevrons.svg", pal.faint).size(z(14.)),
                                            )
                                            .when_some(
                                                self.mark(
                                                    0,
                                                    Anchor::TopLeft,
                                                    point(z(0.), z(38.)),
                                                    cx,
                                                ),
                                                |s, m| s.child(m),
                                            ),
                                        "space-switcher",
                                        self.spaces_open.then_some(pal.active),
                                        pal.hover,
                                        window,
                                        cx,
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
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.new_folder(window, cx),
                                    )),
                                    &pal,
                                )
                                .when_some(
                                    self.mark(1, Anchor::TopRight, point(z(30.), z(38.)), cx),
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
                                    window,
                                    cx,
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.new_note(window, cx)),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .id("notes-head")
                            .h(z(28.))
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
                            .px(z(18.))
                            .text_size(z(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(pal.faint))
                            .child(t(Key::Notes))
                            .child(notes_n.to_string()),
                    )
                    .child(list)
                    .child(self.render_settings_button(window, cx))
                    .child(self.render_spaces_menu(window, cx)),
            );
        if self.sidebar_gen == 0 {
            return panel.w(z(to)).into_any_element();
        }
        panel
            .with_animation(
                ("sidebar-slide", self.sidebar_gen),
                Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint),
                move |el, d| el.w(z(from + (to - from) * d)),
            )
            .into_any_element()
    }
}
