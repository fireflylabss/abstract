use super::*;

impl AbstractApp {
    /// The tab strip above the editor: one pill per open note — title,
    /// unsaved dot, close button on hover — scrolled sideways when it
    /// overflows. `None` while nothing is open.
    pub(crate) fn render_tabs(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if self.tabs.is_empty() || self.chrome_hidden() {
            return None;
        }
        let pal = cx.palette();
        let mut strip = div()
            .id("tab-strip")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap(z(2.))
            .overflow_x_scroll();
        for (ix, tab) in self.tabs.iter().enumerate() {
            let path = tab.path();
            // Pending notes have no meaningful stem; title the buffer.
            let title = if tab.pending {
                title_of(tab.editor.read(cx).text()).to_string()
            } else {
                stem_of(&path)
            };
            let active = self.active == Some(ix);
            let dirty = tab.save != SaveState::Saved;
            let failed = tab.save == SaveState::Failed;
            let mut pill = div()
                .id(("tab", tab.id))
                .group("tab")
                .role(Role::Tab)
                .h(z(26.))
                .max_w(z(200.))
                .pl(z(8.))
                .pr(z(4.))
                .flex_none()
                .flex()
                .items_center()
                .gap(z(5.))
                .rounded(z(6.))
                .cursor_pointer()
                .text_size(z(12.))
                .text_color(rgb(if active { pal.fg } else { pal.dim }))
                .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, window, cx)))
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(ix, window, cx);
                    }),
                )
                .child(div().min_w_0().truncate().child(SharedString::from(title)));
            if dirty {
                pill = pill.child(
                    div()
                        .size(z(6.))
                        .flex_none()
                        .rounded_full()
                        .bg(rgb(if failed { pal.fg } else { pal.dim })),
                );
            }
            pill = pill.child(
                div()
                    .id(("tab-close", tab.id))
                    .role(Role::Button)
                    .aria_label(t(Key::CloseTab))
                    .size(z(18.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(z(4.))
                    .when(!active, |el| {
                        el.invisible().group_hover("tab", |s| s.visible())
                    })
                    .hover(|s| s.bg(rgb(pal.line)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(ix, window, cx);
                    }))
                    .child(icon("icons/close.svg", pal.dim).size(z(11.))),
            );
            pill = if active {
                pill.bg(rgb(pal.active))
            } else {
                pill.hover(|s| s.bg(rgb(pal.hover)))
                    .active(|s| s.bg(rgb(pal.active)))
            };
            strip = strip.child(pill);
        }
        Some(
            div()
                .id("tab-bar")
                .role(Role::TabList)
                .h(z(34.))
                .flex()
                .items_center()
                .gap(z(2.))
                .px(z(6.))
                .border_b_1()
                .border_color(rgb(pal.line))
                .child(strip),
        )
    }
}
