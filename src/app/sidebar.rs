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
        if let Some(p) = &self.pending_new
            && !rows.iter().any(|r| r.path == *p)
            && let Some(parent) = p.parent()
        {
            let ix = insert(&mut rows, parent);
            rows[ix].path = p.clone();
            rows[ix].name = "Sem título".into();
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
                icon("icons/note.svg", if active { pal.fg } else { pal.faint })
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
                        .aria_label("Renomear")
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
                        .aria_label("Mover para a Lixeira")
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
                    // 48px header = toolbar height; empty area drags the window.
                    .child(
                        titlebar_drag(div().id("sidebar-head"))
                            .h(px(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .pl(px(9.))
                            .pr(px(9.))
                            .child(
                                self.ring(
                                    0,
                                    div()
                                        .id("space-switcher")
                                        .role(Role::Button)
                                        .aria_label(format!("Trocar de espaço ({MOD}+O)").as_str())
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(30.))
                                        .px(px(8.))
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
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
                                        "Nova pasta".into(),
                                        false,
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
                                    format!("Nova nota ({MOD}+N)").into(),
                                    false,
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.new_note(window, cx)),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(28.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(18.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(pal.faint))
                            .child("NOTAS")
                            .child(notes_n.to_string()),
                    )
                    .child(list)
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
}
