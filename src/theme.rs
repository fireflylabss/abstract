//! Theme preference (Sistema/Claro/Escuro), the named light and dark
//! palettes, and the active palette: an app-global snapshot render code
//! copies out of `cx.global()`.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

use crate::store::Settings;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: u32,
    pub panel: u32,
    pub hover: u32,
    pub active: u32,
    pub line: u32,
    pub fg: u32,
    pub body: u32,
    pub dim: u32,
    pub faint: u32,
    pub menu_bg: u32,
    pub menu_border: u32,
    pub frame_border: u32,
    pub head: u32,
    pub quote: u32,
    pub mark: u32,
    pub muted: u32,
    pub code_bg: u32,
    pub code_kw: u32,
    pub code_str: u32,
    pub code_comment: u32,
    pub code_num: u32,
    pub inline_code_bg: u32,
    pub rule: u32,
    pub caret: u32,
    /// rgba, alpha included.
    pub selection: u32,
    /// `==marked==` text background, rgba.
    pub highlight: u32,
    /// Active find match background, rgba.
    pub find_current: u32,
    /// Callout accents by `md::Tone`: note, tip, warning, danger.
    pub callout: [u32; 4],
    /// `=={tint}…==` backgrounds by `md::Tint`, rgba — the palette's accent
    /// hues at the highlight's alpha.
    pub marks: [u32; 6],
}

impl Global for Palette {}

/// Pure monochrome, dark.
pub const DARK: Palette = Palette {
    bg: 0x0a0a0a,
    panel: 0x0f0f0f,
    hover: 0x1a1a1a,
    active: 0x222222,
    line: 0x1f1f1f,
    fg: 0xededed,
    body: 0xd4d4d4,
    dim: 0x8a8a8a,
    faint: 0x555555,
    menu_bg: 0x141414,
    menu_border: 0x2a2a2a,
    frame_border: 0x262626,
    head: 0xffffff,
    quote: 0x9a9a9a,
    mark: 0x4d4d4d,
    muted: 0x7a7a7a,
    code_bg: 0x161616,
    code_kw: 0xededed,
    code_str: 0xb8b8b8,
    code_comment: 0x6a6a6a,
    code_num: 0xcfcfcf,
    inline_code_bg: 0x1f1f1f,
    rule: 0x2e2e2e,
    caret: 0xf2f2f2,
    selection: 0xffffff26,
    highlight: 0xe5c07b40,
    find_current: 0xe5a13bb3,
    callout: [0x6ea8fe, 0x5fc88f, 0xe5b454, 0xf0736b],
    marks: [
        0xf0736b40, 0xd19a6640, 0xe5c07b40, 0x5fc88f40, 0x6ea8fe40, 0xb392f040,
    ],
};

/// Warm monochrome, light.
pub const LIGHT: Palette = Palette {
    bg: 0xfafafa,
    panel: 0xf3f3f2,
    hover: 0xe9e9e7,
    active: 0xe0e0de,
    line: 0xe4e4e2,
    fg: 0x111111,
    body: 0x2b2b2b,
    dim: 0x6b6b6b,
    faint: 0x9a9a9a,
    menu_bg: 0xffffff,
    menu_border: 0xdedede,
    frame_border: 0xd6d6d6,
    head: 0x000000,
    quote: 0x5c5c5c,
    mark: 0xb5b5b5,
    muted: 0x777777,
    code_bg: 0xf0f0ee,
    code_kw: 0x111111,
    code_str: 0x4a4a4a,
    code_comment: 0x9a9a9a,
    code_num: 0x333333,
    inline_code_bg: 0xe8e8e6,
    rule: 0xdcdcda,
    caret: 0x111111,
    selection: 0x0000001f,
    highlight: 0xffd84a80,
    find_current: 0xff9f1ab3,
    callout: [0x2f6fd6, 0x1f8a52, 0xa66a00, 0xc4372f],
    marks: [
        0xc4372f80, 0xd9770680, 0xffd84a80, 0x1f8a5280, 0x2f6fd680, 0x7c3aed80,
    ],
};

/// Cool neutrals on white, light.
pub const PAPER: Palette = Palette {
    bg: 0xffffff,
    panel: 0xf6f8fa,
    hover: 0xeaeef2,
    active: 0xdde3ea,
    line: 0xe1e4e8,
    fg: 0x1f2328,
    body: 0x24292f,
    dim: 0x57606a,
    faint: 0x8c959f,
    menu_bg: 0xffffff,
    menu_border: 0xd0d7de,
    frame_border: 0xd0d7de,
    head: 0x1f2328,
    quote: 0x57606a,
    mark: 0xafb8c1,
    muted: 0x6e7781,
    code_bg: 0xf6f8fa,
    code_kw: 0xcf222e,
    code_str: 0x0a3069,
    code_comment: 0x6e7781,
    code_num: 0x0550ae,
    inline_code_bg: 0xeff1f3,
    rule: 0xd8dee4,
    caret: 0x1f2328,
    selection: 0x0969da26,
    highlight: 0xffd33d66,
    find_current: 0xff9a00b3,
    callout: [0x0969da, 0x1a7f37, 0x9a6700, 0xcf222e],
    marks: [
        0xcf222e66, 0xbc4c0066, 0xffd33d66, 0x1a7f3766, 0x0969da66, 0x8250df66,
    ],
};

