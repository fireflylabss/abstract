//! Stored settings values, independent of the UI framework.

pub const MOD: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
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

    pub fn icon(&self) -> &'static str {
        match self {
            Self::System => "icons/monitor.svg",
            Self::Light => "icons/sun.svg",
            Self::Dark => "icons/moon.svg",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextWidth {
    Narrow,
    #[default]
    Medium,
    Wide,
}

impl TextWidth {
    pub const ALL: [Self; 3] = [Self::Narrow, Self::Medium, Self::Wide];

    /// Missing or unknown values read as `Medium`.
    pub fn parse(s: &str) -> Self {
        match s {
            "narrow" => Self::Narrow,
            "wide" => Self::Wide,
            _ => Self::Medium,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Narrow => "narrow",
            Self::Medium => "medium",
            Self::Wide => "wide",
        }
    }

    /// Column cap in points, before the zoom factor.
    pub fn max_col(&self) -> f32 {
        match self {
            Self::Narrow => 580.,
            Self::Medium => 700.,
            Self::Wide => 880.,
        }
    }
}

pub const DEFAULT_TINT_COLOR: u32 = 0x6ea8fe;

/// A paintable surface whose opacity can be tuned independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Sidebar,
    Tabs,
    Toolbar,
    Menus,
    Panel,
    Editor,
}

impl Surface {
    pub const ALL: [Surface; 6] = [
        Surface::Sidebar,
        Surface::Tabs,
        Surface::Toolbar,
        Surface::Menus,
        Surface::Panel,
        Surface::Editor,
    ];

    /// Settings key holding `off` or an opacity percentage.
    pub fn key(self) -> &'static str {
        match self {
            Surface::Sidebar => "glass_sidebar",
            Surface::Tabs => "glass_tabs",
            Surface::Toolbar => "glass_toolbar",
            Surface::Menus => "glass_menus",
            Surface::Panel => "glass_panel",
            Surface::Editor => "glass_editor",
        }
    }

    /// Default opacity (percent) when glass is first turned on: the sidebar
    /// and tab strip go translucent, everything else stays solid.
    pub fn default_percent(self) -> Option<u8> {
        match self {
            Surface::Sidebar | Surface::Tabs => Some(75),
            _ => None,
        }
    }
}

/// The window material; the stored value is validated against the running
/// OS because each OS exposes a different set of materials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// macOS NSVisualEffect blur.
    Blur,
    /// Windows acrylic (Win10+).
    Acrylic,
    /// Windows DWM Mica (Win11).
    Mica,
    /// Windows DWM Mica Alt (Win11).
    MicaAlt,
    /// Linux: Transparent; the compositor (Hyprland/KDE/picom) applies its
    /// own blur when it has one. On KDE a `Blurred` request is made.
    Compositor,
}

impl Material {
    /// Materials meaningful on the running OS.
    pub fn options() -> &'static [Material] {
        #[cfg(target_os = "macos")]
        {
            &[Material::Blur]
        }
        #[cfg(target_os = "windows")]
        {
            &[Material::Acrylic, Material::Mica, Material::MicaAlt]
        }
        #[cfg(target_os = "linux")]
        {
            &[Material::Compositor]
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            &[Material::Compositor]
        }
    }

    pub fn default_for_os() -> Material {
        Self::options()[0]
    }

    pub fn parse(s: &str) -> Material {
        let m = match s {
            "blur" => Material::Blur,
            "acrylic" => Material::Acrylic,
            "mica" => Material::Mica,
            "mica-alt" => Material::MicaAlt,
            "compositor" => Material::Compositor,
            _ => Material::default_for_os(),
        };
        // A stored material from another OS falls back to this OS's default.
        if Self::options().contains(&m) {
            m
        } else {
            Material::default_for_os()
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Material::Blur => "blur",
            Material::Acrylic => "acrylic",
            Material::Mica => "mica",
            Material::MicaAlt => "mica-alt",
            Material::Compositor => "compositor",
        }
    }
}

pub fn default_enabled() -> bool {
    !std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
        .contains("gnome")
}
pub const SANS: &str = "Noto Sans";
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 2.;

/// `spell-lang` setting: `auto` (default) follows the UI language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpellLang {
    Auto,
    En,
    PtBr,
    Both,
}

impl SpellLang {
    pub fn parse(s: &str) -> Self {
        match s {
            "en" => Self::En,
            "pt-BR" | "pt" => Self::PtBr,
            "both" => Self::Both,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::En => "en",
            Self::PtBr => "pt-BR",
            Self::Both => "both",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::En,
            Self::En => Self::PtBr,
            Self::PtBr => Self::Both,
            Self::Both => Self::Auto,
        }
    }
}

/// Stored in settings key `lang`: `system` | `en` | `pt-BR`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LangPref {
    System,
    En,
    PtBr,
}

impl LangPref {
    pub fn parse(s: &str) -> Self {
        match s {
            "en" => Self::En,
            "pt-BR" | "pt" => Self::PtBr,
            _ => Self::System,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }

    /// Sistema → En → PtBr → Sistema (the menu row cycle).
    pub fn next(&self) -> Self {
        match self {
            Self::System => Self::En,
            Self::En => Self::PtBr,
            Self::PtBr => Self::System,
        }
    }
}
