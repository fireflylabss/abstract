//! The user-chosen font families (interface and editor prose) and the
//! editor's typography: app-global snapshots render code copies out of
//! `cx.global()`, like `Palette`. Persisted as `ui_font` / `editor_font`
//! (older files: one shared `font`) plus the numeric keys in `store`.

use gpui_kit::*;

use crate::assets::{MONO, SANS};
use crate::store::{CaretStyle, Settings};

/// Virtual family resolved by the platform text system to the OS UI font
/// (SF on macOS, Segoe UI on Windows, …).
pub(crate) const SYSTEM: &str = ".SystemUIFont";

/// Families offered in Settings → General, in show order. Entries past
/// the bundled pair and `SYSTEM` appear only where the platform reports
/// them, so the same list serves every OS.
pub(crate) const CANDIDATES: &[&str] = &[
    SANS,
    SYSTEM,
    "Helvetica Neue",
    "Segoe UI",
    "Ubuntu",
    "Adwaita Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Arial",
    "Georgia",
    "Times New Roman",
    MONO,
    "Menlo",
    "SF Mono",
    "Consolas",
    "JetBrains Mono",
    "Fira Code",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Courier New",
];

/// Interface typeface (`sans`), editor prose (`prose`) and the code/table
/// one (`mono`) — code always stays on the bundled mono.
#[derive(Clone)]
pub(crate) struct Fonts {
    pub sans: SharedString,
    pub prose: SharedString,
    pub mono: SharedString,
}

impl Global for Fonts {}

/// Editor typography and caret look, before the zoom factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Typo {
    /// Body size in points.
    pub size: f32,
    /// Body line height as a multiple of `size`.
    pub line_height: f32,
    /// Extra points below each paragraph.
    pub paragraph_spacing: f32,
    pub heading_scale: f32,
    pub caret: CaretStyle,
    pub blink: bool,
}

impl Global for Typo {}

impl Typo {
    pub fn from_settings(s: &Settings) -> Self {
        Self {
            size: s.editor_size(),
            line_height: s.line_height(),
            paragraph_spacing: s.paragraph_spacing(),
            heading_scale: s.heading_scale(),
            caret: s.caret_style(),
            blink: s.caret_blink(),
        }
    }
}

/// Canonical spelling for `name`: the bundled pair and `SYSTEM` always
/// resolve; anything else must appear in `available` (case-insensitive),
/// else it falls back to the bundled default instead of rendering tofu.
pub(crate) fn resolve<'a>(name: &str, available: &'a [String]) -> &'a str {
    let name = name.trim();
    for c in [SANS, MONO, SYSTEM] {
        if name.eq_ignore_ascii_case(c) {
            return c;
        }
    }
    available
        .iter()
        .find(|f| f.eq_ignore_ascii_case(name))
        .map_or(SANS, String::as_str)
}

/// The picker list: bundled + `SYSTEM` first, then whichever candidates
/// the platform reports.
pub(crate) fn choices(available: &[String]) -> Vec<&'static str> {
    CANDIDATES
        .iter()
        .copied()
        .filter(|c| {
            *c == SANS || *c == MONO || *c == SYSTEM || {
                available.iter().any(|f| f.eq_ignore_ascii_case(c))
            }
        })
        .collect()
}

/// Resolve the chosen families against installed ones and set `Fonts`
/// and `Typo`.
pub(crate) fn apply(settings: &Settings, cx: &mut App) {
    let available = cx.text_system().all_font_names();
    let fonts = Fonts {
        sans: resolve(settings.ui_font(), &available).into(),
        prose: resolve(settings.editor_font(), &available).into(),
        mono: MONO.into(),
    };
    cx.set_global(fonts);
    cx.set_global(Typo::from_settings(settings));
}

#[cfg(test)]
mod tests {
    // No `super::*`: it would re-import the parent's `gpui_kit::*` glob,
    // whose `test` attribute macro shadows the builtin `#[test]`.
    use super::{MONO, SANS, SYSTEM, choices, resolve};

    #[test]
    fn resolve_falls_back_and_normalizes() {
        let available = vec!["Menlo".to_string(), "Georgia".to_string()];
        assert_eq!(resolve("  menlo ", &available), "Menlo");
        assert_eq!(resolve(".systemuifont", &available), SYSTEM);
        assert_eq!(resolve("noto sans", &available), SANS);
        assert_eq!(resolve("No Such Family", &available), SANS);
        assert_eq!(resolve("", &available), SANS);
    }

    #[test]
    fn choices_keeps_bundled_and_filters_system() {
        let available = vec!["Menlo".to_string()];
        assert_eq!(choices(&available), &[SANS, SYSTEM, MONO, "Menlo"]);
        assert_eq!(choices(&[]), &[SANS, SYSTEM, MONO]);
    }
}
