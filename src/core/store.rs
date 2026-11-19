//! Local persistence: settings live in `$XDG_CONFIG_HOME/abstract/settings`
//! and the session in `$XDG_STATE_HOME/abstract/session`, both as `key =
//! value` lines. Everything on disk goes through `write_atomic`.

use std::io;
use std::path::{Path, PathBuf};

use super::prefs::SpellLang;
use super::prefs::TextWidth;
use super::prefs::ThemePref;
use crate::core::i18n::LangPref;

/// XDG-style base dir: `var` wins; on Windows fall back to `%APPDATA%` /
/// `%LOCALAPPDATA%`, then `HOME`/`USERPROFILE` + `fallback`, else the cwd.
pub(crate) fn xdg(var: &str, fallback: &str) -> PathBuf {
    if let Some(p) = std::env::var_os(var) {
        return PathBuf::from(p);
    }
    if cfg!(windows) {
        let base = match var {
            "XDG_CONFIG_HOME" | "XDG_DATA_HOME" => "APPDATA",
            _ => "LOCALAPPDATA",
        };
        if let Some(p) = std::env::var_os(base) {
            return PathBuf::from(p);
        }
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| PathBuf::from(h).join(fallback))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn settings_file() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
        .join("abstract")
        .join("settings")
}

fn session_file() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
        .join("abstract")
        .join("session")
}

impl Session {
    pub fn snapshot(&self) -> (PathBuf, String) {
        (session_file(), self.kv.serialize())
    }
}

/// Write via a hidden sibling temp file + rename so a crash never leaves a
/// truncated target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    crate::io::atomic::write(path, bytes)
}

/// Ordered `key = value` lines. Unrecognized lines are kept verbatim so a
/// rewrite never drops keys a newer version wrote.
#[derive(Clone, Debug, Default)]
pub struct KeyVals {
    pub lines: Vec<String>,
}

impl KeyVals {
    pub fn parse(src: &str) -> Self {
        Self {
            lines: src
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect(),
        }
    }

    fn key_of(line: &str) -> Option<&str> {
        line.split_once('=').map(|(k, _)| k.trim())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim())
        })
    }

    /// Replace the first `key = …` line in place, or append one.
    pub fn set(&mut self, key: &str, value: &str) {
        for l in &mut self.lines {
            if Self::key_of(l) == Some(key) {
                *l = format!("{key} = {value}");
                return;
            }
        }
        self.lines.push(format!("{key} = {value}"));
    }

    pub fn remove(&mut self, key: &str) {
        self.lines.retain(|l| Self::key_of(l) != Some(key));
    }

    pub fn serialize(&self) -> String {
        self.lines.iter().map(|l| format!("{l}\n")).collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub kv: KeyVals,
}

impl Settings {
    pub fn preference(&self, key: &str) -> String {
        if matches!(key, "ui_font" | "editor_font") {
            return self
                .kv
                .get(key)
                .filter(|s| !s.is_empty())
                .unwrap_or(self.font())
                .into();
        }
        super::prefs::preferences()
            .into_iter()
            .find(|p| p.key == key)
            .map(|p| p.value(self.kv.get(key)))
            .unwrap_or_default()
    }

    pub fn number(&self, key: &str) -> f32 {
        self.preference(key).parse().unwrap_or_default()
    }

    pub fn enabled(&self, key: &str) -> bool {
        self.preference(key) == "on"
    }
    pub fn snapshot(&self) -> (PathBuf, String) {
        (settings_file(), self.kv.serialize())
    }

    /// Blocking. Missing/malformed lines fall back to defaults.
    pub fn load() -> Self {
        let kv = std::fs::read_to_string(settings_file())
            .map(|s| KeyVals::parse(&s))
            .unwrap_or_default();
        Self { kv }
    }

    pub fn theme(&self) -> ThemePref {
        self.kv
            .get("theme")
            .map_or(ThemePref::System, ThemePref::parse)
    }

    /// `system` | `en` | `pt-BR`; missing/unknown → system.
    pub fn lang(&self) -> LangPref {
        self.kv
            .get("lang")
            .map_or(LangPref::System, LangPref::parse)
    }

    pub fn set_lang(&mut self, lang: LangPref) {
        self.kv.set("lang", lang.as_str());
    }

    pub fn tour_done(&self) -> bool {
        self.kv.get("tour") == Some("done")
    }

    pub fn set_theme(&mut self, theme: ThemePref) {
        self.kv.set("theme", theme.as_str());
    }

    /// Id of the palette used in light mode (`theme::LIGHTS`).
    pub fn light_theme(&self) -> &str {
        self.kv.get("light_theme").unwrap_or("abstract")
    }

    pub fn set_light_theme(&mut self, id: &str) {
        self.kv.set("light_theme", id);
    }

