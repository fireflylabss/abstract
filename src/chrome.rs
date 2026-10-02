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

/// Width reserved for the native macOS traffic lights in the header.
pub(crate) const TRAFFIC_LIGHT_INSET: f32 = 78.;

/// Left padding for header rows: on macOS, reserve space for the traffic
/// lights when they sit over this area.
pub(crate) fn chrome_left_pad(native_controls_left: bool) -> f32 {
    if cfg!(target_os = "macos") && native_controls_left {
        TRAFFIC_LIGHT_INSET
    } else {
        9.
    }
}

/// Minimize / maximize-restore / close. Not shown on macOS, where the native
/// traffic lights already provide them; on other platforms the app requests
/// no compositor titlebar, so these are the only controls.
pub(crate) fn window_controls(window: &Window, pal: &Palette) -> impl IntoElement {
    let caps = window.window_controls();
    let maximized = window.is_maximized();
    let line = pal.line;
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
                        WindowControlArea::Min,
                        false,
                        pal,
                    )
                    .when(!cfg!(windows), |b| {
                        b.on_click(|_, window, _| window.minimize_window())
                    }),
                )
            })
            .when(caps.maximize, |r| {
                let (path, label) = if maximized {
                    ("icons/restore.svg", "Restaurar")
                } else {
                    ("icons/maximize.svg", "Maximizar")
                };
                r.child(
                    win_btn("win-max", path, label, WindowControlArea::Max, false, pal)
                        .when(!cfg!(windows), |b| {
                            b.on_click(|_, window, _| window.zoom_window())
                        }),
                )
            })
            .child(
                win_btn(
                    "win-close",
                    "icons/close.svg",
                    "Fechar",
                    WindowControlArea::Close,
                    true,
                    pal,
                )
                .when(!cfg!(windows), |b| {
                    b.on_click(|_, window, _| window.remove_window())
                })
                .text_color(rgb(fg)),
            )
    })
}

/// A window-chrome button: icon centered in a small square, red hover when
/// `danger` (the close button). On Windows the control area routes the click
/// through the native non-client handler, which also handles restore and
/// Win11 snap layouts; other platforms use the client `on_click`.
pub(crate) fn win_btn(
    id: &'static str,
    path: &'static str,
    label: &'static str,
    area: WindowControlArea,
    danger: bool,
    pal: &Palette,
) -> Stateful<Div> {
    let hover = pal.hover;
    let active = pal.active;
    div()
        .id(id)
        .when(cfg!(windows), |b| b.window_control_area(area))
        .role(Role::Button)
        .aria_label(label)
        .size(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .occlude()
        .hover(move |s| {
            if danger {
                s.bg(rgb(0xd92d20)).text_color(rgb(0xffffff))
            } else {
                s.bg(rgb(hover))
            }
        })
        .active(move |s| {
            if danger {
                s.bg(rgb(0xd92d20)).text_color(rgb(0xffffff))
            } else {
                s.bg(rgb(active))
            }
        })
        .child(icon(path, pal.dim).size(px(14.)))
}

/// Marks `el` as a window-drag region: primary-button drags move the window.
/// The control area resolves the drag on Windows, where `start_window_move`
/// is a no-op; the mouse-down handler covers the other platforms.
///
/// Only wrap empty filler elements: Windows hit-tests the control area over
/// the whole element, so children under it would never receive clicks.
pub(crate) fn titlebar_drag(el: Stateful<Div>) -> Stateful<Div> {
    el.window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
}

/// Drag handler for headers whose children fill them: `start_window_move`
/// on macOS/Linux; a no-op on Windows, where drags need `titlebar_drag`.
pub(crate) fn drag_fallback(el: Stateful<Div>) -> Stateful<Div> {
    el.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
}
