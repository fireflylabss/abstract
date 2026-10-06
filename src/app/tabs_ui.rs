use super::*;

/// Strip height; collapses to 0 in step with the sidebar slide when
/// focus mode hides chrome.
const TAB_BAR_H: f32 = 34.;

impl AbstractApp {
    /// The tab strip above the editor: one pill per open note — title,
    /// unsaved dot, close button on hover — scrolled sideways when it
    /// overflows. `None` while nothing is open; in focus mode it collapses
    /// its height and fades out over the sidebar slide.
    pub(crate) fn render_tabs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.tabs.is_empty() {
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
        // Real pills plus fade-out ghosts of just-closed tabs, merged in
        // strip order (a ghost occupies its old slot for its fade).
        let mut pills: Vec<(usize, bool, AnyElement)> = Vec::new();
        for g in &self.closing_tabs {
            if g.at.elapsed() < Duration::from_millis(MOTION_OUT_MS) {
                pills.push((g.ix, false, self.ghost_tab(g, pal)));
            }
        }
        for (ix, tab) in self.tabs.iter().enumerate() {
            pills.push((ix, true, self.render_tab(ix, tab, pal, window, cx)));
        }
        pills.sort_by_key(|(ix, real, _)| (*ix, *real));
        for (_, _, pill) in pills {
            strip = strip.child(pill);
        }
        let hidden = self.chrome_hidden();
        let (from, to) = if hidden {
            (TAB_BAR_H, 0.)
        } else {
            (0., TAB_BAR_H)
        };
        let line = pal.line;
        let bar = div()
            .id("tab-bar")
            .role(Role::TabList)
            .h(z(to))
            .flex()
            .items_center()
            .gap(z(2.))
            .px(z(6.))
            .overflow_hidden()
            .bg(glass::bg(pal.bg, Surface::Tabs, cx))
            .border_b_1()
            .border_color(rgb(pal.line))
            .child(strip);
        Some(if self.sidebar_gen == 0 {
            bar.into_any_element()
        } else {
            // Keyed on `sidebar_gen` like the sidebar slide, so the strip
            // collapses/expands in the same 280ms.
            bar.with_animation(
                ("tab-bar", self.sidebar_gen),
                Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint),
                move |el, d| {
                    let h = from + (to - from) * d;
                    let a = (h / TAB_BAR_H * 255.).clamp(0., 255.) as u32;
                    el.h(z(h))
                        .opacity(h / TAB_BAR_H)
                        .border_color(rgba(line << 8 | a))
                },
            )
            .into_any_element()
        })
    }

    /// One pill: title, unsaved dot (fades in/out), close button on hover.
    /// A freshly mounted pill fades in over MOTION_IN_MS.
    fn render_tab(
        &self,
        ix: usize,
        tab: &NoteTab,
        pal: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        let dot = base::motion::Presence::new(("tab-dot", tab.id.to_string()), dirty)
            .transition(
                base::motion::Transition::new(Duration::from_millis(MOTION_HOVER_MS))
                    .ease(ease_out_quint),
            )
            .sample(window, cx);
        if dot.should_render() {
            pill = pill.child(
                div()
                    .size(z(6.))
                    .flex_none()
                    .rounded_full()
                    .opacity(dot.progress)
                    .bg(rgb(if failed { pal.fg } else { pal.dim })),
            );
        }
        let close = div()
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
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.close_tab(ix, window, cx);
            }))
            .child(icon("icons/close.svg", pal.dim).size(z(11.)));
        pill = pill.child(hover_bg(
            close,
            ("tab-close", tab.id),
            None,
            pal.line,
            window,
            cx,
        ));
        let pill = if active {
            pill.bg(rgb(pal.active))
        } else {
            hover_bg(pill, ("tab", tab.id), None, pal.hover, window, cx)
                .active(|s| s.bg(rgb(pal.active)))
        };
        pill.with_animation(
            ("tab-in", tab.id),
            Animation::new(Duration::from_millis(MOTION_IN_MS)).with_easing(ease_out_quint),
            |el, d| el.opacity(d),
        )
        .into_any_element()
    }

    /// Pill a just-closed tab leaves behind: same look, no handlers; it
    /// fades out over MOTION_OUT_MS while still occupying its old slot.
    fn ghost_tab(&self, g: &ClosingTab, pal: Palette) -> AnyElement {
        let mut pill = div()
            .id(("tab-ghost", g.id))
            .h(z(26.))
            .max_w(z(200.))
            .pl(z(8.))
            .pr(z(4.))
            .flex_none()
            .flex()
            .items_center()
            .gap(z(5.))
            .rounded(z(6.))
            .text_size(z(12.))
            .text_color(rgb(pal.dim))
            .child(div().min_w_0().truncate().child(g.title.clone()));
        if g.dirty {
            pill = pill.child(
                div()
                    .size(z(6.))
                    .flex_none()
                    .rounded_full()
                    .bg(rgb(if g.failed { pal.fg } else { pal.dim })),
            );
        }
        pill.with_animation(
            ("tab-out", g.id),
            Animation::new(Duration::from_millis(MOTION_OUT_MS)).with_easing(ease_out_quint),
            |el, d| el.opacity(1. - d),
        )
        .into_any_element()
    }
}