    /// Id of the palette used in dark mode (`theme::DARKS`).
    pub fn dark_theme(&self) -> &str {
        self.kv.get("dark_theme").unwrap_or("abstract")
    }

    pub fn set_dark_theme(&mut self, id: &str) {
        self.kv.set("dark_theme", id);
    }

    /// `font = <family>` — UI/prose typeface; missing or empty keeps the
    /// bundled default (`fonts::resolve` validates it against installed
    /// families).
    pub fn font(&self) -> &str {
        self.kv
            .get("font")
            .filter(|s| !s.is_empty())
            .unwrap_or(super::prefs::SANS)
    }

    pub fn set_font(&mut self, name: &str) {
        self.kv.set("font", name);
    }

    /// `raw_tables = on` shows tables as their Markdown source instead of a
    /// grid.
    pub fn raw_tables(&self) -> bool {
        self.kv.get("raw_tables") == Some("on")
    }

    pub fn set_raw_tables(&mut self, on: bool) {
        self.kv.set("raw_tables", if on { "on" } else { "off" });
    }

    pub fn set_tour_done(&mut self) {
        self.kv.set("tour", "done");
    }

    /// `updates = off` disables the daily release check.
    pub fn updates(&self) -> bool {
        self.kv.get("updates") != Some("off")
    }

    pub fn set_updates(&mut self, on: bool) {
        self.kv.set("updates", if on { "on" } else { "off" });
    }

    /// `discord = off` keeps the Discord presence socket untouched.
    pub fn discord(&self) -> bool {
        self.kv.get("discord") != Some("off")
    }

    pub fn set_discord(&mut self, on: bool) {
        self.kv.set("discord", if on { "on" } else { "off" });
    }