/// Warm paper and brown ink, light.
pub const SEPIA: Palette = Palette {
    bg: 0xf7f1e3,
    panel: 0xefe7d4,
    hover: 0xe6dcc6,
    active: 0xddd1b8,
    line: 0xe2d8c3,
    fg: 0x2a2118,
    body: 0x3b3025,
    dim: 0x7a6a55,
    faint: 0xa6957c,
    menu_bg: 0xfbf6ea,
    menu_border: 0xdccfb5,
    frame_border: 0xd6c8ab,
    head: 0x1e160e,
    quote: 0x6b5a45,
    mark: 0xbfae92,
    muted: 0x8a785f,
    code_bg: 0xece3cd,
    code_kw: 0x8b3a1e,
    code_str: 0x5f7a2a,
    code_comment: 0xa6957c,
    code_num: 0x9a5b13,
    inline_code_bg: 0xe6dcc4,
    rule: 0xd9ccb1,
    caret: 0x2a2118,
    selection: 0x7a5a2a2e,
    highlight: 0xf2c14e80,
    find_current: 0xe8891ab3,
    callout: [0x3a6ea5, 0x4f7d2c, 0x9a6a00, 0xb03a2e],
    marks: [
        0xb03a2e80, 0xc0562180, 0xf2c14e80, 0x4f7d2c80, 0x3a6ea580, 0x7d5ba680,
    ],
};

/// Solarized (Ethan Schoonover), light.
pub const SOLARIZED_LIGHT: Palette = Palette {
    bg: 0xfdf6e3,
    panel: 0xf5eedb,
    hover: 0xeee8d5,
    active: 0xe4ddc8,
    line: 0xe9e2cc,
    fg: 0x073642,
    body: 0x586e75,
    dim: 0x657b83,
    faint: 0x93a1a1,
    menu_bg: 0xfffbee,
    menu_border: 0xe0d9c3,
    frame_border: 0xd9d2bc,
    head: 0x002b36,
    quote: 0x657b83,
    mark: 0xb4bcb4,
    muted: 0x93a1a1,
    code_bg: 0xeee8d5,
    code_kw: 0x859900,
    code_str: 0x2aa198,
    code_comment: 0x93a1a1,
    code_num: 0xd33682,
    inline_code_bg: 0xeee8d5,
    rule: 0xe0d9c3,
    caret: 0x073642,
    selection: 0x268bd22e,
    highlight: 0xb5890040,
    find_current: 0xcb4b1699,
    callout: [0x268bd2, 0x859900, 0xb58900, 0xdc322f],
    marks: [
        0xdc322f40, 0xcb4b1640, 0xb5890040, 0x85990040, 0x268bd240, 0x6c71c440,
    ],
};

/// Blue-black with cool grays, dark.
pub const MIDNIGHT: Palette = Palette {
    bg: 0x0d1117,
    panel: 0x010409,
    hover: 0x161b22,
    active: 0x1f2630,
    line: 0x21262d,
    fg: 0xe6edf3,
    body: 0xc9d1d9,
    dim: 0x8b949e,
    faint: 0x6e7681,
    menu_bg: 0x161b22,
    menu_border: 0x30363d,
    frame_border: 0x30363d,
    head: 0xf0f6fc,
    quote: 0x8b949e,
    mark: 0x484f58,
    muted: 0x7d8590,
    code_bg: 0x161b22,
    code_kw: 0xff7b72,
    code_str: 0xa5d6ff,
    code_comment: 0x8b949e,
    code_num: 0x79c0ff,
    inline_code_bg: 0x1f2630,
    rule: 0x30363d,
    caret: 0xe6edf3,
    selection: 0x388bfd40,
    highlight: 0xbb800966,
    find_current: 0xd29922b3,
    callout: [0x4493f8, 0x3fb950, 0xd29922, 0xf85149],
    marks: [
        0xf8514966, 0xdb6d2866, 0xbb800966, 0x3fb95066, 0x4493f866, 0xa371f766,
    ],
};

/// Nord (Arctic Ice Studio), dark.
pub const NORD: Palette = Palette {
    bg: 0x2e3440,
    panel: 0x2a2f3a,
    hover: 0x3b4252,
    active: 0x434c5e,
    line: 0x3b4252,
    fg: 0xeceff4,
    body: 0xd8dee9,
    dim: 0x9aa5b8,
    faint: 0x6c7891,
    menu_bg: 0x3b4252,
    menu_border: 0x4c566a,
    frame_border: 0x434c5e,
    head: 0x88c0d0,
    quote: 0xa3acbd,
    mark: 0x4c566a,
    muted: 0x7b88a1,
    code_bg: 0x333a47,
    code_kw: 0x81a1c1,
    code_str: 0xa3be8c,
    code_comment: 0x6c7891,
    code_num: 0xb48ead,
    inline_code_bg: 0x3b4252,
    rule: 0x434c5e,
    caret: 0xd8dee9,
    selection: 0x88c0d033,
    highlight: 0xebcb8b40,
    find_current: 0xd08770b3,
    callout: [0x81a1c1, 0xa3be8c, 0xebcb8b, 0xbf616a],
    marks: [
        0xbf616a40, 0xd0877040, 0xebcb8b40, 0xa3be8c40, 0x81a1c140, 0xb48ead40,
    ],
};

