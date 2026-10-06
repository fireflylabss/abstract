use super::*;

impl AbstractApp {
    // ── Spaces & window chrome ────────────────────────────────────────────

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        self.sidebar_gen += 1;
        self.save_session(cx);
        cx.notify();
    }

    /// Idioma row in the spaces menu: Sistema → English → Português (Brasil).
    pub(crate) fn cycle_lang(&mut self, cx: &mut Context<Self>) {
        let next = self.settings.lang().next();
        self.settings.set_lang(next);
        i18n::set(match next {
            i18n::LangPref::System => i18n::detect(),
            i18n::LangPref::En => i18n::Lang::En,
            i18n::LangPref::PtBr => i18n::Lang::PtBr,
        });
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    pub(crate) fn lang_label(&self) -> &'static str {
        match self.settings.lang() {
            i18n::LangPref::System => t(Key::LangSystem),
            i18n::LangPref::En => "English",
            i18n::LangPref::PtBr => "Português (Brasil)",
        }
    }

    pub(crate) fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_theme_pref(self.theme_pref.next(), window, cx);
    }

    pub(crate) fn set_theme_pref(
        &mut self,
        pref: ThemePref,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_pref = pref;
        self.settings.set_theme(pref);
        self.save_settings(window, cx);
    }

    /// Re-apply the palette and persist `settings` off-thread.
    pub(crate) fn save_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_theme(window, cx);
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    /// `theme::apply` plus a crossfade: keep drawing the old palette while a
    /// task lerps the global to the new one over MOTION_IN_MS (snaps under
    /// reduced motion or when the palette didn't actually change).
    pub(crate) fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = *cx.global::<Palette>();
        theme::apply(&self.settings, window.appearance(), cx);
        let to = *cx.global::<Palette>();
        if from == to || cx.reduce_motion() {
            return;
        }
        cx.set_global(from);
        let start = Instant::now();
        self._pal_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let t = ease_out_quint(
                    (start.elapsed().as_secs_f32() * 1000. / MOTION_IN_MS as f32).min(1.),
                );
                let gone = this
                    .update(cx, |_, cx| {
                        cx.set_global(theme::lerp(&from, &to, t));
                        cx.notify();
                    })
                    .is_err();
                if gone || t >= 1. {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            }
        }));
    }

    /// Switch to `spaces.current()`: flush the open note, persist the list,
    /// scan the folder off-thread, then reopen where the session left off.
    pub(crate) fn enter_space(
        &mut self,
        spaces: Spaces,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Snapshot the incoming space's session state before the flush below
        // rewrites it — `tab`/`tab_active` lines for this space would
        // otherwise be replaced by whatever is open right now.
        let new_dir = spaces.current().to_path_buf();
        let saved: Vec<(PathBuf, usize, f32)> = self
            .session_notes
            .iter()
            .filter(|n| n.space == new_dir)
            .map(|n| (new_dir.join(&n.rel), n.cursor, n.scroll))
            .collect();
        let active = self
            .session
            .active_tab()
            .filter(|(s, _)| *s == new_dir)
            .map(|(_, r)| new_dir.join(r));
        self.save_session(cx);
        self.flush_all(cx);
        self.editing = None;
        self.target_folder = None;
        self.spaces = spaces;
        self.spaces_open = false;
        self.dir = self.spaces.current().to_path_buf();
        self.start_watch(cx);
        self.tree.clear();
        self.expanded.clear();
        // The old space's tabs die here; the mirrors reset when the new
        // space's tabs are restored (or a note is opened) below.
        self.tabs.clear();
        self.active = None;
        self.closed_tabs.clear();
        self.trash_undo = None;
        self.closing_tabs.clear();
        self._scratch_subs = None;
        self.current = None;
        self.editor = self.scratch_editor(cx);
        self.save = SaveState::Saved;
        self.words = 0;
        self.completion = None;
        self.backlinks.clear();
        self.backlinks_key = None;
        self.presence.set(None, spaces::name_of(&self.dir));
        self.loading = true;
        let dir = self.dir.clone();
        let snapshot = self.spaces.clone();
        self._io_task = Some(cx.spawn_in(window, async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move {
                    spaces::save(&snapshot);
                    let tree = vault::scan(&dir);
                    let reads: Vec<(PathBuf, String, Option<SystemTime>, usize, f32)> = saved
                        .into_iter()
                        .filter_map(|(p, cursor, scroll)| {
                            std::fs::read_to_string(&p).ok().map(|text| {
                                let m = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
                                (p, text, m, cursor, scroll)
                            })
                        })
                        .collect();
                    (tree, reads)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.loading = false;
                match read.0 {
                    Ok(tree) => this.tree = tree,
                    Err(err) => eprintln!("abstract: cannot read space: {err}"),
                }
                for (path, text, mtime, cursor, scroll) in read.1 {
                    let synced = vault::synced_stem(&stem_of(&path), &title_of(&text));
                    this.push_tab(
                        NoteFile {
                            path,
                            mtime,
                            deleted: false,
                        },
                        synced,
                        text,
                        Some((cursor, scroll)),
                        false,
                        cx,
                    );
                }
                if !this.tabs.is_empty() {
                    let ix = active
                        .and_then(|p| this.tabs.iter().position(|t| t.path() == p))
                        .unwrap_or(this.tabs.len() - 1);
                    this.activate(ix, window, cx);
                } else {
                    match vault::newest_note(&this.tree, None) {
                        Some(p) => this.open_path(p, None, window, cx),
                        None => this.new_note(window, cx),
                    }
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
            prompt: Some(t(Key::OpenAsSpace).into()),
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
    pub(crate) fn render_spaces_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pal = cx.palette();
        let removable = self.spaces.paths.len() > 1;
        let mut rows = div().flex().flex_col().gap(z(2.)).p(z(4.));
        for (ix, path) in self.spaces.paths.iter().enumerate() {
            let active = ix == self.spaces.active;
            rows = rows.child(hover_bg(
                div()
                    .id(("space", ix))
                    .group("space-row")
                    .role(Role::MenuItem)
                    .aria_label(SharedString::from(spaces::name_of(path)))
                    .h(z(40.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .rounded(z(6.))
                    .cursor_pointer()
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
                                    .text_size(z(13.))
                                    .line_height(z(17.))
                                    .text_color(rgb(if active { pal.fg } else { pal.body }))
                                    .child(SharedString::from(spaces::name_of(path))),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(z(11.))
                                    .line_height(z(15.))
                                    .text_color(rgb(pal.faint))
                                    .child(SharedString::from(path.display().to_string())),
                            ),
                    )
                    .when(active, |row| {
                        row.child(icon("icons/check.svg", pal.fg).size(z(14.)))
                    })
                    .when(!active && removable, |row| {
                        row.child(hover_bg(
                            div()
                                .id(("space-remove", ix))
                                .role(Role::Button)
                                .aria_label(t(Key::RemoveFromList))
                                .size(z(22.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(z(6.))
                                .invisible()
                                .group_hover("space-row", |s| s.visible())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.remove_space(ix, window, cx);
                                }))
                                .child(icon("icons/close.svg", pal.dim).size(z(12.))),
                            ("space-remove", ix),
                            None,
                            pal.active,
                            window,
                            cx,
                        ))
                    }),
                ("space", ix),
                None,
                pal.hover,
                window,
                cx,
            ));
        }
        let ps = presence("spaces-menu", self.spaces_open, window, cx);
        if !ps.should_render() {
            return div().into_any_element();
        }
        let menu = div()
            .id("spaces-menu")
            .role(Role::Menu)
            .absolute()
            .top(z(38. + 6. * ps.progress))
            .opacity(ps.progress)
            .left(z(8.))
            .w(z(SIDEBAR_W - 16.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(z(8.))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.spaces_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .px(z(12.))
                    .pt(z(10.))
                    .pb(z(2.))
                    .text_size(z(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(pal.faint))
                    .child(t(Key::Spaces)),
            )
            .child(rows)
            .child(div().h(z(1.)).bg(rgb(pal.line)))
            .child(
                div().p(z(4.)).child(hover_bg(
                    div()
                        .id("open-space")
                        .role(Role::MenuItem)
                        .h(z(32.))
                        .px(z(8.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .rounded(z(6.))
                        .cursor_pointer()
                        .text_size(z(13.))
                        .text_color(rgb(pal.body))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(|this, _, window, cx| this.open_space(window, cx)))
                        .child(icon("icons/folder-add.svg", pal.dim).size(z(15.)))
                        .child(t(Key::OpenFolderAsSpace)),
                    "open-space",
                    None,
                    pal.hover,
                    window,
                    cx,
                )),
            );
        menu.into_any_element()
    }
}
