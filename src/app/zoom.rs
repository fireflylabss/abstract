use super::*;

impl AbstractApp {
    pub(crate) fn zoom_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_set(crate::zoom::factor() + crate::zoom::STEP, window, cx);
    }

    pub(crate) fn zoom_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_set(crate::zoom::factor() - crate::zoom::STEP, window, cx);
    }

    pub(crate) fn zoom_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_set(1., window, cx);
    }

    fn zoom_set(&mut self, f: f32, window: &mut Window, cx: &mut Context<Self>) {
        let f = f.clamp(crate::zoom::MIN, crate::zoom::MAX);
        crate::zoom::apply(Some(window), f, cx);
        self.settings.set_zoom(f);
        self.settings.save();
        let pct = (f * 100.).round() as i32;
        self.notice = Some(tf(Key::Zoom, &[("pct", &pct.to_string())]).into());
        cx.notify();
    }
}