/// Solarized (Ethan Schoonover), dark.
pub const SOLARIZED_DARK: Palette = Palette {
    bg: 0x002b36,
    panel: 0x00252f,
    hover: 0x073642,
    active: 0x0a4252,
    line: 0x073642,
    fg: 0xeee8d5,
    body: 0x93a1a1,
    dim: 0x839496,
    faint: 0x5f7880,
    menu_bg: 0x073642,
    menu_border: 0x0f4a5a,
    frame_border: 0x0a3f4e,
    head: 0xfdf6e3,
    quote: 0x839496,
    mark: 0x3d5a63,
    muted: 0x708a92,
    code_bg: 0x04303c,
    code_kw: 0x859900,
    code_str: 0x2aa198,
    code_comment: 0x5f7880,
    code_num: 0xd33682,
    inline_code_bg: 0x073642,
    rule: 0x0f4a5a,
    caret: 0xeee8d5,
    selection: 0x268bd240,
    highlight: 0xb5890050,
    find_current: 0xcb4b16b3,
    callout: [0x268bd2, 0x859900, 0xb58900, 0xdc322f],
    marks: [
        0xdc322f50, 0xcb4b1650, 0xb5890050, 0x85990050, 0x268bd250, 0x6c71c450,
    ],
};

/// A palette the settings can pick by `id`; `name` is shown as is.
pub struct Named {
    pub id: &'static str,
    pub name: &'static str,
    pub palette: Palette,
}

/// Light palettes, the default first.
pub const LIGHTS: [Named; 4] = [
    Named {
        id: "abstract",
        name: "Abstract",
        palette: LIGHT,
    },
    Named {
        id: "paper",
        name: "Paper",
        palette: PAPER,
    },
    Named {
        id: "sepia",
        name: "Sepia",
        palette: SEPIA,
    },
    Named {
        id: "solarized",
        name: "Solarized",
        palette: SOLARIZED_LIGHT,
    },
];

/// Dark palettes, the default first.
pub const DARKS: [Named; 4] = [
    Named {
        id: "abstract",
        name: "Abstract",
        palette: DARK,
    },
    Named {
        id: "midnight",
        name: "Midnight",
        palette: MIDNIGHT,
    },
    Named {
        id: "nord",
        name: "Nord",
        palette: NORD,
    },
    Named {
        id: "solarized",
        name: "Solarized",
        palette: SOLARIZED_DARK,
    },
];

/// The palette in `list` with `id`; unknown ids fall back to the first.
pub fn named<'a>(list: &'a [Named], id: &str) -> &'a Named {
    list.iter().find(|n| n.id == id).unwrap_or(&list[0])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePref {
    System,
    Light,
    Dark,
}

impl ThemePref {
    pub fn parse(s: &str) -> Self {
        match s {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Sistema → Claro → Escuro → Sistema.
    pub fn next(&self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::System => "icons/monitor.svg",
            Self::Light => "icons/sun.svg",
            Self::Dark => "icons/moon.svg",
        }
    }
}

/// Shorthand for `*cx.global::<Palette>()` — resolves on `Context` too via
/// deref.
pub trait PaletteAccess {
    fn palette(&self) -> Palette;
}

impl PaletteAccess for App {
    fn palette(&self) -> Palette {
        *self.global::<Palette>()
    }
}

/// Set the palette global from `settings` (mode plus the chosen light and
/// dark palettes) and push the matching mode into the component library so
/// tooltips/inputs follow.
pub fn apply(settings: &Settings, appearance: WindowAppearance, cx: &mut App) {
    let light = match settings.theme() {
        ThemePref::Light => true,
        ThemePref::Dark => false,
        ThemePref::System => matches!(
            appearance,
            WindowAppearance::Light | WindowAppearance::VibrantLight
        ),
    };
    cx.set_global(if light {
        named(&LIGHTS, settings.light_theme()).palette
    } else {
        named(&DARKS, settings.dark_theme()).palette
    });
    Theme::change(
        if light {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        },
        None,
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::{DARKS, LIGHTS, NORD, named};

    #[test]
    fn named_palettes_have_unique_ids_and_a_fallback() {
        for list in [&LIGHTS[..], &DARKS[..]] {
            for (i, a) in list.iter().enumerate() {
                assert!(list[i + 1..].iter().all(|b| b.id != a.id), "{}", a.id);
            }
            assert_eq!(named(list, "nope").id, "abstract");
        }
        assert_eq!(named(&DARKS, "nord").palette.bg, NORD.bg);
    }
}
