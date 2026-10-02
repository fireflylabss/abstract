use std::borrow::Cow;
use std::time::Duration;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::theme::Palette;
use crate::zoom::z;
// ── Palette: pure monochrome, driven by `Palette` global ──────────────────
pub(crate) const SANS: &str = "Noto Sans";
pub(crate) const MONO: &str = "Noto Sans Mono";

/// Bundled Noto fonts (OFL), registered at startup so Windows/Linux don't
/// depend on system fonts.
pub(crate) const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Medium.ttf"),
    include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Italic.ttf"),
    include_bytes!("../assets/fonts/NotoSans-SemiBoldItalic.ttf"),
    include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf"),
];
/// Embedded Hugeicons (stroke-rounded, MIT). Anything else falls through to
/// the component library's default icon set.
const ICONS: [(&str, &[u8]); 17] = [
    (
        "icons/add.svg",
        include_bytes!("../assets/icons/add-01.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/tick-02.svg"),
    ),
    (
        "icons/chevrons.svg",
        include_bytes!("../assets/icons/unfold-more.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/icons/cancel-01.svg"),
    ),
    (
        "icons/delete.svg",
        include_bytes!("../assets/icons/delete-02.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder-01.svg"),
    ),
    (
        "icons/folder-add.svg",
        include_bytes!("../assets/icons/folder-add.svg"),
    ),
    (
        "icons/maximize.svg",
        include_bytes!("../assets/icons/square.svg"),
    ),
    (
        "icons/minimize.svg",
        include_bytes!("../assets/icons/minus-sign.svg"),
    ),
    (
        "icons/monitor.svg",
        include_bytes!("../assets/icons/monitor.svg"),
    ),
    (
        "icons/note.svg",
        include_bytes!("../assets/icons/note-01.svg"),
    ),
    (
        "icons/pencil.svg",
        include_bytes!("../assets/icons/pencil.svg"),
    ),
    (
        "icons/globe.svg",
        include_bytes!("../assets/icons/globe.svg"),
    ),
    ("icons/link.svg", include_bytes!("../assets/icons/link.svg")),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/restore.svg",
        include_bytes!("../assets/icons/square-arrow-shrink-02.svg"),
    ),
    (
        "icons/sidebar.svg",
        include_bytes!("../assets/icons/view-sidebar-left.svg"),
    ),
];
pub(crate) struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match ICONS.iter().find(|(p, _)| *p == path) {
            Some((_, bytes)) => Ok(Some(Cow::Borrowed(bytes))),
            None => gpui_kit_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = gpui_kit_assets::Assets.list(path)?;
        out.extend(
            ICONS
                .iter()
                .filter(|(p, _)| p.starts_with(path))
                .map(|(p, _)| (*p).into()),
        );
        Ok(out)
    }
}

pub(crate) fn icon(name: &'static str, color: u32) -> Svg {
    svg()
        .path(name)
        .size(z(16.))
        .flex_none()
        .text_color(rgb(color))
}

/// Quart-out fade + short travel. Keyed by `id`; a new id replays it.
/// `AnimationExt` honors the platform reduced-motion setting.
pub(crate) fn rise<E: Styled + IntoElement + 'static>(
    el: E,
    id: impl Into<ElementId>,
    ms: u64,
    delay: f32,
    travel: f32,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(ms)).with_easing(move |t| {
            let t = ((t - delay) / (1.0 - delay)).clamp(0.0, 1.0);
            1.0 - (1.0 - t).powi(4)
        }),
        move |el, d| el.opacity(d).mt(z(travel * (1.0 - d))),
    )
}
pub(crate) fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

pub(crate) fn icon_btn(
    id: &'static str,
    path: &'static str,
    label: SharedString,
    on: bool,
    pal: &Palette,
) -> Stateful<Div> {
    let tip = label.clone();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
        })
        .size(z(30.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(6.))
        .cursor_pointer()
        .occlude()
        .hover(move |s| s.bg(rgb(pal.hover)))
        .when(on, move |s| s.bg(rgb(pal.active)))
        .child(icon(path, pal.fg))
}
