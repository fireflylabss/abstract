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

#[derive(Clone, Copy)]
pub enum Input {
    Choice(&'static [&'static str]),
    Color(&'static [&'static str]),
    Number { min: f32, max: f32, step: f32 },
    Opacity,
}

#[derive(Clone, Copy)]
pub struct Preference {
    pub key: &'static str,
    pub label: crate::core::i18n::Key,
    pub group: usize,
    pub default: &'static str,
    pub input: Input,
}

impl Preference {
    pub fn value(self, stored: Option<&str>) -> String {
        let value = stored.unwrap_or(self.default);
        match self.input {
            Input::Color(options) => {
                if options.contains(&value) {
                    value.into()
                } else {
                    let hex = value.trim_start_matches('#');
                    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                        hex.to_ascii_lowercase()
                    } else {
                        self.default.into()
                    }
                }
            }
            Input::Choice(options) => if options.contains(&value) {
                value
            } else {
                self.default
            }
            .into(),
            Input::Number { min, max, .. } => value
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(min, max).to_string())
                .unwrap_or_else(|| self.default.into()),
            Input::Opacity => {
                if value == "off" {
                    "off".into()
                } else {
                    value
                        .parse::<u8>()
                        .ok()
                        .map(|v| v.min(100).to_string())
                        .unwrap_or_else(|| self.default.into())
                }
            }
        }
    }

    pub fn step(self, stored: Option<&str>, forward: bool) -> String {
        let value = self.value(stored);
        match self.input {
            Input::Choice(options) | Input::Color(options) => {
                let n = options.len();
                let i = options.iter().position(|v| *v == value).unwrap_or(0);
                options[(i + if forward { 1 } else { n - 1 }) % n].into()
            }
            Input::Number { min, max, step } => {
                let value = value.parse::<f32>().unwrap_or(min);
                let v = (value + if forward { step } else { -step }).clamp(min, max);
                format!("{v:.2}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .into()
            }
            Input::Opacity => {
                let i = if value == "off" {
                    -1
                } else {
                    value.parse::<i32>().unwrap_or(75) / 5
                };
                let next = (i + if forward { 1 } else { -1 }).rem_euclid(22);
                if next == 21 {
                    "off".into()
                } else {
                    (next * 5).to_string()
                }
            }
        }
    }
}

pub const FONTS: &[&str] = &[
    "Noto Sans",
    "Noto Sans Mono",
    "Georgia",
    "Helvetica",
    "Times New Roman",
    "Courier New",
];