    /// Unix seconds of the last release check.
    pub fn update_checked(&self) -> u64 {
        self.kv
            .get("update_checked")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    pub fn set_update_checked(&mut self, secs: u64) {
        self.kv.set("update_checked", &secs.to_string());
    }

    /// `zoom = 1.2` scales the whole UI; default 1.0 (100%).
    pub fn zoom(&self) -> f32 {
        self.kv
            .get("zoom")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(1.)
            .clamp(super::prefs::MIN_ZOOM, super::prefs::MAX_ZOOM)
    }

    pub fn set_zoom(&mut self, factor: f32) {
        let factor = if factor.is_finite() {
            factor.clamp(super::prefs::MIN_ZOOM, super::prefs::MAX_ZOOM)
        } else {
            1.
        };
        self.kv.set("zoom", &format!("{factor:.2}"));
    }

    /// `smart_quotes = off` keeps `"`/`'`/`--` exactly as typed.
    pub fn smart_quotes(&self) -> bool {
        self.kv.get("smart_quotes") != Some("off")
    }

    pub fn set_smart_quotes(&mut self, on: bool) {
        self.kv.set("smart_quotes", if on { "on" } else { "off" });
    }

    /// Blocking.
    pub fn save(&self) {
        let file = settings_file();
        if let Err(err) = write_atomic(&file, self.kv.serialize().as_bytes()) {
            eprintln!(
                "abstract: failed to save settings to {}: {err}",
                file.display()
            );
        }
    }

    /// `spellcheck = off` disables spell checking; on by default.
    pub fn spellcheck(&self) -> bool {
        self.kv.get("spellcheck") != Some("off")
    }

    pub fn set_spellcheck(&mut self, on: bool) {
        self.kv.set("spellcheck", if on { "on" } else { "off" });
    }

    /// `spell_lang = auto|en|pt-BR|both`; missing/unknown → auto (follows the
    /// UI language).
    pub fn spell_lang(&self) -> SpellLang {
        self.kv
            .get("spell_lang")
            .map_or(SpellLang::Auto, SpellLang::parse)
    }

    pub fn set_spell_lang(&mut self, lang: SpellLang) {
        self.kv.set("spell_lang", lang.as_str());
    }

    /// `text_width = narrow|medium|wide` caps the text column; missing or
    /// unknown reads as `Medium` (the old fixed 700pt column).
    pub fn text_width(&self) -> TextWidth {
        self.kv
            .get("text_width")
            .map_or(TextWidth::Medium, TextWidth::parse)
    }

    pub fn set_text_width(&mut self, width: TextWidth) {
        self.kv.set("text_width", width.as_str());
    }

    /// `glass = on|off` — translucent window surfaces; missing → the OS
    /// default (on where the compositor can blur, off on GNOME).
    pub fn glass(&self) -> bool {
        self.kv
            .get("glass")
            .map_or_else(super::prefs::default_enabled, |v| v == "on")
    }

    pub fn set_glass(&mut self, on: bool) {
        self.kv.set("glass", if on { "on" } else { "off" });
    }

    /// `glass_material = blur|acrylic|mica|mica-alt|compositor`; missing or
    /// from-another-OS values resolve to this OS's default.
    pub fn glass_material(&self) -> super::prefs::Material {
        self.kv.get("glass_material").map_or_else(
            super::prefs::Material::default_for_os,
            super::prefs::Material::parse,
        )
    }

    pub fn set_glass_material(&mut self, m: super::prefs::Material) {
        self.kv.set("glass_material", m.as_str());
    }

    /// `glass_intensity = 0-100` overall opacity multiplier.
    pub fn glass_intensity(&self) -> u8 {
        pct(self.kv.get("glass_intensity"), 100)
    }

    pub fn set_glass_intensity(&mut self, v: u8) {
        self.kv.set("glass_intensity", &v.min(100).to_string());
    }

    /// `glass_<surface> = off|0-100` per-surface opacity; missing → the
    /// surface default (sidebar/tab strip 75, others solid).
    pub fn glass_surface(&self, s: super::prefs::Surface) -> Option<u8> {
        match self.kv.get(s.key()) {
            None => s.default_percent(),
            Some("off") => None,
            Some(v) => v
                .parse::<u8>()
                .ok()
                .map(|p| p.min(100))
                .or_else(|| s.default_percent()),
        }
    }

    pub fn set_glass_surface(&mut self, s: super::prefs::Surface, v: Option<u8>) {
        let v = v.map_or_else(|| "off".to_string(), |p| p.min(100).to_string());
        self.kv.set(s.key(), &v);
    }

    /// `glass_tint = theme|custom`; custom reads `glass_tint_color = RRGGBB`.
    pub fn glass_tint(&self) -> bool {
        self.kv.get("glass_tint") == Some("custom")
    }

    pub fn set_glass_tint(&mut self, custom: bool) {
        self.kv
            .set("glass_tint", if custom { "custom" } else { "theme" });
    }

    pub fn glass_tint_color(&self) -> u32 {
        self.kv
            .get("glass_tint_color")
            .and_then(|v| u32::from_str_radix(v.trim_start_matches('#'), 16).ok())
            .filter(|c| *c <= 0xff_ff_ff)
            .unwrap_or(super::prefs::DEFAULT_TINT_COLOR)
    }

    pub fn set_glass_tint_color(&mut self, c: u32) {
        self.kv.set("glass_tint_color", &format!("{c:06x}"));
    }

    /// `glass_tint_strength = 0-100`.
    pub fn glass_tint_strength(&self) -> u8 {
        pct(self.kv.get("glass_tint_strength"), 50)
    }

    pub fn set_glass_tint_strength(&mut self, v: u8) {
        self.kv.set("glass_tint_strength", &v.min(100).to_string());
    }

    /// `glass_text_opacity = 0-100`.
    pub fn glass_text_opacity(&self) -> u8 {
        pct(self.kv.get("glass_text_opacity"), 100)
    }

    pub fn set_glass_text_opacity(&mut self, v: u8) {
        self.kv.set("glass_text_opacity", &v.min(100).to_string());
    }

    /// `glass_text_contrast = on|off` nudges editor text toward the
    /// foreground colour for legibility over busy wallpapers.
    pub fn glass_text_contrast(&self) -> bool {
        self.kv.get("glass_text_contrast") == Some("on")
    }

    pub fn set_glass_text_contrast(&mut self, on: bool) {
        self.kv
            .set("glass_text_contrast", if on { "on" } else { "off" });
    }
}

/// Parse a `0-100` percentage, `default` when missing or malformed.
fn pct(v: Option<&str>, default: u8) -> u8 {
    v.and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(default)
        .min(100)
}

/// `window = maximized|windowed <x> <y> <w> <h>` in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SessionWindow {
    pub maximized: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// One `note = <space>\t<rel path>\t<cursor>\t<scroll>` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionNote {
    pub space: PathBuf,
    pub rel: PathBuf,
    pub cursor: usize,
    pub scroll: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Session {
    pub kv: KeyVals,
}

impl Session {
    /// Blocking.
    pub fn load() -> Self {
        let kv = std::fs::read_to_string(session_file())
            .map(|s| KeyVals::parse(&s))
            .unwrap_or_default();
        Self { kv }
    }

    pub fn window(&self) -> Option<SessionWindow> {
        let v = self.kv.get("window")?;
        let mut it = v.split_whitespace();
        let maximized = match it.next()? {
            "maximized" | "fullscreen" => true,
            "windowed" => false,
            _ => return None,
        };
        Some(SessionWindow {
            maximized,
            x: it.next()?.parse().ok()?,
            y: it.next()?.parse().ok()?,
            w: it.next()?.parse().ok()?,
            h: it.next()?.parse().ok()?,
        })
    }

