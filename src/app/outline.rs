use super::*;

const OUTLINE_W: f32 = 220.;

impl AbstractApp {
    pub(crate) fn toggle_outline(&mut self, cx: &mut Context<Self>) {
        self.outline_open = !self.outline_open;
        self.outline_gen += 1;
        self.settings.set_outline(self.outline_open);
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    /// Caret at the heading's line start; `autoscroll` brings it into view.
    fn jump_to_heading(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |ed, cx| {
            ed.select_range(offset..offset, cx);
            ed.focus(window, cx);
        });
    }

    /// Right-side heading list of the open note, mirroring the sidebar's
    /// width animation; mounted always so the slide can play both ways.
    pub(crate) fn render_outline(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        let headings = self.editor.read(cx).headings();
        let mut rows = div()
            .id("outline-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (ix, h) in headings.iter().enumerate() {
            let offset = h.offset;
            let title = if h.title.is_empty() {
                t(Key::Untitled).into()
            } else {
                h.title.clone()
            };
            rows = rows.child(
                div()
                    .id(("heading", ix))
                    .role(Role::Button)
                    .aria_label(title.clone())
                    .h(px(28.))
                    .flex_none()
                    .mx(px(8.))
                    .pl(px(6. + (h.level.saturating_sub(1) as f32) * 12.))
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(pal.hover)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.jump_to_heading(offset, window, cx)
                    }))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.))
                            .text_color(rgb(if h.level <= 1 { pal.body } else { pal.dim }))
                            .child(title),
                    ),
            );
        }
        if headings.is_empty() {
            rows = rows.child(
                div()
                    .px(px(18.))
                    .py(px(8.))
                    .text_size(px(12.))
                    .text_color(rgb(pal.faint))
                    .child(t(Key::NoHeadings)),
            );
        }
        let (from, to) = if self.outline_open {
            (0., OUTLINE_W)
        } else {
            (OUTLINE_W, 0.)
        };
        let panel = div()
            .id("outline")
            .flex_none()
            .h_full()
            .overflow_hidden()
            .bg(rgb(pal.panel))
            .border_l_1()
            .border_color(rgb(pal.line))
            .child(
                div()
                    .relative()
                    .w(px(OUTLINE_W))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("outline-head")
                            .h(px(28.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(18.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(pal.faint))
                            .child(t(Key::Outline))
                            .child(headings.len().to_string()),
                    )
                    .child(rows),
            );
        if self.outline_gen == 0 {
            return panel.w(px(to)).into_any_element();
        }
        panel
            .with_animation(
                ("outline-slide", self.outline_gen),
                Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint),
                move |el, d| el.w(px(from + (to - from) * d)),
            )
            .into_any_element()
    }
}
