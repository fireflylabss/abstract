//! The user-chosen font family for UI and prose text: an app-global
//! snapshot render code copies out of `cx.global()`, like `Palette`.
//! Persisted as `font = <family>` in settings.

use gpui_kit::*;

use crate::assets::{MONO, SANS};
use crate::store::Settings;

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

/// UI and prose typeface (`sans`) plus the code/table one (`mono`) —
/// code always stays on the bundled mono.
#[derive(Clone)]
pub(crate) struct Fonts {
    pub sans: SharedString,
    pub mono: SharedString,
}

impl Global for Fonts {}

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

/// Resolve `settings.font()` against installed families and set `Fonts`.
pub(crate) fn apply(settings: &Settings, cx: &mut App) {
    let available = cx.text_system().all_font_names();
    let sans = resolve(settings.font(), &available);
    cx.set_global(Fonts {
        sans: sans.into(),
        mono: MONO.into(),
    });
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
