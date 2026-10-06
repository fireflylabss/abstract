//! Configurable translucency ("glass"): a per-OS window material plus
//! per-surface alpha composited over whatever the OS shows behind the
//! window. All settings resolve through [`Glass`], which lives in a global
//! so render sites only read it — toggling glass off makes every helper
//! return the original opaque colour, byte-identical to before.
//!
//! Platform reality (GPUI 0.3.4 `WindowBackgroundAppearance`): macOS
//! `Blurred` is a NSVisualEffect-style blur; Windows `Blurred` is acrylic
//! (Win10+) and `MicaBackdrop`/`MicaAltBackdrop` are DWM Mica (Win11);
//! Linux Wayland `Blurred` only reaches KDE via org_kde_kwin_blur, while
//! compositors like Hyprland blur any `Transparent` window themselves;
//! X11 gets transparency only; GNOME offers no blur at all.

use gpui_kit::component::Theme;
use gpui_kit::*;

use crate::store::Settings;

/// Fallback colour for a custom tint: a soft blue that reads well over
/// both light and dark wallpapers.
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

/// Resolved glass configuration, kept in a global so render sites only
/// read it. Built from [`Settings`] by [`Glass::from_settings`].
#[derive(Clone, Debug)]
pub struct Glass {
    pub enabled: bool,
    pub material: Material,
    /// Overall multiplier applied to every surface's opacity, 0..=1.
    intensity: f32,
    /// Per-surface opacity 0..=1; `None` keeps the surface opaque.
    alphas: [Option<f32>; 6],
    /// Custom tint colour when `glass_tint = custom`.
    tint_color: Option<Hsla>,
    /// Tint blend strength 0..=1.
    tint_strength: f32,
    /// Editor text opacity 0..=1.
    text_opacity: f32,
    /// Push editor text toward the strongest foreground colour.
    text_contrast: bool,
}

impl Global for Glass {}

impl Glass {
    pub fn from_settings(s: &Settings) -> Glass {
        Glass {
            enabled: s.glass(),
            material: s.glass_material(),
            intensity: f32::from(s.glass_intensity()) / 100.,
            alphas: Surface::ALL.map(|surf| s.glass_surface(surf).map(|p| f32::from(p) / 100.)),
            tint_color: if s.glass_tint() {
                Some(rgb(s.glass_tint_color()).into())
            } else {
                None
            },
            tint_strength: f32::from(s.glass_tint_strength()) / 100.,
            text_opacity: f32::from(s.glass_text_opacity()) / 100.,
            text_contrast: s.glass_text_contrast(),
        }
    }

    /// Effective alpha for a surface: `None` when the surface stays opaque
    /// (glass off, or the surface disabled), else 0..=1 with the overall
    /// intensity folded in.
    pub fn alpha(&self, s: Surface) -> Option<f32> {
        if !self.enabled {
            return None;
        }
        self.alphas[s as usize].map(|a| (a * self.intensity).clamp(0., 1.))
    }

    /// `base` shaded for a surface: the opaque input untouched when the
    /// surface is opaque; otherwise tinted and given the surface alpha.
    pub fn shade(&self, base: Hsla, s: Surface) -> Hsla {
        let Some(a) = self.alpha(s) else {
            return Hsla { a: 1., ..base };
        };
        let mut c = base;
        if let Some(t) = self.tint_color {
            c = blend(c, t, self.tint_strength);
        }
        c.a = a;
        c
    }

    /// Editor text alpha: 1.0 when glass is off or text is untouched.
    pub fn text_alpha(&self) -> f32 {
        if self.enabled { self.text_opacity } else { 1. }
    }

    /// Whether editor text should be pushed toward the foreground colour.
    pub fn text_contrast(&self) -> bool {
        self.enabled && self.text_contrast
    }

