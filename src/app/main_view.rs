use super::*;

impl AbstractApp {
    pub(crate) fn render_main(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pal = cx.palette();
        let has_note = self.current.is_some();
        let status: SharedString = self.notice.clone().unwrap_or_else(|| match self.save {
            SaveState::Pending => t(Key::Saving).into(),
            SaveState::Failed => t(Key::SaveFailed).into(),
            SaveState::Saved => tf(Key::Words, &[("n", &self.words.to_string())]).into(),
        });
        let theme_tip = SharedString::from(tf(
            Key::Theme,
            &[(
                "name",
                match self.theme_pref {
                    ThemePref::System => t(Key::ThemeSystem),
                    ThemePref::Light => t(Key::ThemeLight),
                    ThemePref::Dark => t(Key::ThemeDark),
                },
            )],
        ));

        // Focus mode: the status chip goes quiet until hovered.
        let status_dim = base::motion::transition(
            ("status", "focus-dim"),
            if self.chrome_hidden() { 0.35 } else { 1. },
            base::motion::Transition::new(Duration::from_millis(MOTION_IN_MS)).ease(ease_out_quint),
            window,
            cx,
        );
        let toolbar = drag_fallback(div().id("toolbar"))
            .h(z(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap(z(2.))
            .pl(px(chrome_left_pad(!self.sidebar_visible())))
            .pr(z(9.))
            .child(
                self.ring(
                    5,
                    icon_btn(
                        "toggle-sidebar",
                        "icons/sidebar.svg",
                        tf(Key::Sidebar, &[]).into(),
                        !self.sidebar_visible(),
                        &pal,
                        window,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    &pal,
                )
                .when_some(
                    self.mark(5, Anchor::TopLeft, point(z(0.), z(38.)), cx),
                    |s, m| s.child(m),
                ),
            )
            .child(
                icon_btn(
                    "search",
                    "icons/search.svg",
                    tf(Key::Search, &[]).into(),
                    false,
                    &pal,
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx))),
            )
            .child(titlebar_drag(div().id("toolbar-drag")).flex_1().h_full())
            .child(rise(
                self.ring(
                    3,
                    hover_bg(
                        div()
                            .id("status")
                            .role(Role::Button)
                            .aria_label(t(Key::NoteStatus))
                            .debug_selector(|| "status".into())
                            .w(z(140.))
                            .h(z(28.))
                            .flex()
                            .items_center()
                            .justify_end()
                            .px(z(8.))
                            .rounded(z(6.))
                            .cursor_pointer()
                            .active(|s| s.bg(rgb(pal.active)))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.status_open = !this.status_open;
                                cx.notify();
                            }))
                            .text_size(z(12.))
                            .text_color(rgb(if self.save == SaveState::Failed {
                                pal.fg
                            } else {
                                pal.faint
                            }))
                            .opacity(status_dim)
                            .hover(|s| s.opacity(1.))
                            .child(status),
                        "status",
                        self.status_open.then_some(pal.active),
                        pal.hover,
                        window,
                        cx,
                    ),
                    &pal,
                )
                .when_some(
                    self.mark(3, Anchor::TopRight, point(z(140.), z(34.)), cx),
                    |s, m| s.child(m),
                ),
                (
                    "status",
                    self.save as usize * 2 + self.notice.is_some() as usize,
                ),
                220,
                0.,
                2.,
            ))
            .child(
                self.ring(
                    4,
                    icon_btn(
                        "theme",
                        self.theme_pref.icon(),
                        theme_tip,
                        false,
                        &pal,
                        window,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.cycle_theme(window, cx))),
                    &pal,
                )
                .when_some(
                    self.mark(4, Anchor::TopRight, point(z(30.), z(38.)), cx),
                    |s, m| s.child(m),
                ),
            )
            .child(
                icon_btn(
                    "focus",
                    "icons/focus.svg",
                    tf(Key::FocusMode, &[]).into(),
                    self.chrome_hidden(),
                    &pal,
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_focus(cx))),
            )
            .when(has_note, |bar| {
                bar.child(
                    icon_btn(
                        "delete",
                        "icons/delete.svg",
                        tf(Key::DeleteNote, &[]).into(),
                        false,
                        &pal,
                        window,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.delete_note(window, cx))),
                )
            })
            .when(cfg!(not(target_os = "macos")), |t| {
                t.child(window_controls(window, &pal, cx))
            });

        let body = if self.loading {
            div().flex_1().into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(
                    div()
                        .id("editor-col")
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .when(has_note, |s| {
                            s.child(div().flex_1().min_h_0().w_full().child(rise(
                                div().size_full().child(self.editor.clone()),
                                ("editor-in", self.open_gen),
                                420,
                                0.,
                                10.,
                            )))
                        })
                        .when_some(self.render_backlinks(window, cx), |s, m| s.child(m))
                        .when_some(
                            self.mark(2, Anchor::TopLeft, point(z(70.), z(70.)), cx),
                            |s, m| s.child(m),
                        )
                        .when_some(self.render_completion(window, cx), |s, m| s.child(m))
                        .when_some(self.render_find(window, cx), |s, m| s.child(m)),
                )
                .into_any_element()
        };

        div()
            .id("app-root")
            .track_focus(&self.empty_focus)
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .when_some(self.render_tabs(cx), |s, tabs| s.child(tabs))
            .child(body)
            .when_some(self.search.as_ref(), |el, _| {
                el.child(self.render_search(window, cx))
            })
            .child(self.render_status(has_note, window, cx))
            .when_some(self.render_update(window, cx), |el, card| el.child(card))
            .when_some(self.crash.clone(), |el, c| {
                el.child(self.render_crash(c, window, cx))
            })
    }

    fn render_crash(
        &self,
        crash: crate::crash::Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pal = cx.palette();
        let button = |id: &'static str, label: &'static str, window: &mut Window, cx: &mut App| {
            hover_bg(
                div()
                    .id(id)
                    .role(Role::Button)
                    .aria_label(label)
                    .h(z(26.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(12.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .child(label),
                id,
                None,
                pal.hover,
                window,
                cx,
            )
        };
        let report = crash.text.clone();
        let issue = crate::crash::issue_url(&crash.text);
        let path = crash.path.clone();
        rise(
            div()
                .id("crash-report")
                .role(Role::Dialog)
                .aria_label(t(Key::CrashTitle))
                .absolute()
                .top(z(52.))
                .right(z(12.))
                .w(z(320.))
                .p(z(14.))
                .flex()
                .flex_col()
                .gap(z(6.))
                .bg(rgb(pal.menu_bg))
                .border_1()
                .border_color(rgb(pal.menu_border))
                .rounded(z(8.))
                .shadow_lg()
                .occlude()
                .child(
                    div()
                        .text_size(z(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(pal.fg))
                        .child(t(Key::CrashTitle)),
                )
                .child(
                    div()
                        .text_size(z(12.))
                        .line_height(z(18.))
                        .text_color(rgb(pal.dim))
                        .child(t(Key::CrashBody)),
                )
                .when(crash.recovered, |el| {
                    el.child(
                        div()
                            .text_size(z(12.))
                            .line_height(z(18.))
                            .text_color(rgb(pal.dim))
                            .child(t(Key::CrashRecovered)),
                    )
                })
                .child(
                    div()
                        .mt(z(4.))
                        .flex()
                        .flex_wrap()
                        .gap(z(4.))
                        .child(
                            button("crash-copy", t(Key::CopyReport), window, cx).on_click(
                                move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(report.clone()))
                                },
                            ),
                        )
                        .child(
                            button("crash-issue", t(Key::OpenIssue), window, cx)
                                .on_click(move |_, _, cx| cx.open_url(&issue)),
                        )
                        .child(
                            button("crash-reveal", t(Key::Reveal), window, cx)
                                .on_click(move |_, _, cx| cx.reveal_path(&path)),
                        )
                        .child(
                            button("crash-dismiss", t(Key::Dismiss), window, cx).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.crash = None;
                                    cx.notify();
                                }),
                            ),
                        ),
                ),
            "crash-report",
            220,
            0.,
            6.,
        )
    }

    fn render_status(
        &self,
        has_note: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pal = cx.palette();
        let editor = self.editor.read(cx);
        let chars = editor.text().chars().count();
        let selected = editor.selected_text().split_whitespace().count();
        let path = self.current.as_ref().map(|c| c.path());
        let mtime = self.current.as_ref().and_then(|c| guard(&c.file).mtime);
        let (dot, state) = match self.save {
            SaveState::Saved => (pal.faint, t(Key::Saved)),
            SaveState::Pending => (pal.dim, t(Key::Saving)),
            SaveState::Failed => (pal.fg, t(Key::SaveFailed)),
        };
        let name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t(Key::Untitled).into());
        let folder = path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| {
                let rel = p.strip_prefix(&self.dir).unwrap_or(p);
                let space = spaces::name_of(&self.dir);
                if rel.as_os_str().is_empty() {
                    space
                } else {
                    format!("{space}/{}", rel.display())
                }
            })
            .unwrap_or_default();
        let modified = match mtime {
            Some(m) => tf(Key::Modified, &[("when", &ago(m))]),
            None => t(Key::NotSavedYet).into(),
        };
        let stat = |label: String| {
            div()
                .text_size(z(12.))
                .line_height(z(18.))
                .text_color(rgb(pal.dim))
                .child(label)
        };
        let action = |id: &'static str, label: &'static str, window: &mut Window, cx: &mut App| {
            hover_bg(
                div()
                    .id(id)
                    .role(Role::Button)
                    .aria_label(label)
                    .h(z(26.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(12.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .child(label),
                id,
                None,
                pal.hover,
                window,
                cx,
            )
        };
        let ps = presence("status", self.status_open, window, cx);
        if !ps.should_render() {
            return div().into_any_element();
        }
        div()
            .id("status-menu")
            .role(Role::Dialog)
            .aria_label(t(Key::NoteStatus))
            .absolute()
            .top(z(38. + 6. * ps.progress))
            .opacity(ps.progress)
            .right(z(if has_note { 72. } else { 40. }))
            .w(z(240.))
            .p(z(14.))
            .flex()
            .flex_col()
            .gap(z(2.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(z(8.))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.status_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(6.))
                    .text_size(z(11.))
                    .text_color(rgb(pal.faint))
                    .child(div().size(z(6.)).rounded_full().bg(rgb(dot)))
                    .child(state),
            )
            .child(
                div()
                    .mt(z(4.))
                    .truncate()
                    .text_size(z(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(pal.fg))
                    .child(name),
            )
            .child(
                div()
                    .truncate()
                    .text_size(z(11.))
                    .text_color(rgb(pal.faint))
                    .child(folder),
            )
            .child(div().h(z(1.)).my(z(8.)).bg(rgb(pal.line)))
            .child(stat(tf(Key::Words, &[("n", &self.words.to_string())])))
            .child(stat(tf(Key::Characters, &[("n", &chars.to_string())])))
            .child(stat(tf(
                Key::ReadingTime,
                &[("n", &self.words.div_ceil(200).max(1).to_string())],
            )))
            .when(selected > 0, |el| {
                el.child(stat(tf(
                    Key::SelectionWords,
                    &[("n", &selected.to_string())],
                )))
            })
            .child(stat(modified))
            .when_some(path.filter(|p| p.exists()), |el, p| {
                let copy = p.clone();
                el.child(div().h(z(1.)).my(z(8.)).bg(rgb(pal.line))).child(
                    div()
                        .flex()
                        .gap(z(4.))
                        .child(
                            action("status-reveal", t(Key::Reveal), window, cx)
                                .on_click(move |_, _, cx| cx.reveal_path(&p)),
                        )
                        .child(
                            action("status-copy-path", t(Key::CopyPath), window, cx).on_click(
                                move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        copy.display().to_string(),
                                    ))
                                },
                            ),
                        ),
                )
            })
            .into_any_element()
    }
}

/// "3 min ago"-style age of `at`.
fn ago(at: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(at)
        .map_or(0, |d| d.as_secs());
    let n = |v: u64| v.to_string();
    match secs {
        0..60 => t(Key::JustNow).into(),
        60..3600 => tf(Key::MinutesAgo, &[("n", &n(secs / 60))]),
        3600..86400 => tf(Key::HoursAgo, &[("n", &n(secs / 3600))]),
        _ => tf(Key::DaysAgo, &[("n", &n(secs / 86400))]),
    }
}
