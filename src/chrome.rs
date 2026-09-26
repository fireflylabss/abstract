use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::assets::icon;
use crate::store::SessionWindow;
use crate::theme::Palette;

pub(crate) fn session_window(window: &Window) -> SessionWindow {
    let wb = window.window_bounds();
    let b = wb.get_bounds();
    SessionWindow {
        maximized: matches!(wb, WindowBounds::Maximized(_) | WindowBounds::Fullscreen(_)),
        x: b.origin.x.into(),
        y: b.origin.y.into(),
        w: b.size.width.into(),
        h: b.size.height.into(),
    }
}

/// Minimize / maximize-restore / close. Shown in both decoration modes: the
/// app requests no compositor titlebar, so these are the only controls.
pub(crate) fn window_controls(window: &Window, pal: &Palette) -> impl IntoElement {
    let caps = window.window_controls();
    let maximized = window.is_maximized();
    let line = pal.line;
    let hover = pal.hover;
    let dim = pal.dim;
    let fg = pal.fg;
    div().map(|row| {
        row.flex()
            .items_center()
            .gap(px(2.))
            .ml(px(6.))
            .pl(px(8.))
            .border_l_1()
            .border_color(rgb(line))
            .when(caps.minimize, |r| {
                r.child(
                    win_btn(
                        "win-min",
                        "icons/minimize.svg",
                        "Minimizar",
                        false,
                        dim,
                        hover,
                    )
                    .on_click(|_, window, _| window.minimize_window()),
                )
            })
            .when(caps.maximize, |r| {
                let (path, label) = if maximized {
                    ("icons/restore.svg", "Restaurar")
                } else {
                    ("icons/maximize.svg", "Maximizar")
                };
                r.child(
                    win_btn("win-max", path, label, false, dim, hover)
                        .on_click(|_, window, _| window.zoom_window()),
                )
            })
            .child(
                win_btn("win-close", "icons/close.svg", "Fechar", true, dim, hover)
                    .on_click(|_, window, _| window.remove_window())
                    .text_color(rgb(fg)),
            )
    })
}

/// A window-chrome button: icon centered in a small square, red hover when
/// `danger` (the close button).
pub(crate) fn win_btn(
    id: &'static str,
    path: &'static str,
    label: &'static str,
    danger: bool,
    dim: u32,
    hover: u32,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .size(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .hover(move |s| {
            if danger {
                s.bg(rgb(0xd92d20)).text_color(rgb(0xffffff))
            } else {
                s.bg(rgb(hover))
            }
        })
        .child(icon(path, dim).size(px(14.)))
}

/// Marks `el` as a window-drag region: primary-button drags move the window.
pub(crate) fn titlebar_drag(el: Stateful<Div>) -> Stateful<Div> {
    el.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
}
