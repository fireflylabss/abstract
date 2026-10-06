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

    fn set_raw_tables(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_raw_tables(on);
        self.editor.update(cx, |ed, cx| ed.set_raw_tables(on, cx));
        self.save_settings(window, cx);
    }

    fn set_discord(&mut self, on: bool, cx: &mut Context<Self>) {
        self.settings.set_discord(on);
        self.presence.set_enabled(on);
        if on {
            let title = self
                .current
                .as_ref()
                .map(|_| title_of(self.editor.read(cx).text()).to_string());
            self.presence.set(title, spaces::name_of(&self.dir));
        }
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
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
    pub(crate) fn render_settings_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pal = cx.palette();
        let tip = SharedString::from(tf(Key::SettingsTip, &[]));
        div()
            .flex_none()
            .p(z(8.))
            .border_t_1()
            .border_color(rgb(pal.line))
            .child(hover_bg(
                div()
                    .id("settings-open")
                    .role(Role::Button)
                    .aria_label(tip.clone())
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
                    .h(z(30.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(13.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_settings(window, cx)))
                    .child(icon("icons/settings.svg", pal.dim).size(z(15.)))
                    .child(t(Key::Settings)),
                "settings-open",
                self.settings_open.then_some(pal.active),
                pal.hover,
                window,
                cx,
            ))
    }

    pub(crate) fn render_settings(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pal = cx.palette();
        let section = |label: &'static str| {
            div()
                .pt(z(14.))
                .pb(z(6.))
                .text_size(z(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(pal.faint))
                .child(label)
        };
        let sub = |label: &'static str| {
            div()
                .pt(z(10.))
                .pb(z(6.))
                .text_size(z(12.))
                .text_color(rgb(pal.dim))
                .child(label)
        };

        let modes = div().flex().gap(z(4.)).children(
            [ThemePref::System, ThemePref::Light, ThemePref::Dark].map(|pref| {
                let on = self.theme_pref == pref;
                hover_bg(
                    div()
                        .id(SharedString::from(format!("theme-mode-{}", pref.as_str())))
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(z(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(z(6.))
                        .rounded(z(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(z(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_theme_pref(pref, window, cx)
                        }))
                        .child(icon(pref.icon(), if on { pal.fg } else { pal.dim }).size(z(14.)))
                        .child(t(match pref {
                            ThemePref::System => Key::ThemeSystem,
                            ThemePref::Light => Key::ThemeLight,
                            ThemePref::Dark => Key::ThemeDark,
                        })),
                    SharedString::from(format!("theme-mode-{}", pref.as_str())),
                    on.then_some(pal.active),
                    pal.hover,
                    window,
                    cx,
                )
            }),
        );

        let swatches = |dark: bool, window: &mut Window, cx: &mut Context<Self>| {
            let (list, current) = if dark {
                (&theme::DARKS, self.settings.dark_theme())
            } else {
                (&theme::LIGHTS, self.settings.light_theme())
            };
            let current = theme::named(list, current).id;
            div().flex().gap(z(8.)).children(list.iter().map(|n| {
                let on = n.id == current;
                let p = n.palette;
                let id = n.id;
                hover_bg(
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
                        .p(z(4.))
                        .flex()
                        .flex_col()
                        .gap(z(6.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pick_palette(dark, id, window, cx)
                        }))
                        .child(
                            div()
                                .h(z(46.))
                                .p(z(7.))
                                .flex()
                                .flex_col()
                                .gap(z(4.))
                                .rounded(z(4.))
                                .bg(rgb(p.bg))
                                .border_1()
                                .border_color(rgb(p.line))
                                .child(div().w(z(34.)).h(z(5.)).rounded(z(2.)).bg(rgb(p.head)))
                                .child(div().w_full().h(z(3.)).rounded(z(2.)).bg(rgb(p.dim)))
                                .child(
                                    div()
                                        .flex()
                                        .gap(z(3.))
                                        .child(
                                            div().w(z(18.)).h(z(3.)).rounded(z(2.)).bg(rgb(p.dim)),
                                        )
                                        .child(
                                            div()
                                                .w(z(12.))
                                                .h(z(3.))
                                                .rounded(z(2.))
                                                .bg(rgb(p.callout[0])),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .px(z(2.))
                                .pb(z(2.))
                                .truncate()
                                .text_size(z(12.))
                                .text_color(rgb(if on { pal.fg } else { pal.body }))
                                .child(n.name),
                        ),
                    SharedString::from(format!(
                        "{}-theme-{id}",
                        if dark { "dark" } else { "light" }
                    )),
                    None,
                    pal.hover,
                    window,
                    cx,
                )
            }))
        };

        let toggle = |id: &'static str,
                      label: &'static str,
                      hint: Option<&'static str>,
                      on: bool,
                      window: &mut Window,
                      cx: &mut App| {
            hover_bg(
                div()
                    .id(id)
                    .role(Role::Switch)
                    .aria_label(label)
                    .aria_toggled(on.into())
                    .px(z(8.))
                    .py(z(7.))
                    .mx(z(-8.))
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .active(|s| s.bg(rgb(pal.active)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(z(2.))
                            .child(
                                div()
                                    .text_size(z(13.))
                                    .text_color(rgb(pal.body))
                                    .child(label),
                            )
                            .when_some(hint, |el, h| {
                                el.child(
                                    div()
                                        .text_size(z(12.))
                                        .line_height(z(17.))
                                        .text_color(rgb(pal.dim))
                                        .child(h),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(z(30.))
                            .h(z(18.))
                            .p(z(2.))
                            .flex()
                            .items_center()
                            .when(on, |s| s.justify_end())
                            .rounded_full()
                            .bg(rgb(if on { pal.fg } else { pal.active }))
                            .child(div().size(z(14.)).rounded_full().bg(rgb(if on {
                                pal.bg
                            } else {
                                pal.dim
                            }))),
                    ),
                id,
                None,
                pal.hover,
                window,
                cx,
            )
        };

        let link = |id: &'static str,
                    glyph: &'static str,
                    label: &'static str,
                    url: String,
                    window: &mut Window,
                    cx: &mut App| {
            hover_bg(
                div()
                    .id(id)
                    .role(Role::Link)
                    .aria_label(label)
                    .h(z(28.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .gap(z(6.))
                    .rounded(z(6.))
                    .border_1()
                    .border_color(rgb(pal.line))
                    .cursor_pointer()
                    .text_size(z(12.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .child(icon(glyph, pal.dim).size(z(13.)))
                    .child(label),
                id,
                None,
                pal.hover,
                window,
                cx,
            )
        };

        let raw = self.settings.raw_tables();
        let updates = self.settings.updates();
        let font_names = cx.text_system().all_font_names();
        let current_font = fonts::resolve(self.settings.font(), &font_names);
        let font_rows =
            div()
                .flex()
                .flex_col()
                .children(fonts::choices(&font_names).into_iter().map(|family| {
                    let on = current_font.eq_ignore_ascii_case(family);
                    let label: SharedString = if family == fonts::SYSTEM {
                        t(Key::ThemeSystem).into()
                    } else {
                        family.into()
                    };
                    hover_bg(
                        div()
                            .id(SharedString::from(format!("font-{family}")))
                            .role(Role::RadioButton)
                            .aria_label(label.clone())
                            .aria_selected(on)
                            .px(z(8.))
                            .py(z(6.))
                            .mx(z(-8.))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .rounded(z(6.))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_font(family, window, cx)
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(family)
                                    .text_size(z(13.))
                                    .text_color(rgb(if on { pal.fg } else { pal.body }))
                                    .child(label),
                            )
                            .when(on, |s| {
                                s.child(icon("icons/check.svg", pal.dim).size(z(14.)))
                            }),
                        SharedString::from(format!("font-{family}")),
                        None,
                        pal.hover,
                        window,
                        cx,
                    )
                }));
        let widths = div()
            .flex()
            .gap(z(4.))
            .children(TextWidth::ALL.map(|width| {
                let on = self.settings.text_width() == width;
                hover_bg(
                    div()
                        .id(SharedString::from(format!("text-width-{}", width.as_str())))
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(z(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(z(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_text_width(width, window, cx)
                        }))
                        .child(t(match width {
                            TextWidth::Narrow => Key::TextWidthNarrow,
                            TextWidth::Medium => Key::TextWidthMedium,
                            TextWidth::Wide => Key::TextWidthWide,
                        })),
                    SharedString::from(format!("text-width-{}", width.as_str())),
                    on.then_some(pal.active),
                    pal.hover,
                    window,
                    cx,
                )
            }));
        let discord = self.settings.discord();
        let body = div()
            .id("settings-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(z(20.))
            .pb(z(20.))
            .child(section(t(Key::Appearance)))
            .child(modes)
            .child(sub(t(Key::LightTheme)))
            .child(swatches(false, window, cx))
            .child(sub(t(Key::DarkTheme)))
            .child(swatches(true, window, cx))
            .child(section(t(Key::EditorSection)))
            .child(
                toggle(
                    "raw-tables",
                    t(Key::RawTables),
                    Some(t(Key::RawTablesHint)),
                    raw,
                    window,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_raw_tables(!raw, window, cx)),
                ),
            )
            .child(sub(t(Key::TextWidth)))
            .child(widths)
            .child(section(t(Key::General)))
            .child(hover_bg(
                div()
                    .id("lang-cycle")
                    .role(Role::Button)
                    .aria_label(t(Key::Language))
                    .px(z(8.))
                    .py(z(7.))
                    .mx(z(-8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(13.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_lang(cx)))
                    .child(div().flex_1().child(t(Key::Language)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .text_size(z(12.))
                            .text_color(rgb(pal.dim))
                            .child(self.lang_label())
                            .child(icon("icons/chevrons.svg", pal.faint).size(z(13.))),
                    ),
                "lang-cycle",
                None,
                pal.hover,
                window,
                cx,
            ))
            .child(
                toggle(
                    "updates-toggle",
                    t(Key::CheckUpdates),
                    None,
                    updates,
                    window,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_updates(!updates, cx))),
            )
            .child(sub(t(Key::Font)))
            .child(
                div()
                    .pb(z(4.))
                    .text_size(z(12.))
                    .line_height(z(17.))
                    .text_color(rgb(pal.dim))
                    .child(t(Key::FontHint)),
            )
            .child(font_rows)
            .child(
                toggle(
                    "discord-presence",
                    t(Key::DiscordPresence),
                    Some(t(Key::DiscordPresenceHint)),
                    discord,
                    window,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_discord(!discord, cx))),
            )
            .child(section(t(Key::About)))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(z(8.))
                    .child(
                        div()
                            .text_size(z(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(pal.fg))
                            .child("Abstract"),
                    )
                    .child(
                        div()
                            .text_size(z(12.))
                            .text_color(rgb(pal.dim))
                            .child(tf(Key::Version, &[("v", VERSION)])),
                    ),
            )
            .child(
                div()
                    .pt(z(4.))
                    .text_size(z(12.))
                    .line_height(z(18.))
                    .text_color(rgb(pal.dim))
                    .child(t(Key::AboutTagline)),
            )
            .child(
                div()
                    .pt(z(12.))
                    .flex()
                    .flex_wrap()
                    .gap(z(6.))
                    .child(link(
                        "about-source",
                        "icons/github.svg",
                        t(Key::SourceCode),
                        REPO.to_string(),
                        window,
                        cx,
                    ))
                    .child(link(
                        "about-release",
                        "icons/external-link.svg",
                        t(Key::ReleaseNotes),
                        format!("{REPO}/releases/tag/v{VERSION}"),
                        window,
                        cx,
                    ))
                    .child(link(
                        "about-issue",
                        "icons/external-link.svg",
                        t(Key::ReportIssue),
                        format!("{REPO}/issues/new"),
                        window,
                        cx,
                    ))
                    .child(link(
                        "about-license",
                        "icons/external-link.svg",
                        t(Key::License),
                        format!("{REPO}/blob/main/LICENSE"),
                        window,
                        cx,
                    )),
            );

        let card = div()
            .id("settings")
            .role(Role::Dialog)
            .aria_label(t(Key::Settings))
            .track_focus(&self.settings_focus)
            .w(z(480.))
            .max_h(relative(0.86))
            .flex()
            .flex_col()
            .bg(rgb(pal.menu_bg))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(z(8.))
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
                    .h(z(48.))
                    .pl(z(20.))
                    .pr(z(10.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(pal.line))
                    .child(
                        div()
                            .flex_1()
                            .text_size(z(14.))
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
                            window,
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.close_settings(window, cx)),
                        ),
                    ),
            )
            .child(body);

        let ps = presence("settings", self.settings_open, window, cx);
        if !ps.should_render() {
            return div().into_any_element();
        }
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
            .opacity(ps.progress)
            .occlude()
            .child(card)
            .into_any_element()
    }
    /// `font` picker in General: applies the family live, then persists it.
    fn set_font(&mut self, family: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_font(family);
        fonts::apply(&self.settings, cx);
        self.save_settings(window, cx);
    }

    /// `text_width` picker in Editor: applies the column cap live, then
    /// persists it.
    fn set_text_width(&mut self, width: TextWidth, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_text_width(width);
        self.apply_text_width(&self.editor.clone(), cx);
        self.save_settings(window, cx);
    }

    /// Pushes the `text_width` column cap onto one `LiveEditor` — called at
    /// startup and whenever the setting changes; each new editor needs it.
    pub(crate) fn apply_text_width(&self, editor: &Entity<LiveEditor>, cx: &mut App) {
        let col = self.settings.text_width().max_col();
        editor.update(cx, |ed, cx| ed.set_max_col(col, cx));
    }
}
