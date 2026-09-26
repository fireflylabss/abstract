//! Theme preference (Sistema/Claro/Escuro) and the color palette. The palette
//! is an app-global snapshot: render code copies it out of `cx.global()`.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

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
};

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

    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Light => "Claro",
            Self::Dark => "Escuro",
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

/// Set the palette global and push the matching mode into the component
/// library so tooltips/inputs follow.
pub fn apply(pref: ThemePref, appearance: WindowAppearance, cx: &mut App) {
    let light = match pref {
        ThemePref::Light => true,
        ThemePref::Dark => false,
        ThemePref::System => matches!(
            appearance,
            WindowAppearance::Light | WindowAppearance::VibrantLight
        ),
    };
    cx.set_global(if light { LIGHT } else { DARK });
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
