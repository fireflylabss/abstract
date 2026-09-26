use super::*;

impl AbstractApp {
    // ── Tour ──────────────────────────────────────────────────────────────

    pub(crate) fn start_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tour_step.is_some() {
            return;
        }
        self.tour_step = Some(0);
        self.tour_shown = true;
        if !self.sidebar_open {
            self.sidebar_open = true;
            self.sidebar_gen += 1;
        }
        self.tour_focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn tour_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.tour_step {
            Some(step) if step + 1 < tour::STEPS.len() => {
                self.tour_step = Some(step + 1);
                self.tour_focus.focus(window, cx);
            }
            _ => self.finish_tour(window, cx),
        }
        cx.notify();
    }

    pub(crate) fn tour_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(step) = self.tour_step {
            self.tour_step = Some(step.saturating_sub(1));
            self.tour_focus.focus(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn tour_skip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_tour(window, cx);
    }

    pub(crate) fn finish_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tour_step = None;
        self.settings.set_tour_done();
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        self.editor.update(cx, |ed, cx| ed.focus(window, cx));
        cx.notify();
    }

    /// Coach mark + highlight ring for `step`, to hang on an anchor element.
    pub(crate) fn mark(
        &self,
        step: usize,
        anchor: Anchor,
        offset: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        (self.tour_step == Some(step)).then(|| {
            tour::mark(step, anchor, offset, self.tour_focus.clone(), cx).into_any_element()
        })
    }

    pub(crate) fn ring(&self, step: usize, el: Stateful<Div>, pal: &Palette) -> Stateful<Div> {
        el.when(self.tour_step == Some(step), |el| {
            el.shadow(vec![gpui_base::box_shadow(
                px(0.),
                px(0.),
                px(0.),
                px(2.),
                rgb(pal.fg).into(),
            )])
        })
    }
}
