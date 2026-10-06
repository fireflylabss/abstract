use super::*;

impl AbstractApp {
    // ── Focus mode ────────────────────────────────────────────────────────

    /// Chrome hidden while focus mode is on (sidebar, status chip). Kept in
    /// one place so another strip (tabs, #59) joins here at merge time.
    pub(crate) fn chrome_hidden(&self) -> bool {
        self.focus_mode
    }

    /// The saved sidebar preference, masked while focus hides chrome.
    pub(crate) fn sidebar_visible(&self) -> bool {
        self.sidebar_open && !self.chrome_hidden()
    }

    pub(crate) fn toggle_focus(&mut self, cx: &mut Context<Self>) {
        self.set_focus(!self.focus_mode, cx);
    }

    fn set_focus(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.focus_mode == on {
            return;
        }
        self.focus_mode = on;
        // Replay the sidebar slide for the visibility change.
        self.sidebar_gen += 1;
        self.editor.update(cx, |ed, cx| ed.set_focus_mode(on, cx));
        cx.notify();
    }

    /// Esc reached the app unconsumed — leave focus mode if it is on.
    pub(crate) fn exit_focus(&mut self, cx: &mut Context<Self>) {
        self.set_focus(false, cx);
    }
}
