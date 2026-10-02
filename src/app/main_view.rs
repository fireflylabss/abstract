use super::*;

impl AbstractApp {
    pub(crate) fn render_main(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        let toolbar = drag_fallback(div().id("toolbar"))
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .pl(px(chrome_left_pad(!self.sidebar_open)))
            .pr(px(9.))
            .child(
                self.ring(
                    5,
                    icon_btn(
                        "toggle-sidebar",
                        "icons/sidebar.svg",
                        tf(Key::Sidebar, &[]).into(),
                        !self.sidebar_open,
                        &pal,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    &pal,
                )
                .when_some(
                    self.mark(5, Anchor::TopLeft, point(px(0.), px(38.)), cx),
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
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx))),
            )
            .child(titlebar_drag(div().id("toolbar-drag")).flex_1().h_full())
            .child(rise(
                self.ring(
                    3,
                    div()
                        .id("status")
                        .role(Role::Button)
                        .aria_label(t(Key::NoteStatus))
                        .debug_selector(|| "status".into())
                        .w(px(140.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .px(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(self.status_open, |s| s.bg(rgb(pal.active)))
                        .hover(|s| s.bg(rgb(pal.hover)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.status_open = !this.status_open;
                            cx.notify();
                        }))
                        .text_size(px(12.))
                        .text_color(rgb(if self.save == SaveState::Failed {
                            pal.fg
                        } else {
                            pal.faint
                        }))
                        .child(status),
                    &pal,
                )
                .when_some(
                    self.mark(3, Anchor::TopRight, point(px(140.), px(34.)), cx),
                    |s, m| s.child(m),
                ),
                ("status", self.save as usize),
                220,
                0.,
                2.,
            ))
            .child(
                self.ring(
                    4,
                    icon_btn("theme", self.theme_pref.icon(), theme_tip, false, &pal)
                        .on_click(cx.listener(|this, _, window, cx| this.cycle_theme(window, cx))),
                    &pal,
                )
                .when_some(
                    self.mark(4, Anchor::TopRight, point(px(30.), px(38.)), cx),
                    |s, m| s.child(m),
                ),
            )
            .when(has_note, |bar| {
                bar.child(
                    icon_btn(
                        "delete",
                        "icons/delete.svg",
                        tf(Key::DeleteNote, &[]).into(),
                        false,
                        &pal,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.delete_note(window, cx))),
                )
            })
            .when(cfg!(not(target_os = "macos")), |t| {
                t.child(window_controls(window, &pal))
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
                        .child(div().flex_1().min_h_0().w_full().when(has_note, |el| {
                            el.child(rise(
                                div().size_full().child(self.editor.clone()),
                                ("editor-in", self.open_gen),
                                420,
                                0.,
                                10.,
                            ))
                        }))
                        .when_some(self.render_backlinks(cx), |s, m| s.child(m))
                        .when_some(
                            self.mark(2, Anchor::TopLeft, point(px(70.), px(70.)), cx),
                            |s, m| s.child(m),
                        )
                        .when_some(self.render_completion(cx), |s, m| s.child(m))
                        .when_some(self.render_find(cx), |s, m| s.child(m)),
                )
                .into_any_element()
        };

        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .when_some(self.render_tabs(cx), |el, tabs| el.child(tabs))
            .child(body)
            .when_some(self.search.as_ref(), |el, _| {
                el.child(self.render_search(cx))
            })
            .when(self.status_open, |el| {
                el.child(self.render_status(has_note, cx))
            })
            .when_some(self.render_update(cx), |el, card| el.child(card))
            .when_some(self.crash.clone(), |el, c| {
                el.child(self.render_crash(c, cx))
            })
    }

    fn render_crash(
        &self,
        crash: crate::crash::Pending,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pal = cx.palette();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(rgb(pal.body))
                .hover(|s| s.bg(rgb(pal.hover)))
                .child(label)
        };
        let report = crash.text.clone();
        let issue = crate::crash::issue_url(&crash.text);
        let path = crash.path.clone();
        div()
            .id("crash-report")
            .role(Role::Dialog)
            .aria_label(t(Key::CrashTitle))
            .absolute()
            .top(px(52.))
            .right(px(12.))
            .w(px(320.))
            .p(px(14.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(px(8.))
            .shadow_lg()
            .occlude()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(pal.fg))
                    .child(t(Key::CrashTitle)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(rgb(pal.dim))
                    .child(t(Key::CrashBody)),
            )
            .when(crash.recovered, |el| {
                el.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(rgb(pal.dim))
                        .child(t(Key::CrashRecovered)),
                )
            })
            .child(
                div()
                    .mt(px(4.))
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(
                        button("crash-copy", t(Key::CopyReport)).on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(report.clone()))
                        }),
                    )
                    .child(
                        button("crash-issue", t(Key::OpenIssue))
                            .on_click(move |_, _, cx| cx.open_url(&issue)),
                    )
                    .child(
                        button("crash-reveal", t(Key::Reveal))
                            .on_click(move |_, _, cx| cx.reveal_path(&path)),
                    )
                    .child(
                        button("crash-dismiss", t(Key::Dismiss)).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.crash = None;
                                cx.notify();
                            },
                        )),
                    ),
            )
    }

    fn render_status(&self, has_note: bool, cx: &mut Context<Self>) -> impl IntoElement {
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
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(rgb(pal.dim))
                .child(label)
        };
        let action = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(rgb(pal.body))
                .hover(|s| s.bg(rgb(pal.hover)))
                .child(label)
        };
        div()
            .id("status-menu")
            .role(Role::Dialog)
            .aria_label(t(Key::NoteStatus))
            .absolute()
            .top(px(44.))
            .right(px(if has_note { 72. } else { 40. }))
            .w(px(240.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(px(8.))
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
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(rgb(pal.faint))
                    .child(div().size(px(6.)).rounded_full().bg(rgb(dot)))
                    .child(state),
            )
            .child(
                div()
                    .mt(px(4.))
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(pal.fg))
                    .child(name),
            )
            .child(
                div()
                    .truncate()
                    .text_size(px(11.))
                    .text_color(rgb(pal.faint))
                    .child(folder),
            )
            .child(div().h(px(1.)).my(px(8.)).bg(rgb(pal.line)))
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
                el.child(div().h(px(1.)).my(px(8.)).bg(rgb(pal.line)))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                action("status-reveal", t(Key::Reveal))
                                    .on_click(move |_, _, cx| cx.reveal_path(&p)),
                            )
                            .child(action("status-copy-path", t(Key::CopyPath)).on_click(
                                move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        copy.display().to_string(),
                                    ))
                                },
                            )),
                    )
            })
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
