use super::*;

impl AbstractApp {
    pub(crate) fn render_main(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        let has_note = self.current.is_some();
        let status: SharedString = self.notice.clone().unwrap_or_else(|| match self.save {
            SaveState::Pending => "Salvando…".into(),
            SaveState::Failed => "Erro ao salvar".into(),
            SaveState::Saved => format!("{} palavras", self.words).into(),
        });
        let theme_tip =
            SharedString::from(format!("Tema: {} ({MOD}+Shift+L)", self.theme_pref.label()));

        let toolbar = titlebar_drag(div().id("toolbar"))
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .px(px(9.))
            .child(
                self.ring(
                    5,
                    icon_btn(
                        "toggle-sidebar",
                        "icons/sidebar.svg",
                        format!("Barra lateral ({MOD}+\\)").into(),
                        !self.sidebar_open,
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
                    format!("Buscar notas ({MOD}+P)").into(),
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx))),
            )
            .child(div().flex_1())
            .child(rise(
                self.ring(
                    3,
                    div()
                        .id("status")
                        .w(px(140.))
                        .text_right()
                        .px(px(8.))
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
                    icon_btn("theme", self.theme_pref.icon(), theme_tip, false)
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
                        format!("Apagar nota ({MOD}+Shift+Backspace)").into(),
                        false,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.delete_note(window, cx))),
                )
            })
            .child(window_controls(window, &pal));

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
                        .child(div().flex_1().min_h_0().w_full().child(rise(
                            div().size_full().child(self.editor.clone()),
                            ("editor-in", self.open_gen),
                            420,
                            0.,
                            10.,
                        )))
                        .when_some(self.render_backlinks(cx), |s, m| s.child(m))
                        .when_some(
                            self.mark(2, Anchor::TopLeft, point(px(70.), px(70.)), cx),
                            |s, m| s.child(m),
                        )
                        .when_some(self.render_completion(cx), |s, m| s.child(m)),
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
            .child(body)
            .when_some(self.search.as_ref(), |el, _| {
                el.child(self.render_search(cx))
            })
    }
}
