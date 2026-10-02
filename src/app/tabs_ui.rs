use super::*;

impl AbstractApp {
    /// Open-note strip between the toolbar and the editor: back/forward
    /// plus one pill per tab (title, dirty dot, close, middle-click close).
    pub(crate) fn render_tabs(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.tabs.is_empty() {
            return None;
        }
        let pal = cx.palette();
        let mut bar = div()
            .id("tab-bar")
            .role(Role::TabList)
            .h(px(34.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .px(px(6.))
            .border_b_1()
            .border_color(rgb(pal.line));
        for (id, icon_path, tip, forward) in [
            ("nav-back", "icons/arrow-left.svg", t(Key::Back), false),
            (
                "nav-forward",
                "icons/arrow-right.svg",
                t(Key::Forward),
                true,
            ),
        ] {
            let enabled = self.can_nav(forward);
            let mut btn = div()
                .id(id)
                .role(Role::Button)
                .aria_label(tip)
                .size(px(22.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.));
            if enabled {
                btn = btn
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.history_nav(forward, window, cx)
                    }));
            }
            bar = bar.child(
                btn.child(icon(icon_path, if enabled { pal.dim } else { pal.faint }).size(px(13.))),
            );
        }
        let mut strip = div()
            .id("tab-strip")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap(px(2.))
            .overflow_x_scroll();
        for ix in 0..self.tabs.len() {
            strip = strip.child(self.render_tab(ix, &pal, cx));
        }
        Some(bar.child(strip))
    }

    fn render_tab(&self, ix: usize, pal: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = &self.tabs[ix];
        let active = self.active == Some(ix);
        let title = if tab.pending {
            title_of(tab.editor.read(cx).text())
        } else {
            SharedString::from(stem_of(&tab.path()))
        };
        let dirty = tab.save != SaveState::Saved;
        let id = tab.id as usize;
        div()
            .id(("tab", id))
            .group("tab")
            .role(Role::Tab)
            .aria_label(title.clone())
            .h(px(26.))
            .max_w(px(200.))
            .pl(px(8.))
            .pr(px(4.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(active, |t| t.bg(rgb(pal.active)))
            .when(!active, |t| {
                t.hover(|s| s.bg(rgb(pal.hover)))
                    .active(|s| s.bg(rgb(pal.active)))
            })
            .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, true, window, cx)))
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_tab(ix, window, cx);
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(if active { pal.fg } else { pal.dim }))
                    .child(title),
            )
            .when(dirty, |t| {
                t.child(div().size(px(6.)).flex_none().rounded_full().bg(rgb(
                    if tab.save == SaveState::Failed {
                        pal.fg
                    } else {
                        pal.dim
                    },
                )))
            })
            .child(
                div()
                    .id(("tab-close", id))
                    .role(Role::Button)
                    .aria_label(t(Key::CloseTab))
                    .size(px(18.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .when(!active, |b| {
                        b.invisible().group_hover("tab", |s| s.visible())
                    })
                    .hover(|s| s.bg(rgb(pal.line)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(ix, window, cx);
                    }))
                    .child(icon("icons/close.svg", pal.dim).size(px(11.))),
            )
    }
}
