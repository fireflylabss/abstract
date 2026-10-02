//! Whole-app UI zoom (browser-style `Cmd/Ctrl` `+`/`-`/`0`): a single factor
//! drives the `z()` metric helper, the window rem size (which resolves `rems`
//! lengths and default text size) and the component theme's font sizes (which
//! component widgets read), so chrome and editor text zoom together.

use std::sync::atomic::{AtomicU32, Ordering};

use gpui_kit::component::Theme;
use gpui_kit::*;

/// Clamps and step for the zoom factor: 50%–200% in ~10% increments.
pub(crate) const MIN: f32 = 0.5;
pub(crate) const MAX: f32 = 2.0;
pub(crate) const STEP: f32 = 0.1;
const BASE_REM: f32 = 16.;
const BASE_MONO: f32 = 13.;

static FACTOR: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

/// The current factor: `1.0` is 100%.
pub(crate) fn factor() -> f32 {
    f32::from_bits(FACTOR.load(Ordering::Relaxed))
}

/// `z(v)` is a zoom-scaled `px(v)`: use it for literal sizes that should grow
/// and shrink with the UI zoom. Runtime coordinates stay `px`.
pub(crate) fn z(v: f32) -> Pixels {
    px(v * factor())
}

/// Push `f` into every sizing source: the `z()` factor, the window rem size
/// and the component theme's font sizes (re-synced to the Base layer). Called
/// once at startup from settings and again on every zoom action; idempotent.
pub(crate) fn apply(window: Option<&mut Window>, f: f32, cx: &mut App) {
    let f = f.clamp(MIN, MAX);
    FACTOR.store(f.to_bits(), Ordering::Relaxed);
    if let Some(window) = window {
        window.set_rem_size(px(BASE_REM * f));
    }
    let theme = Theme::global_mut(cx);
    theme.font_size = px(BASE_REM * f);
    theme.mono_font_size = px(BASE_MONO * f);
    Theme::sync_base(cx);
}