    /// `None` when the key is missing or malformed.
    pub fn sidebar_open(&self) -> Option<bool> {
        match self.kv.get("sidebar") {
            Some("open") => Some(true),
            Some("closed") => Some(false),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn notes(&self) -> Vec<SessionNote> {
        self.kv
            .lines
            .iter()
            .filter_map(|l| {
                let (k, v) = l.split_once('=')?;
                if k.trim() != "note" {
                    return None;
                }
                let mut parts = v.split('\t');
                let note = SessionNote {
                    space: PathBuf::from(parts.next()?.trim()),
                    rel: PathBuf::from(parts.next()?.trim()),
                    cursor: parts.next()?.trim().parse().ok()?,
                    scroll: parts.next()?.trim().parse().ok()?,
                };
                (parts.next().is_none() && !note.rel.as_os_str().is_empty()).then_some(note)
            })
            .collect()
    }

    pub fn set_window(&mut self, w: &SessionWindow) {
        let mode = if w.maximized { "maximized" } else { "windowed" };
        self.kv
            .set("window", &format!("{mode} {} {} {} {}", w.x, w.y, w.w, w.h));
    }

    pub fn set_sidebar(&mut self, open: bool) {
        self.kv.set("sidebar", if open { "open" } else { "closed" });
    }

    /// Paths containing a tab or newline can't be represented and are skipped.
    pub fn set_notes(&mut self, notes: &[SessionNote]) {
        self.kv.remove("note");
        for n in notes {
            let (space, rel) = (n.space.to_string_lossy(), n.rel.to_string_lossy());
            if space.contains(['\t', '\n']) || rel.contains(['\t', '\n']) {
                continue;
            }
            self.kv
                .lines
                .push(format!("note = {space}\t{rel}\t{}\t{}", n.cursor, n.scroll));
        }
    }

    /// `tab` lines: every open note of each space, in tab order. `note`
    /// lines from a pre-tab version parse the same way, so a space with no
    /// `tab` lines still restores one note.
    pub fn tabs(&self) -> Vec<SessionNote> {
        let parse = |l: &String| {
            let (k, v) = l.split_once('=')?;
            let k = k.trim();
            if k != "tab" && k != "note" {
                return None;
            }
            let mut parts = v.split('\t');
            let note = SessionNote {
                space: PathBuf::from(parts.next()?.trim()),
                rel: PathBuf::from(parts.next()?.trim()),
                cursor: parts.next()?.trim().parse().ok()?,
                scroll: parts.next()?.trim().parse().ok()?,
            };
            (parts.next().is_none() && !note.rel.as_os_str().is_empty())
                .then_some((k == "tab", note))
        };
        let (tabs, notes): (Vec<_>, Vec<_>) = self
            .kv
            .lines
            .iter()
            .filter_map(parse)
            .partition(|(is_tab, _)| *is_tab);
        let mut out: Vec<SessionNote> = tabs.into_iter().map(|(_, n)| n).collect();
        // Legacy `note` lines fill in spaces that have no `tab` lines.
        for (_, n) in notes {
            if !out.iter().any(|t| t.space == n.space) {
                out.push(n);
            }
        }
        out
    }

    /// `tab_active = <space>\t<rel>` — which tab was front-most.
    pub fn active_tab(&self) -> Option<(PathBuf, PathBuf)> {
        let v = self.kv.get("tab_active")?;
        let (space, rel) = v.split_once('\t')?;
        if rel.trim().is_empty() {
            return None;
        }
        Some((PathBuf::from(space.trim()), PathBuf::from(rel.trim())))
    }

    /// Paths containing a tab or newline can't be represented and are skipped.
    pub fn set_tabs(&mut self, notes: &[SessionNote]) {
        self.kv.remove("tab");
        for n in notes {
            let (space, rel) = (n.space.to_string_lossy(), n.rel.to_string_lossy());
            if space.contains(['\t', '\n']) || rel.contains(['\t', '\n']) {
                continue;
            }
            self.kv
                .lines
                .push(format!("tab = {space}\t{rel}\t{}\t{}", n.cursor, n.scroll));
        }
    }

    pub fn set_active_tab(&mut self, active: Option<(&Path, &Path)>) {
        match active {
            Some((space, rel)) => {
                let (space, rel) = (space.to_string_lossy(), rel.to_string_lossy());
                if space.contains(['\t', '\n']) || rel.contains(['\t', '\n']) {
                    self.kv.remove("tab_active");
                } else {
                    self.kv.set("tab_active", &format!("{space}\t{rel}"));
                }
            }
            None => self.kv.remove("tab_active"),
        }
    }

    /// Blocking.
    pub fn save(&self) {
        let file = session_file();
        if let Err(err) = write_atomic(&file, self.kv.serialize().as_bytes()) {
            eprintln!(
                "abstract: failed to save session to {}: {err}",
                file.display()
            );
        }
    }
}

#[cfg(test)]
mod tests;
