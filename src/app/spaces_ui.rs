use super::*;

impl AbstractApp {
    // ── Spaces & window chrome ────────────────────────────────────────────

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        self.sidebar_gen += 1;
        self.save_session(cx);
        cx.notify();
    }

    pub(crate) fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_pref = self.theme_pref.next();
        theme::apply(self.theme_pref, window.appearance(), cx);
        self.settings.set_theme(self.theme_pref);
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    /// Switch to `spaces.current()`: flush the open note, persist the list,
    /// scan the folder off-thread, then reopen where the session left off.
    pub(crate) fn enter_space(
        &mut self,
        spaces: Spaces,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.record_session_note(cx);
        self.save_session(cx);
        self.flush(cx);
        self._save_task = None;
        self.editing = None;
        self.pending_new = None;
        self.target_folder = None;
        self.spaces = spaces;
        self.spaces_open = false;
        self.dir = self.spaces.current().to_path_buf();
        self.tree.clear();
        self.expanded.clear();
        self.current = None;
        self.loading = true;
        let dir = self.dir.clone();
        let snapshot = self.spaces.clone();
        self._io_task = Some(cx.spawn_in(window, async move |this, cx| {
            let scanned = cx
                .background_executor()
                .spawn(async move {
                    spaces::save(&snapshot);
                    vault::scan(&dir)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.loading = false;
                match scanned {
                    Ok(tree) => this.tree = tree,
                    Err(err) => eprintln!("abstract: cannot read space: {err}"),
                }
                // Session's last note for this space, else the newest.
                let restore = this
                    .session_notes
                    .iter()
                    .rfind(|n| n.space == this.dir)
                    .map(|n| (this.dir.join(&n.rel), n.cursor, n.scroll));
                match restore {
                    Some((p, cursor, scroll)) if vault::contains(&this.tree, &p) => {
                        this.open_path(p, Some((cursor, scroll)), window, cx)
                    }
                    _ => match vault::newest_note(&this.tree, None) {
                        Some(p) => this.open_path(p, None, window, cx),
                        None => this.new_note(window, cx),
                    },
                }
                if !this.tour_shown && !this.settings.tour_done() {
                    this.start_tour(window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    pub(crate) fn switch_space(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix == self.spaces.active {
            self.spaces_open = false;
            cx.notify();
            return;
        }
        let mut spaces = self.spaces.clone();
        spaces.active = ix;
        self.enter_space(spaces, window, cx);
    }

    pub(crate) fn remove_space(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut spaces = self.spaces.clone();
        let was_active = ix == spaces.active;
        if !spaces.remove(ix) {
            return;
        }
        if was_active {
            self.enter_space(spaces, window, cx);
        } else {
            self.spaces = spaces;
            let snapshot = self.spaces.clone();
            cx.background_spawn(async move { spaces::save(&snapshot) })
                .detach();
            cx.notify();
        }
    }

    /// Native folder picker; the chosen folder becomes (or reactivates) a space.
    pub(crate) fn open_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.spaces_open = false;
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Abrir como espaço".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let mut spaces = this.spaces.clone();
                spaces.activate_path(path);
                this.enter_space(spaces, window, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn toggle_spaces(&mut self, cx: &mut Context<Self>) {
        self.spaces_open = !self.spaces_open;
        cx.notify();
    }

    pub(crate) fn toggle_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.expanded.remove(&path) {
            self.expanded.insert(path.clone());
        }
        self.target_folder = Some(path);
        cx.notify();
    }
    pub(crate) fn render_spaces_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        let removable = self.spaces.paths.len() > 1;
        let mut rows = div().flex().flex_col().gap(px(2.)).p(px(4.));
        for (ix, path) in self.spaces.paths.iter().enumerate() {
            let active = ix == self.spaces.active;
            rows = rows.child(
                div()
                    .id(("space", ix))
                    .group("space-row")
                    .role(Role::MenuItem)
                    .aria_label(SharedString::from(spaces::name_of(path)))
                    .h(px(40.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.switch_space(ix, window, cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.))
                                    .line_height(px(17.))
                                    .text_color(rgb(if active { pal.fg } else { pal.body }))
                                    .child(SharedString::from(spaces::name_of(path))),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .line_height(px(15.))
                                    .text_color(rgb(pal.faint))
                                    .child(SharedString::from(path.display().to_string())),
                            ),
                    )
                    .when(active, |row| {
                        row.child(icon("icons/check.svg", pal.fg).size(px(14.)))
                    })
                    .when(!active && removable, |row| {
                        row.child(
                            div()
                                .id(("space-remove", ix))
                                .role(Role::Button)
                                .aria_label("Remover da lista (os arquivos ficam)")
                                .size(px(22.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .invisible()
                                .group_hover("space-row", |s| s.visible())
                                .hover(|s| s.bg(rgb(pal.active)))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.remove_space(ix, window, cx);
                                }))
                                .child(icon("icons/close.svg", pal.dim).size(px(12.))),
                        )
                    }),
            );
        }
        let menu = div()
            .id("spaces-menu")
            .role(Role::Menu)
            .absolute()
            .top(px(44.))
            .left(px(8.))
            .w(px(SIDEBAR_W - 16.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(px(8.))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.spaces_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .px(px(12.))
                    .pt(px(10.))
                    .pb(px(2.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(pal.faint))
                    .child("ESPAÇOS"),
            )
            .child(rows)
            .child(div().h(px(1.)).bg(rgb(pal.line)))
            .child(
                div().p(px(4.)).child(
                    div()
                        .id("open-space")
                        .role(Role::MenuItem)
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(rgb(pal.body))
                        .hover(|s| s.bg(rgb(pal.hover)))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(|this, _, window, cx| this.open_space(window, cx)))
                        .child(icon("icons/folder-add.svg", pal.dim).size(px(15.)))
                        .child("Abrir pasta como espaço…"),
                ),
            );
        menu.with_animation(
            "spaces-menu-in",
            Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint),
            |el, d| el.opacity(d).top(px(38. + 6. * d)),
        )
    }
}
