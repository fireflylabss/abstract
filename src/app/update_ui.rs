use std::time::UNIX_EPOCH;

use super::*;
use crate::update::{self, Release};

pub(crate) enum UpdateState {
    Available(Release),
    Installing(Release),
    Failed(Release, String),
}

impl UpdateState {
    fn release(&self) -> &Release {
        match self {
            Self::Available(r) | Self::Installing(r) | Self::Failed(r, _) => r,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl AbstractApp {
    // ── Updates ───────────────────────────────────────────────────────────

    /// At most once per `update::EVERY`, a few seconds after launch so the
    /// check never competes with opening the space.
    pub(crate) fn check_updates(&mut self, cx: &mut Context<Self>) {
        if cfg!(debug_assertions) || !self.settings.updates() {
            return;
        }
        let now = now_secs();
        if now.saturating_sub(self.settings.update_checked()) < update::EVERY.as_secs() {
            return;
        }
        self._update_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            let latest = cx.background_spawn(async { update::latest() }).await;
            this.update(cx, |this, cx| {
                this.settings.set_update_checked(now);
                let settings = this.settings.clone();
                cx.background_spawn(async move { settings.save() }).detach();
                match latest {
                    Ok(r) if update::is_newer(env!("CARGO_PKG_VERSION"), &r.version) => {
                        this.update = Some(UpdateState::Available(r));
                        cx.notify();
                    }
                    Ok(_) => {}
                    Err(err) => eprintln!("abstract: update check failed: {err}"),
                }
            })
            .ok();
        }));
    }

    pub(crate) fn set_updates(&mut self, on: bool, cx: &mut Context<Self>) {
        self.settings.set_updates(on);
        if !on {
            self.update = None;
            self._update_task = None;
        }
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        let Some(UpdateState::Available(release)) = self.update.take() else {
            return;
        };
        let install = self.install.clone();
        self.update = Some(UpdateState::Installing(release.clone()));
        cx.notify();
        self._update_task = Some(cx.spawn(async move |this, cx| {
            let r = release.clone();
            let done = cx
                .background_spawn(async move { update::install(&r, &install) })
                .await;
            this.update(cx, |this, cx| {
                let relaunched = done.and_then(|target| {
                    this.flush_blocking(cx);
                    update::relaunch(&target).map_err(|e| e.to_string())
                });
                match relaunched {
                    Ok(()) => cx.quit(),
                    Err(err) => {
                        this.update = Some(UpdateState::Failed(release, err));
                        cx.notify();
                    }
                }
            })
            .ok();
        }));
    }

    pub(crate) fn render_update(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.update.as_ref()?;
        let pal = cx.palette();
        let release = state.release();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(rgb(pal.body))
                .hover(|s| s.bg(rgb(pal.hover)))
                .child(label)
        };
        let body: Option<SharedString> = match state {
            UpdateState::Available(_) if !self.install.automatic() => {
                Some(t(Key::UpdateManual).into())
            }
            UpdateState::Available(_) => None,
            UpdateState::Installing(_) => Some(t(Key::UpdateInstalling).into()),
            UpdateState::Failed(_, err) => Some(tf(Key::UpdateFailed, &[("err", err)]).into()),
        };
        let page = release.page();
        let idle = !matches!(state, UpdateState::Installing(_));
        let offer = matches!(state, UpdateState::Available(_)) && self.install.automatic();
        let title = tf(Key::UpdateAvailable, &[("v", &release.version)]);
        Some(
            div()
                .id("update-card")
                .role(Role::Dialog)
                .aria_label(SharedString::from(title.clone()))
                .absolute()
                .bottom(px(12.))
                .right(px(12.))
                .w(px(300.))
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .bg(rgb(pal.menu_bg))
                .border_1()
                .border_color(rgb(pal.menu_border))
                .rounded(px(8.))
                .shadow_lg()
                .occlude()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(pal.fg))
                        .child(title),
                )
                .when_some(body, |el, body| {
                    el.child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(rgb(pal.dim))
                            .child(body),
                    )
                })
                .when(idle, |el| {
                    el.child(
                        div()
                            .mt(px(4.))
                            .flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .when(offer, |el| {
                                el.child(button("update-install", t(Key::UpdateRestart)).on_click(
                                    cx.listener(|this, _, _, cx| this.install_update(cx)),
                                ))
                            })
                            .child(
                                button("update-notes", t(Key::ReleaseNotes))
                                    .on_click(move |_, _, cx| cx.open_url(&page)),
                            )
                            .child(button("update-later", t(Key::Later)).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.update = None;
                                    cx.notify();
                                },
                            )))
                            .child(button("update-off", t(Key::UpdatesOff)).on_click(
                                cx.listener(|this, _, _, cx| this.set_updates(false, cx)),
                            )),
                    )
                })
                .into_any_element(),
        )
    }
}