    /// The OS window material for the current settings. `material` is
    /// already clamped to this OS's options by [`Material::parse`], so the
    /// arms here only see valid values: Blur (macOS), Acrylic/Mica/Mica Alt
    /// (Windows), Compositor (Linux).
    pub fn appearance(&self) -> WindowBackgroundAppearance {
        if !self.enabled {
            return WindowBackgroundAppearance::Opaque;
        }
        match self.material {
            // macOS Blur and Windows acrylic both map to `Blurred`.
            Material::Blur | Material::Acrylic => WindowBackgroundAppearance::Blurred,
            Material::Mica => WindowBackgroundAppearance::MicaBackdrop,
            Material::MicaAlt => WindowBackgroundAppearance::MicaAltBackdrop,
            Material::Compositor => {
                // KDE honours an explicit blur request; other compositors
                // (Hyprland, picom) blur a transparent window themselves.
                #[cfg(target_os = "linux")]
                {
                    if kde() {
                        WindowBackgroundAppearance::Blurred
                    } else {
                        WindowBackgroundAppearance::Transparent
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    WindowBackgroundAppearance::Transparent
                }
            }
        }
    }
}

/// Whether glass defaults on for this OS: on where a blur material exists
/// (macOS, Windows, Linux outside GNOME), off on GNOME which cannot blur.
pub fn default_enabled() -> bool {
    #[cfg(target_os = "linux")]
    {
        !is_gnome()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// True on GNOME (and GNOME-derived sessions like ubuntu:GNOME), which has
/// no window blur; glass there fades to the desktop instead.
pub fn is_gnome() -> bool {
    #[cfg(target_os = "linux")]
    {
        desktop_env_contains("gnome")
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// True on KDE Plasma, where `Blurred` maps to org_kde_kwin_blur.
#[cfg(target_os = "linux")]
pub fn kde() -> bool {
    desktop_env_contains("kde") || desktop_env_contains("plasma")
}

#[cfg(target_os = "linux")]
fn desktop_env_contains(needle: &str) -> bool {
    ["XDG_CURRENT_DESKTOP", "DESKTOP_SESSION"]
        .iter()
        .any(|var| {
            std::env::var(var)
                .map(|v| v.to_lowercase().contains(needle))
                .unwrap_or(false)
        })
}

/// Whether glass is active right now.
pub fn enabled(cx: &App) -> bool {
    cx.try_global::<Glass>().is_some_and(|g| g.enabled)
}

/// Surface background for `color` (`Palette` u32): opaque and identical to
/// the input when glass or the surface is off, else translucent + tinted.
pub fn bg(color: u32, surface: Surface, cx: &App) -> Hsla {
    let base: Hsla = rgb(color).into();
    match cx.try_global::<Glass>() {
        Some(g) => g.shade(base, surface),
        None => base,
    }
}

/// Root window background: fully transparent when glass is on so the
/// material shows through between surfaces.
pub fn root_bg(color: u32, cx: &App) -> Hsla {
    if enabled(cx) {
        transparent_black()
    } else {
        rgb(color).into()
    }
}

/// Blend `a` toward `b` by `t` in HSLA space (hue/lightness/saturation;
/// alpha untouched).
pub fn blend(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0., 1.);
    Hsla {
        h: a.h + (b.h - a.h) * t,
        s: a.s + (b.s - a.s) * t,
        l: a.l + (b.l - a.l) * t,
        a: a.a,
    }
}

/// Channel-wise lerp of two packed RGB colours (text contrast nudge).
pub fn mix_u32(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let ch = |shift: u32| {
        let x = ((a >> shift) & 0xff) as f32;
        let y = ((b >> shift) & 0xff) as f32;
        (x + (y - x) * t).round() as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Install the resolved [`Glass`] global and retint the component theme's
/// popover token so gpui-component menus (context menus) match the Menus
/// surface. Call after every `theme::apply` — `Theme::change` rebuilds the
/// popover token from the theme config, so the retint must be re-applied.
pub fn apply(settings: &Settings, cx: &mut App) {
    let glass = Glass::from_settings(settings);
    {
        let theme = Theme::global_mut(cx);
        let base = theme.popover;
        theme.popover = glass.shade(base, Surface::Menus);
    }
    Theme::sync_base(cx);
    cx.set_global(glass);
}

/// Apply the window material live; no restart needed.
pub fn apply_window(settings: &Settings, window: &Window) {
    window.set_background_appearance(Glass::from_settings(settings).appearance());
}

#[cfg(test)]
mod tests {
    // Explicit imports only: `use super::*` re-exports the whole
    // `gpui_kit::*` glob, and resolving that many macros makes rustc
    // hit the recursion limit expanding `#[test]`.
    use super::{Glass, Surface, mix_u32};
    use crate::store::{KeyVals, Settings};
    use gpui_kit::{Hsla, WindowBackgroundAppearance, rgb};

    fn settings(kv: &str) -> Settings {
        Settings {
            kv: KeyVals::parse(kv),
        }
    }

    #[test]
    fn off_is_opaque_and_identical() {
        let g = Glass::from_settings(&settings("glass = off\n"));
        for s in Surface::ALL {
            assert_eq!(g.alpha(s), None);
        }
        let base: Hsla = rgb(0x112233).into();
        assert_eq!(g.shade(base, Surface::Sidebar), base);
        assert_eq!(g.text_alpha(), 1.);
        assert_eq!(g.appearance(), WindowBackgroundAppearance::Opaque);
    }

    #[test]
    fn surface_opacity_applied() {
        let g = Glass::from_settings(&settings("glass = on\nglass_sidebar = 50\n"));
        let base: Hsla = rgb(0x112233).into();
        let c = g.shade(base, Surface::Sidebar);
        assert!((c.a - 0.5).abs() < 1e-4);
        // Editor defaults to solid.
        assert_eq!(g.shade(base, Surface::Editor).a, 1.);
        assert_eq!(c.h, base.h);
    }

    #[test]
    fn intensity_scales_every_surface() {
        let g = Glass::from_settings(&settings(
            "glass = on\nglass_sidebar = 80\nglass_intensity = 50\n",
        ));
        let base: Hsla = rgb(0x112233).into();
        assert!((g.shade(base, Surface::Sidebar).a - 0.4).abs() < 1e-4);
    }

    #[test]
    fn tint_blends_hue_but_not_alpha() {
        let g = Glass::from_settings(&settings(
            "glass = on\nglass_sidebar = 50\nglass_tint = custom\nglass_tint_color = ff0000\nglass_tint_strength = 100\n",
        ));
        let c = g.shade(rgb(0x112233).into(), Surface::Sidebar);
        assert!((c.h - Hsla::from(rgb(0xff0000)).h).abs() < 0.02);
        assert!((c.a - 0.5).abs() < 1e-4);
    }

    #[test]
    fn text_opacity_and_contrast() {
        let g = Glass::from_settings(&settings(
            "glass = on\nglass_text_opacity = 40\nglass_text_contrast = on\n",
        ));
        assert!((g.text_alpha() - 0.4).abs() < 1e-4);
        assert!(g.text_contrast());
        let mixed = mix_u32(0x888888, 0xffffff, 0.5);
        assert!(mixed > 0x888888 && mixed < 0xffffff);
    }
}
