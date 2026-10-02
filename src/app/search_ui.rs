use super::*;

pub(super) struct SearchPalette {
    state: Entity<InputState>,
    hits: Vec<crate::search::Hit>,
    /// `>` mode: matched commands (note hits stay empty). `None` = note mode.
    commands: Option<Vec<AppCommand>>,
    selected: usize,
    generation: u64,
    _sub: Subscription,
    _task: Option<Task<()>>,
}

impl AbstractApp {
    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.is_some() {
            return;
        }
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(t(Key::SearchPlaceholder)));
        let sub = cx.subscribe(&state, |this: &mut Self, _, ev: &InputEvent, cx| match ev {
            InputEvent::Change => this.search_changed(cx),
            InputEvent::PressEnter { .. } => {
                cx.spawn(async move |this, cx| {
                    this.update_in(cx, |this, window, cx| this.open_selected(window, cx))
                        .ok();
                })
                .detach();
            }
            _ => {}
        });
        state.update(cx, |s, cx| s.focus(window, cx));
        self.search = Some(SearchPalette {
            state: state.clone(),
            hits: Vec::new(),
            commands: None,
            selected: 0,
            generation: 0,
            _sub: sub,
            _task: None,
        });
        self.run_search(cx);
        cx.notify();
    }

    fn close_search(&mut self, cx: &mut Context<Self>) {
        if self.search.take().is_some() {
            self.focus_editor(cx);
            cx.notify();
        }
    }

    /// (Re)run the query off-thread; a bumped `gen` discards stale results.
    /// A leading `>` switches to command mode — matched locally, no I/O.
    fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(p) = &mut self.search else { return };
        let query = p.state.read(cx).value().to_string();
        p.generation += 1;
        let generation = p.generation;
        p.selected = 0;
        if let Some(cmd) = query.trim_start().strip_prefix('>') {
            p.hits = Vec::new();
            p.commands = Some(AppCommand::matching(cmd));
            p._task = None;
            return;
        }
        p.commands = None;
        let dir = self.dir.clone();
        p._task = Some(cx.spawn(async move |this, cx| {
            let hits = cx
                .background_executor()
                .spawn(async move { crate::search::search(&dir, &query) })
                .await;
            this.update(cx, |this, cx| {
                if let Some(p) = &mut this.search
                    && p.generation == generation
                {
                    p.hits = hits;
                    p.selected = 0;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    /// Editor menu: open the palette prefilled with the selection.
    pub(crate) fn search_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.editor.read(cx).selected_text().trim().to_string();
        if query.is_empty() {
            return;
        }
        self.open_search(window, cx);
        if let Some(p) = &self.search {
            p.state.update(cx, |s, cx| {
                s.set_value(query, window, cx);
                s.focus(window, cx);
            });
        }
        self.search_changed(cx);
    }

    fn search_changed(&mut self, cx: &mut Context<Self>) {
        self.run_search(cx);
        cx.notify();
    }

    fn move_search(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(p) = &mut self.search else { return };
        let n = p.commands.as_ref().map_or(p.hits.len(), |c| c.len());
        if n == 0 {
            return;
        }
        p.selected = if down {
            (p.selected + 1) % n
        } else {
            (p.selected + n - 1) % n
        };
        cx.notify();
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = &self.search else { return };
        if let Some(cmds) = &p.commands {
            let cmd = cmds.get(p.selected).copied();
            self.close_search(cx);
            if let Some(cmd) = cmd {
                let editor = self.editor.read(cx).focus_handle(cx);
                // Action listeners re-enter AbstractApp — dispatch after this update.
                window.defer(cx, move |window, cx| cmd.dispatch(&editor, window, cx));
            }
            return;
        }
        let Some(hit) = self
            .search
            .as_ref()
            .and_then(|p| p.hits.get(p.selected))
            .map(|h| (h.path.clone(), h.line))
        else {
            self.close_search(cx);
            return;
        };
        self.close_search(cx);
        let restore = crate::search::offset_of_line(&hit.0, hit.1).map(|o| (o, 0.));
        self.open_path(hit.0, restore, window, cx);
    }

    pub(crate) fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        let Some(p) = &self.search else {
            return div().into_any_element();
        };
        let mut rows = div()
            .id("palette-rows")
            .flex()
            .flex_col()
            .max_h(px(320.))
            .overflow_y_scroll();
        if let Some(cmds) = &p.commands {
            for (ix, cmd) in cmds.iter().enumerate() {
                let selected = ix == p.selected;
                rows = rows.child(
                    div()
                        .id(("hit", ix))
                        .role(Role::ListBoxOption)
                        .aria_label(t(cmd.label()))
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(selected, |s| s.bg(rgb(pal.active)))
                        .when(!selected, |s| s.hover(|s| s.bg(rgb(pal.hover))))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(p) = &mut this.search {
                                p.selected = ix;
                            }
                            this.open_selected(window, cx);
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .line_height(px(17.))
                                .text_color(rgb(if selected { pal.fg } else { pal.body }))
                                .child(t(cmd.label())),
                        )
                        .when_some(cmd.hint().map(SharedString::from), |r, hint| {
                            r.child(
                                div()
                                    .flex_none()
                                    .text_size(px(11.))
                                    .text_color(rgb(pal.faint))
                                    .child(hint),
                            )
                        }),
                );
            }
            if cmds.is_empty() {
                rows = rows.child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .text_size(px(12.))
                        .text_color(rgb(pal.faint))
                        .child(t(Key::NoCommands)),
                );
            }
        } else {
            for (ix, hit) in p.hits.iter().enumerate() {
                let selected = ix == p.selected;
                let title = div()
                    .truncate()
                    .text_size(px(13.))
                    .line_height(px(17.))
                    .text_color(rgb(if selected { pal.fg } else { pal.body }))
                    .child(hit.title.clone());
                let mut snippet = div()
                    .flex()
                    .overflow_hidden()
                    .text_size(px(11.))
                    .line_height(px(15.))
                    .text_color(rgb(pal.faint));
                if hit.snippet.is_empty() {
                    snippet = snippet.child(div());
                } else {
                    let q = p.state.read(cx).value().to_string().trim().to_lowercase();
                    match crate::search::match_range(&hit.snippet, &q) {
                        Some(r) if !q.is_empty() => {
                            let (a, b, c) = (
                                hit.snippet[..r.start].to_string(),
                                hit.snippet[r.clone()].to_string(),
                                hit.snippet[r.end..].to_string(),
                            );
                            snippet = snippet
                                .child(div().child(a))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(pal.body))
                                        .child(b),
                                )
                                .child(div().child(c));
                        }
                        _ => {
                            snippet = snippet.child(div().truncate().child(hit.snippet.clone()));
                        }
                    }
                }
                rows = rows.child(
                    div()
                        .id(("hit", ix))
                        .role(Role::ListBoxOption)
                        .aria_label(hit.title.clone())
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(selected, |s| s.bg(rgb(pal.active)))
                        .when(!selected, |s| s.hover(|s| s.bg(rgb(pal.hover))))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(p) = &mut this.search {
                                p.selected = ix;
                            }
                            this.open_selected(window, cx);
                        }))
                        .child(title)
                        .when(!hit.snippet.is_empty(), |r| r.child(snippet)),
                );
            }
            if p.hits.is_empty() {
                rows = rows.child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .text_size(px(12.))
                        .text_color(rgb(pal.faint))
                        .child(t(Key::NoNotesFound)),
                );
            }
        }
        div()
            .id("search-palette")
            .key_context("SearchPalette")
            .absolute()
            .top(px(80.))
            .left_1_2()
            .ml(px(-280.))
            .w(px(560.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(px(8.))
            .shadow_lg()
            .occlude()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                match ev.keystroke.key.as_str() {
                    "up" => {
                        cx.stop_propagation();
                        this.move_search(false, cx);
                    }
                    "down" => {
                        cx.stop_propagation();
                        this.move_search(true, cx);
                    }
                    _ => {}
                }
            }))
            .on_action(cx.listener(|this, _: &input::Escape, _, cx| this.close_search(cx)))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_search(cx)))
            .child(
                div().p(px(4.)).child(
                    Input::new(&p.state)
                        .appearance(false)
                        .bordered(false)
                        .h(px(30.))
                        .w_full()
                        .text_size(px(14.))
                        .text_color(rgb(pal.fg)),
                ),
            )
            .child(div().h(px(1.)).bg(rgb(pal.line)))
            .child(div().p(px(4.)).child(rows))
            .with_animation(
                "search-palette-in",
                Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint),
                |el, d| el.opacity(d).top(px(74. + 6. * d)),
            )
            .into_any_element()
    }
}
