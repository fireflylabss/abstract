use super::*;

const REPO: &str = env!("CARGO_PKG_REPOSITORY");
const VERSION: &str = env!("CARGO_PKG_VERSION");

impl AbstractApp {
    pub(crate) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.spaces_open = false;
            self.status_open = false;
            self.settings_focus.focus(window, cx);
            let folder = SharedString::from(self.settings.daily_folder().to_string());
            self.daily_input
                .update(cx, |s, cx| s.set_value(folder, window, cx));
        } else {
            self.editor.update(cx, |ed, cx| ed.focus(window, cx));
        }
        cx.notify();
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.toggle_settings(window, cx);
        }
    }

    /// Input change: commit a valid folder name (invalid text just renders
    /// red until it parses again).
    pub(crate) fn daily_folder_changed(&mut self, cx: &mut Context<Self>) {
        let name = self.daily_input.read(cx).value().trim().to_string();
        if vault::valid_folder_name(&name) && name != self.settings.daily_folder() {
            self.settings.set_daily_folder(&name);
            let settings = self.settings.clone();
            cx.background_spawn(async move { settings.save() }).detach();
        }
        cx.notify();
    }

    fn set_raw_tables(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_raw_tables(on);
        self.editor.update(cx, |ed, cx| ed.set_raw_tables(on, cx));
        self.save_settings(window, cx);
    }

    fn pick_palette(
        &mut self,
        dark: bool,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if dark {
            self.settings.set_dark_theme(id);
        } else {
            self.settings.set_light_theme(id);
        }
        self.save_settings(window, cx);
    }

    /// Sidebar footer: opens the settings dialog.
    pub(crate) fn render_settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        let tip = SharedString::from(tf(Key::SettingsTip, &[]));
        div()
            .flex_none()
            .p(px(8.))
            .border_t_1()
            .border_color(rgb(pal.line))
            .child(
                div()
                    .id("settings-open")
                    .role(Role::Button)
                    .aria_label(tip.clone())
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
                    .h(px(30.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(rgb(pal.body))
                    .when(self.settings_open, |s| s.bg(rgb(pal.active)))
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_settings(window, cx)))
                    .child(icon("icons/settings.svg", pal.dim).size(px(15.)))
                    .child(t(Key::Settings)),
            )
    }

    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        let section = |label: &'static str| {
            div()
                .pt(px(14.))
                .pb(px(6.))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(pal.faint))
                .child(label)
        };
        let sub = |label: &'static str| {
            div()
                .pt(px(10.))
                .pb(px(6.))
                .text_size(px(12.))
                .text_color(rgb(pal.dim))
                .child(label)
        };

        let modes =
            div().flex().gap(px(4.)).children(
                [ThemePref::System, ThemePref::Light, ThemePref::Dark].map(|pref| {
                    let on = self.theme_pref == pref;
                    div()
                        .id(SharedString::from(format!("theme-mode-{}", pref.as_str())))
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .when(on, |s| s.bg(rgb(pal.active)))
                        .hover(|s| s.bg(rgb(pal.hover)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_theme_pref(pref, window, cx)
                        }))
                        .child(icon(pref.icon(), if on { pal.fg } else { pal.dim }).size(px(14.)))
                        .child(t(match pref {
                            ThemePref::System => Key::ThemeSystem,
                            ThemePref::Light => Key::ThemeLight,
                            ThemePref::Dark => Key::ThemeDark,
                        }))
                }),
            );

        let swatches = |dark: bool, cx: &mut Context<Self>| {
            let (list, current) = if dark {
                (&theme::DARKS, self.settings.dark_theme())
            } else {
                (&theme::LIGHTS, self.settings.light_theme())
            };
            let current = theme::named(list, current).id;
            div().flex().gap(px(8.)).children(list.iter().map(|n| {
                let on = n.id == current;
                let p = n.palette;
                let id = n.id;
                div()
                    .id(SharedString::from(format!(
                        "{}-theme-{id}",
                        if dark { "dark" } else { "light" }
                    )))
                    .role(Role::RadioButton)
                    .aria_label(n.name)
                    .aria_selected(on)
                    .flex_1()
                    .min_w_0()
                    .p(px(4.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(if on { pal.fg } else { pal.line }))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pick_palette(dark, id, window, cx)
                    }))
                    .child(
                        div()
                            .h(px(46.))
                            .p(px(7.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .rounded(px(5.))
                            .bg(rgb(p.bg))
                            .border_1()
                            .border_color(rgb(p.line))
                            .child(div().w(px(34.)).h(px(5.)).rounded(px(2.)).bg(rgb(p.head)))
                            .child(div().w_full().h(px(3.)).rounded(px(2.)).bg(rgb(p.dim)))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(3.))
                                    .child(
                                        div().w(px(18.)).h(px(3.)).rounded(px(2.)).bg(rgb(p.dim)),
                                    )
                                    .child(
                                        div()
                                            .w(px(12.))
                                            .h(px(3.))
                                            .rounded(px(2.))
                                            .bg(rgb(p.callout[0])),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .px(px(2.))
                            .pb(px(2.))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(rgb(if on { pal.fg } else { pal.body }))
                            .child(n.name),
                    )
            }))
        };

        let toggle =
            |id: &'static str, label: &'static str, hint: Option<&'static str>, on: bool| {
                div()
                    .id(id)
                    .role(Role::Switch)
                    .aria_label(label)
                    .aria_toggled(on.into())
                    .px(px(8.))
                    .py(px(7.))
                    .mx(px(-8.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(pal.body))
                                    .child(label),
                            )
                            .when_some(hint, |el, h| {
                                el.child(
                                    div()
                                        .text_size(px(12.))
                                        .line_height(px(17.))
                                        .text_color(rgb(pal.dim))
                                        .child(h),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(30.))
                            .h(px(18.))
                            .p(px(2.))
                            .flex()
                            .items_center()
                            .when(on, |s| s.justify_end())
                            .rounded_full()
                            .bg(rgb(if on { pal.fg } else { pal.active }))
                            .child(div().size(px(14.)).rounded_full().bg(rgb(if on {
                                pal.bg
                            } else {
                                pal.dim
                            }))),
                    )
            };

        let link = |id: &'static str, glyph: &'static str, label: &'static str, url: String| {
            div()
                .id(id)
                .role(Role::Link)
                .aria_label(label)
                .h(px(28.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(pal.line))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(rgb(pal.body))
                .hover(|s| s.bg(rgb(pal.hover)))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .child(icon(glyph, pal.dim).size(px(13.)))
                .child(label)
        };

        let raw = self.settings.raw_tables();
        let updates = self.settings.updates();
        let daily_ok = vault::valid_folder_name(self.daily_input.read(cx).value().trim());
        let body = div()
            .id("settings-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(20.))
            .pb(px(20.))
            .child(section(t(Key::Appearance)))
            .child(modes)
            .child(sub(t(Key::LightTheme)))
            .child(swatches(false, cx))
            .child(sub(t(Key::DarkTheme)))
            .child(swatches(true, cx))
            .child(section(t(Key::EditorSection)))
            .child(
                toggle(
                    "raw-tables",
                    t(Key::RawTables),
                    Some(t(Key::RawTablesHint)),
                    raw,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_raw_tables(!raw, window, cx)),
                ),
            )
            .child(section(t(Key::General)))
            .child(
                div()
                    .id("lang-cycle")
                    .role(Role::Button)
                    .aria_label(t(Key::Language))
                    .px(px(8.))
                    .py(px(7.))
                    .mx(px(-8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(rgb(pal.body))
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_lang(cx)))
                    .child(div().flex_1().child(t(Key::Language)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .text_color(rgb(pal.dim))
                            .child(self.lang_label())
                            .child(icon("icons/chevrons.svg", pal.faint).size(px(13.))),
                    ),
            )
            .child(
                div()
                    .id("daily-folder")
                    .px(px(8.))
                    .py(px(7.))
                    .mx(px(-8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .text_size(px(13.))
                    .text_color(rgb(pal.body))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(div().child(t(Key::DailyFolder)))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(pal.dim))
                                    .truncate()
                                    .child(t(Key::DailyFolderHint)),
                            ),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .flex_none()
                            .h(px(26.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(if daily_ok { pal.line } else { pal.callout[3] }))
                            .child(
                                Input::new(&self.daily_input)
                                    .appearance(false)
                                    .bordered(false)
                                    .w_full()
                                    .text_size(px(12.))
                                    .text_color(rgb(pal.fg)),
                            ),
                    ),
            )
            .child(
                toggle("updates-toggle", t(Key::CheckUpdates), None, updates)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_updates(!updates, cx))),
            )
            .child(section(t(Key::About)))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(pal.fg))
                            .child("Abstract"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(pal.dim))
                            .child(tf(Key::Version, &[("v", VERSION)])),
                    ),
            )
            .child(
                div()
                    .pt(px(4.))
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(rgb(pal.dim))
                    .child(t(Key::AboutTagline)),
            )
            .child(
                div()
                    .pt(px(12.))
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(link(
                        "about-source",
                        "icons/github.svg",
                        t(Key::SourceCode),
                        REPO.to_string(),
                    ))
                    .child(link(
                        "about-release",
                        "icons/external-link.svg",
                        t(Key::ReleaseNotes),
                        format!("{REPO}/releases/tag/v{VERSION}"),
                    ))
                    .child(link(
                        "about-issue",
                        "icons/external-link.svg",
                        t(Key::ReportIssue),
                        format!("{REPO}/issues/new"),
                    ))
                    .child(link(
                        "about-license",
                        "icons/external-link.svg",
                        t(Key::License),
                        format!("{REPO}/blob/main/LICENSE"),
                    )),
            );

        let card = div()
            .id("settings")
            .role(Role::Dialog)
            .aria_label(t(Key::Settings))
            .track_focus(&self.settings_focus)
            .w(px(480.))
            .max_h(relative(0.86))
            .flex()
            .flex_col()
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(px(10.))
            .shadow_lg()
            .occlude()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.close_settings(window, cx);
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_settings(window, cx)))
            .child(
                div()
                    .flex_none()
                    .h(px(48.))
                    .pl(px(20.))
                    .pr(px(10.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(pal.line))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(pal.fg))
                            .child(t(Key::Settings)),
                    )
                    .child(
                        icon_btn(
                            "settings-close",
                            "icons/close.svg",
                            t(Key::Close).into(),
                            false,
                            &pal,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.close_settings(window, cx)),
                        ),
                    ),
            )
            .child(body);

        div()
            .id("settings-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x0000_0059))
            .occlude()
            .child(card)
            .with_animation(
                "settings-in",
                Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint),
                |el, d| el.opacity(d),
            )
    }
}
