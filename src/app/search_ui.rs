use super::*;

pub(super) struct SearchPalette {
    state: Entity<InputState>,
    hits: Vec<crate::search::Hit>,
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
    fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(p) = &mut self.search else { return };
        let query = p.state.read(cx).value().to_string();
        let dir = self.dir.clone();
        p.generation += 1;
        let generation = p.generation;
        p.selected = 0;
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
        if p.hits.is_empty() {
            return;
        }
        let n = p.hits.len();
        p.selected = if down {
            (p.selected + 1) % n
        } else {
            (p.selected + n - 1) % n
        };
        cx.notify();
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let mut rows = div().flex().flex_col().max_h(z(320.)).overflow_hidden();
        for (ix, hit) in p.hits.iter().enumerate() {
            let selected = ix == p.selected;
            let title = div()
                .truncate()
                .text_size(z(13.))
                .line_height(z(17.))
                .text_color(rgb(if selected { pal.fg } else { pal.body }))
                .child(hit.title.clone());
            let mut snippet = div()
                .flex()
                .overflow_hidden()
                .text_size(z(11.))
                .line_height(z(15.))
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
                    .px(z(10.))
                    .py(z(6.))
                    .flex()
                    .flex_col()
                    .gap(z(2.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .when(selected, |s| s.bg(rgb(pal.active)))
                    .when(!selected, |s| s.hover(|s| s.bg(rgb(pal.hover))))
                    .active(|s| s.bg(rgb(pal.active)))
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
                    .px(z(10.))
                    .py(z(8.))
                    .text_size(z(12.))
                    .text_color(rgb(pal.faint))
                    .child(t(Key::NoNotesFound)),
            );
        }
        div()
            .id("search-palette")
            .key_context("SearchPalette")
            .absolute()
            .top(z(80.))
            .left_1_2()
            .ml(z(-280.))
            .w(z(560.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(z(8.))
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
                div().p(z(4.)).child(
                    Input::new(&p.state)
                        .appearance(false)
                        .bordered(false)
                        .h(z(30.))
                        .w_full()
                        .text_size(z(14.))
                        .text_color(rgb(pal.fg)),
                ),
            )
            .child(div().h(z(1.)).bg(rgb(pal.line)))
            .child(div().p(z(4.)).child(rows))
            .with_animation(
                "search-palette-in",
                Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint),
                |el, d| el.opacity(d).top(z(74. + 6. * d)),
            )
            .into_any_element()
    }
}