pub fn preferences() -> Vec<Preference> {
    use crate::core::i18n::Key as K;
    use Input::{Choice, Color, Number, Opacity};
    let field = |key, label, group, default, input| Preference {
        key,
        label,
        group,
        default,
        input,
    };
    vec![
        field(
            "theme",
            K::Theme,
            0,
            "system",
            Choice(&["system", "light", "dark"]),
        ),
        field(
            "light_theme",
            K::LightTheme,
            0,
            "abstract",
            Choice(&["abstract", "paper", "sepia", "solarized"]),
        ),
        field(
            "dark_theme",
            K::DarkTheme,
            0,
            "abstract",
            Choice(&["abstract", "midnight", "nord", "solarized"]),
        ),
        field(
            "accent",
            K::Accent,
            0,
            "theme",
            Color(&[
                "theme", "system", "6ea8fe", "5fc88f", "b392f0", "f0736b", "e5b454",
            ]),
        ),
        field("ui_font", K::UiFont, 0, SANS, Choice(FONTS)),
        field(
            "zoom",
            K::Zoom,
            0,
            "1",
            Number {
                min: 0.5,
                max: 2.,
                step: 0.1,
            },
        ),
        field(
            "sidebar_width",
            K::SidebarWidth,
            0,
            "238",
            Number {
                min: 160.,
                max: 480.,
                step: 10.,
            },
        ),
        field("single_tab", K::SingleTab, 0, "on", Choice(&["on", "off"])),
        field(
            "glass",
            K::GlassEnable,
            0,
            if default_enabled() { "on" } else { "off" },
            Choice(&["on", "off"]),
        ),
        field(
            "glass_material",
            K::GlassMaterial,
            0,
            Material::default_for_os().as_str(),
            Choice(match Material::default_for_os() {
                Material::Blur => &["blur"],
                Material::Compositor => &["compositor"],
                _ => &["acrylic", "mica", "mica-alt"],
            }),
        ),
        field(
            "glass_intensity",
            K::GlassIntensity,
            0,
            "100",
            Number {
                min: 0.,
                max: 100.,
                step: 5.,
            },
        ),
        field("glass_sidebar", K::GlassSidebar, 0, "75", Opacity),
        field("glass_tabs", K::GlassTabs, 0, "75", Opacity),
        field("glass_toolbar", K::GlassToolbar, 0, "off", Opacity),
        field("glass_menus", K::GlassMenus, 0, "off", Opacity),
        field("glass_panel", K::GlassPanel, 0, "off", Opacity),
        field("glass_editor", K::GlassEditor, 0, "off", Opacity),
        field(
            "glass_tint",
            K::GlassTint,
            0,
            "theme",
            Choice(&["theme", "custom"]),
        ),
        field(
            "glass_tint_color",
            K::GlassTintCustom,
            0,
            "6ea8fe",
            Color(&["6ea8fe", "5fc88f", "b392f0", "f0736b", "e5b454"]),
        ),
        field(
            "glass_tint_strength",
            K::GlassTintStrength,
            0,
            "50",
            Number {
                min: 0.,
                max: 100.,
                step: 5.,
            },
        ),
        field(
            "glass_text_opacity",
            K::GlassTextOpacity,
            0,
            "100",
            Number {
                min: 0.,
                max: 100.,
                step: 5.,
            },
        ),
        field(
            "glass_text_contrast",
            K::GlassTextContrast,
            0,
            "off",
            Choice(&["off", "on"]),
        ),
        field("editor_font", K::EditorFont, 1, SANS, Choice(FONTS)),
        field(
            "editor_size",
            K::EditorSize,
            1,
            "17",
            Number {
                min: 10.,
                max: 36.,
                step: 1.,
            },
        ),
        field(
            "line_height",
            K::LineHeight,
            1,
            "1.25",
            Number {
                min: 1.,
                max: 2.5,
                step: 0.05,
            },
        ),
        field(
            "paragraph_spacing",
            K::ParagraphSpacing,
            1,
            "0",
            Number {
                min: 0.,
                max: 30.,
                step: 2.,
            },
        ),
        field(
            "text_width",
            K::TextWidth,
            1,
            "medium",
            Choice(&["narrow", "medium", "wide"]),
        ),
        field("conceal", K::Conceal, 1, "on", Choice(&["on", "off"])),
        field(
            "heading_scale",
            K::HeadingScale,
            1,
            "1",
            Number {
                min: 0.7,
                max: 1.8,
                step: 0.1,
            },
        ),
        field(
            "caret_style",
            K::CaretStyle,
            1,
            "bar",
            Choice(&["bar", "block", "underline"]),
        ),
        field(
            "caret_blink",
            K::CaretBlink,
            1,
            "on",
            Choice(&["on", "off"]),
        ),
        field(
            "smart_pairs",
            K::SmartPairs,
            1,
            "off",
            Choice(&["off", "on"]),
        ),
        field(
            "smart_quotes",
            K::SmartQuotes,
            1,
            "on",
            Choice(&["on", "off"]),
        ),
        field(
            "lang",
            K::Language,
            2,
            "system",
            Choice(&["system", "en", "pt-BR"]),
        ),
        field(
            "autosave_delay",
            K::AutosaveDelay,
            2,
            "0.6",
            Number {
                min: 0.1,
                max: 10.,
                step: 0.1,
            },
        ),
    ]
}

#[cfg(test)]
#[path = "prefs_tests.rs"]
mod tests;
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
