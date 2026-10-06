//! First-run coach marks: a bubble anchored to each highlighted element.
//! Steps advance with buttons or Enter/→, go back with ←, and skip with Esc.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::assets::rise;
use crate::i18n::{Key, t, tf};
use crate::theme::PaletteAccess;
use crate::zoom::z;

actions!(abstract_tour, [TourNext, TourBack, TourSkip]);

pub fn bind_keys(cx: &mut App) {
    let c = Some("Tour");
    cx.bind_keys([
        KeyBinding::new("enter", TourNext, c),
        KeyBinding::new("right", TourNext, c),
        KeyBinding::new("left", TourBack, c),
        KeyBinding::new("escape", TourSkip, c),
    ]);
}

/// A view that hosts the tour bubble and reacts to its buttons/keys.
pub trait TourHost: 'static + Sized {
    fn tour_next(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn tour_back(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn tour_skip(&mut self, window: &mut Window, cx: &mut Context<Self>);
}

pub struct Step {
    pub title: Key,
    pub body: Key,
}

pub const STEPS: &[Step] = &[
    Step {
        title: Key::Tour1Title,
        body: Key::Tour1Body,
    },
    Step {
        title: Key::Tour2Title,
        body: Key::Tour2Body,
    },
    Step {
        title: Key::Tour3Title,
        body: Key::Tour3Body,
    },
    Step {
        title: Key::Tour4Title,
        body: Key::Tour4Body,
    },
    Step {
        title: Key::Tour5Title,
        body: Key::Tour5Body,
    },
    Step {
        title: Key::Tour6Title,
        body: Key::Tour6Body,
    },
];

fn btn(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(|| id.into())
        .role(Role::Button)
        .aria_label(label)
        .h(z(26.))
        .px(z(8.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(6.))
        .cursor_pointer()
        .text_size(z(12.))
        .child(label)
}

/// The bubble for `step`, styled from the active palette. Rendered inside a
/// `deferred`/`anchored` wrapper by the anchor element.
pub fn bubble<V: TourHost>(
    step: usize,
    focus: FocusHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let p = cx.palette();
    let s = &STEPS[step];
    let n = STEPS.len();
    let last = step + 1 == n;
    div()
        .id("tour-bubble")
        .debug_selector(|| "tour-bubble".into())
        .key_context("Tour")
        .track_focus(&focus)
        .role(Role::Dialog)
        .aria_label(tf(
            Key::TourStepAria,
            &[
                ("step", &(step + 1).to_string()),
                ("n", &n.to_string()),
                ("title", t(s.title)),
            ],
        ))
        .w(z(280.))
        .bg(crate::glass::bg(
            p.menu_bg,
            crate::glass::Surface::Menus,
            cx,
        ))
        .border_1()
        .border_color(rgb(p.menu_border))
        .rounded(z(8.))
        .shadow_lg()
        .occlude()
        .on_action(cx.listener(|this, _: &TourNext, window, cx| this.tour_next(window, cx)))
        .on_action(cx.listener(|this, _: &TourBack, window, cx| this.tour_back(window, cx)))
        .on_action(cx.listener(|this, _: &TourSkip, window, cx| this.tour_skip(window, cx)))
        .p(z(14.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .text_size(z(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(p.fg))
                .child(tf(s.title, &[])),
        )
        .child(
            div()
                .text_size(z(12.5))
                .line_height(z(18.))
                .text_color(rgb(p.dim))
                .child(tf(s.body, &[])),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .pt(z(4.))
                .child(div().text_size(z(11.)).text_color(rgb(p.faint)).child(tf(
                    Key::TourStepOf,
                    &[("step", &(step + 1).to_string()), ("n", &n.to_string())],
                )))
                .child(div().flex_1())
                .child(
                    btn("tour-skip", t(Key::TourSkip))
                        .text_color(rgb(p.dim))
                        .hover(|s| s.bg(rgb(p.hover)))
                        .active(|s| s.bg(rgb(p.active)))
                        .on_click(cx.listener(|this, _, window, cx| this.tour_skip(window, cx))),
                )
                .when(step > 0, |row| {
                    row.child(
                        btn("tour-back", t(Key::TourBack))
                            .text_color(rgb(p.dim))
                            .hover(|s| s.bg(rgb(p.hover)))
                            .active(|s| s.bg(rgb(p.active)))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.tour_back(window, cx)),
                            ),
                    )
                })
                .child(
                    btn(
                        "tour-next",
                        if last {
                            t(Key::TourDone)
                        } else {
                            t(Key::TourNext)
                        },
                    )
                    .bg(rgb(p.fg))
                    .text_color(rgb(p.bg))
                    .font_weight(FontWeight::MEDIUM)
                    .hover(|s| s.opacity(0.85))
                    .active(|s| s.opacity(0.7))
                    .on_click(cx.listener(|this, _, window, cx| this.tour_next(window, cx))),
                ),
        )
}

/// Wrapped + animated bubble ready to hang off an anchor element. `offset`
/// is measured from the anchor element's top-left corner: the zero-size
/// absolute wrapper pins the origin there, whatever the anchor's own
/// alignment (a centered flex parent would otherwise center the bubble on
/// top of the element it points at).
pub fn mark<V: TourHost>(
    step: usize,
    anchor: Anchor,
    offset: Point<Pixels>,
    focus: FocusHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    div().absolute().top_0().left_0().size_0().child(
        deferred(
            anchored()
                .anchor(anchor)
                .offset(offset)
                .snap_to_window_with_margin(z(8.))
                .child(rise(
                    div().child(bubble(step, focus, cx)),
                    ("tour-step", step),
                    220,
                    0.,
                    6.,
                )),
        )
        .with_priority(1),
    )
}

#[cfg(test)]
mod tests {
    // No `gpui_kit::*` glob: it re-exports gpui's `test` attribute, which
    // would shadow the built-in `#[test]` that `#[gpui::test]` expands to.
    use super::{Anchor, TourHost, mark};
    use gpui_kit::{
        Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, Modifiers,
        ParentElement as _, Pixels, Point, Render, Styled as _, TestAppContext, VisualTestContext,
        Window, div, point, px,
    };

    struct Harness {
        step: usize,
        anchor: Anchor,
        offset: Point<Pixels>,
        focus: FocusHandle,
        nexts: usize,
        backs: usize,
        skips: usize,
    }

    impl TourHost for Harness {
        fn tour_next(&mut self, _: &mut Window, _: &mut Context<Self>) {
            self.nexts += 1;
        }
        fn tour_back(&mut self, _: &mut Window, _: &mut Context<Self>) {
            self.backs += 1;
        }
        fn tour_skip(&mut self, _: &mut Window, _: &mut Context<Self>) {
            self.skips += 1;
        }
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // Same shape as `icon_btn`: a centered 30px flex box.
            div().size_full().pl(px(400.)).pt(px(60.)).child(
                div()
                    .id("target")
                    .debug_selector(|| "target".into())
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(16.)))
                    .child(mark(
                        self.step,
                        self.anchor,
                        self.offset,
                        self.focus.clone(),
                        cx,
                    )),
            )
        }
    }

    fn open(
        cx: &mut TestAppContext,
        step: usize,
        anchor: Anchor,
        offset: Point<Pixels>,
    ) -> (Entity<Harness>, &mut VisualTestContext) {
        cx.update(|cx| cx.set_global(crate::theme::LIGHT));
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            step,
            anchor,
            offset,
            focus: cx.focus_handle(),
            nexts: 0,
            backs: 0,
            skips: 0,
        });
        cx.run_until_parked();
        (view, cx)
    }

    #[gpui::test]
    fn bubble_sits_below_its_target(cx: &mut TestAppContext) {
        for (anchor, offset) in [
            (Anchor::TopLeft, point(px(0.), px(38.))),
            (Anchor::TopRight, point(px(30.), px(38.))),
        ] {
            let (_, cx) = open(cx, 1, anchor, offset);
            let target = cx.debug_bounds("target").expect("target painted");
            let bubble = cx.debug_bounds("tour-bubble").expect("bubble painted");
            assert!(
                bubble.top() >= target.bottom(),
                "{anchor:?}: bubble {bubble:?} overlaps target {target:?}"
            );
            let edge = if anchor == Anchor::TopLeft {
                bubble.left() - target.left()
            } else {
                bubble.right() - target.right()
            };
            assert!(
                edge.abs() < px(1.),
                "{anchor:?}: bubble not aligned: {edge:?}"
            );
        }
    }

    #[gpui::test]
    fn bubble_buttons_show_labels_and_click(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, 1, Anchor::TopLeft, point(px(0.), px(38.)));
        for id in ["tour-skip", "tour-back", "tour-next"] {
            let b = cx.debug_bounds(id).expect("button painted");
            // 2×10px padding alone: a button without its label is 20px wide.
            assert!(b.size.width > px(30.), "{id} has no visible label: {b:?}");
        }
        for id in ["tour-next", "tour-back", "tour-skip"] {
            let b = cx.debug_bounds(id).unwrap();
            cx.simulate_click(b.center(), Modifiers::none());
        }
        view.read_with(cx, |h, _| {
            assert_eq!((h.nexts, h.backs, h.skips), (1, 1, 1));
        });
    }
}
