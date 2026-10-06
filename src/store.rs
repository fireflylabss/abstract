//! Local persistence: settings live in `$XDG_CONFIG_HOME/abstract/settings`
//! and the session in `$XDG_STATE_HOME/abstract/session`, both as `key =
//! value` lines. Everything on disk goes through `write_atomic`.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::editor::TextWidth;
use crate::i18n::LangPref;
use crate::spell::SpellLang;
use crate::theme::ThemePref;

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

/// Write via a hidden sibling temp file + rename so a crash never leaves a
/// truncated target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)?;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.abstract-tmp"));
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
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
        self.kv.get("font").unwrap_or(crate::assets::SANS)
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
            .unwrap_or(1.)
            .clamp(crate::zoom::MIN, crate::zoom::MAX)
    }

    pub fn set_zoom(&mut self, factor: f32) {
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
            .map_or_else(crate::glass::default_enabled, |v| v == "on")
    }

    pub fn set_glass(&mut self, on: bool) {
        self.kv.set("glass", if on { "on" } else { "off" });
    }

    /// `glass_material = blur|acrylic|mica|mica-alt|compositor`; missing or
    /// from-another-OS values resolve to this OS's default.
    pub fn glass_material(&self) -> crate::glass::Material {
        self.kv.get("glass_material").map_or_else(
            crate::glass::Material::default_for_os,
            crate::glass::Material::parse,
        )
    }

    pub fn set_glass_material(&mut self, m: crate::glass::Material) {
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
    pub fn glass_surface(&self, s: crate::glass::Surface) -> Option<u8> {
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

    pub fn set_glass_surface(&mut self, s: crate::glass::Surface, v: Option<u8>) {
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
            .unwrap_or(crate::glass::DEFAULT_TINT_COLOR)
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
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_preserves_unknown_keys() {
        let mut s = Settings {
            kv: KeyVals::parse("theme = dark\nfuture-key = 1 2\ntour = done\n"),
        };
        assert_eq!(s.theme(), ThemePref::Dark);
        assert!(s.tour_done());
        s.set_theme(ThemePref::Light);
        let text = s.kv.serialize();
        let again = Settings {
            kv: KeyVals::parse(&text),
        };
        assert_eq!(again.theme(), ThemePref::Light);
        assert!(again.tour_done());
        assert_eq!(again.kv.get("future-key"), Some("1 2"));
        // One `theme` line only.
        assert_eq!(text.matches("theme").count(), 1);
    }

    #[test]
    fn settings_defaults_on_garbage() {
        let s = Settings {
            kv: KeyVals::parse("not a key value line\n===\ntheme\n"),
        };
        assert_eq!(s.theme(), ThemePref::System);
        assert!(!s.tour_done());
    }

    #[test]
    fn appearance_settings_default_and_roundtrip() {
        let mut s = Settings::default();
        assert!(!s.raw_tables());
        assert_eq!((s.light_theme(), s.dark_theme()), ("abstract", "abstract"));
        s.set_raw_tables(true);
        s.set_light_theme("sepia");
        s.set_dark_theme("nord");
        let again = Settings {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert!(again.raw_tables());
        assert_eq!((again.light_theme(), again.dark_theme()), ("sepia", "nord"));
        s.set_raw_tables(false);
        assert!(!s.raw_tables());
    }

    #[test]
    fn session_roundtrip() {
        let mut s = Session {
            kv: KeyVals::parse(
                "window = maximized 10 20 1100 720\nsidebar = closed\nweird\nnote = /a\tb/c.md\t12\t3.5\nnote = /a\td.md\t0\t0\n",
            ),
        };
        let w = s.window().unwrap();
        assert!(w.maximized && w.w == 1100.);
        assert_eq!(s.sidebar_open(), Some(false));
        let notes = s.notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].rel, PathBuf::from("b/c.md"));
        assert_eq!(notes[0].cursor, 12);
        assert_eq!(notes[0].scroll, 3.5);

        s.set_window(&SessionWindow {
            maximized: false,
            x: 1.,
            y: 2.,
            w: 800.,
            h: 600.,
        });
        s.set_sidebar(true);
        let mut notes = notes;
        notes.push(SessionNote {
            space: PathBuf::from("/tab\tbed"),
            rel: PathBuf::from("x.md"),
            cursor: 0,
            scroll: 0.,
        });
        s.set_notes(&notes);
        let again = Session {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert_eq!(again.window().unwrap().w, 800.);
        assert!(!again.window().unwrap().maximized);
        assert_eq!(again.sidebar_open(), Some(true));
        // Tab in path → skipped on write; the two good ones survive.
        assert_eq!(again.notes().len(), 2);
        assert_eq!(again.kv.get("weird"), None);
        assert!(again.kv.lines.iter().any(|l| l == "weird"));
    }

    #[test]
    fn session_malformed_values() {
        let s = Session {
            kv: KeyVals::parse("window = banana\nsidebar = maybe\nnote = /a\tb.md\n"),
        };
        assert_eq!(s.window(), None);
        assert_eq!(s.sidebar_open(), None);
        assert!(s.notes().is_empty());
    }

    #[test]
    fn session_tabs_roundtrip_and_note_fallback() {
        let space = PathBuf::from("/s");
        let notes = vec![
            SessionNote {
                space: space.clone(),
                rel: PathBuf::from("a.md"),
                cursor: 1,
                scroll: 0.5,
            },
            SessionNote {
                space: space.clone(),
                rel: PathBuf::from("b.md"),
                cursor: 0,
                scroll: 0.,
            },
        ];
        let mut s = Session::default();
        s.set_tabs(&notes);
        s.set_active_tab(Some((&space, Path::new("b.md"))));
        let again = Session {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert_eq!(again.tabs(), notes);
        assert_eq!(again.active_tab(), Some((space.clone(), "b.md".into())));
        // Pre-tab `note` lines restore as a single-tab space.
        let legacy = Session {
            kv: KeyVals::parse("note = /s\tc.md\t3\t1.0\n"),
        };
        let tabs = legacy.tabs();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].rel, PathBuf::from("c.md"));
        assert_eq!(tabs[0].cursor, 3);
        // A `tab` line wins over `note` lines for the same space.
        let mixed = Session {
            kv: KeyVals::parse("note = /s\told.md\t0\t0\ntab = /s\tnew.md\t0\t0\n"),
        };
        let tabs = mixed.tabs();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].rel, PathBuf::from("new.md"));
        // `tab_active` is removed when nothing is open.
        s.set_active_tab(None);
        let again = Session {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert_eq!(again.active_tab(), None);
    }

    #[test]
    fn session_tabs_keep_strip_order_with_active_in_the_middle() {
        let space = PathBuf::from("/s");
        let n = |rel: &str| SessionNote {
            space: space.clone(),
            rel: PathBuf::from(rel),
            cursor: 0,
            scroll: 0.,
        };
        let notes = vec![n("a.md"), n("b.md"), n("c.md")];
        let mut s = Session::default();
        s.set_tabs(&notes);
        s.set_active_tab(Some((&space, Path::new("b.md"))));
        // The serialized `tab` lines keep strip order — active stays put.
        let tab_lines: Vec<&str> =
            s.kv.lines
                .iter()
                .filter(|l| l.starts_with("tab "))
                .map(String::as_str)
                .collect();
        assert_eq!(tab_lines.len(), 3);
        assert!(tab_lines[0].contains("\ta.md\t"));
        assert!(tab_lines[1].contains("\tb.md\t"));
        assert!(tab_lines[2].contains("\tc.md\t"));
        let again = Session {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert_eq!(again.tabs(), notes);
        assert_eq!(again.active_tab(), Some((space, "b.md".into())));
    }

    #[test]
    fn write_atomic_writes_and_overwrites() {
        let dir = std::env::temp_dir().join(format!("abstract-store-test-{}", std::process::id()));
        let file = dir.join("nested").join("out.txt");
        write_atomic(&file, b"one").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one");
        write_atomic(&file, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        assert!(!dir.join("nested").join(".out.txt.abstract-tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_width_setting_defaults_and_roundtrip() {
        let mut s = Settings::default();
        // Missing reads as medium.
        assert_eq!(s.text_width(), TextWidth::Medium);
        for w in TextWidth::ALL {
            s.set_text_width(w);
            let again = Settings {
                kv: KeyVals::parse(&s.kv.serialize()),
            };
            assert_eq!(again.text_width(), w);
            assert_eq!(again.kv.get("text_width"), Some(w.as_str()));
        }
        // Garbage also reads as medium.
        let bad = Settings {
            kv: KeyVals::parse("text_width = banana\n"),
        };
        assert_eq!(bad.text_width(), TextWidth::Medium);
    }

    #[test]
    fn xdg_explicit_var_wins() {
        // Unique var name: env mutation in tests is racy, so only the
        // explicit-var branch is exercised here.
        const VAR: &str = "ABSTRACT_XDG_TEST_VAR_9F3B";
        unsafe { std::env::set_var(VAR, "/tmp/abstract-xdg-wins") };
        assert_eq!(
            xdg(VAR, "fallback"),
            PathBuf::from("/tmp/abstract-xdg-wins")
        );
        unsafe { std::env::remove_var(VAR) };
    }

    #[test]
    fn glass_defaults_and_roundtrip() {
        let mut s = Settings::default();
        // Missing keys read as the per-OS default material, full strength,
        // sidebar+tabs at 75%, everything else opaque, theme tint.
        assert_eq!(s.glass(), crate::glass::default_enabled());
        assert_eq!(s.glass_material(), crate::glass::Material::default_for_os());
        assert_eq!(s.glass_intensity(), 100);
        assert_eq!(s.glass_surface(crate::glass::Surface::Sidebar), Some(75));
        assert_eq!(s.glass_surface(crate::glass::Surface::Tabs), Some(75));
        for surf in crate::glass::Surface::ALL {
            let want = surf.default_percent();
            assert_eq!(s.glass_surface(surf), want, "{surf:?}");
        }
        assert!(!s.glass_tint());
        assert_eq!(s.glass_tint_color(), crate::glass::DEFAULT_TINT_COLOR);
        assert_eq!(s.glass_tint_strength(), 50);
        assert_eq!(s.glass_text_opacity(), 100);
        assert!(!s.glass_text_contrast());

        // Round-trip through serialization.
        s.set_glass(false);
        s.set_glass_material(crate::glass::Material::default_for_os());
        s.set_glass_intensity(42);
        s.set_glass_surface(crate::glass::Surface::Sidebar, Some(60));
        s.set_glass_surface(crate::glass::Surface::Menus, None);
        s.set_glass_tint(true);
        s.set_glass_tint_color(0xff8800);
        s.set_glass_tint_strength(25);
        s.set_glass_text_opacity(80);
        s.set_glass_text_contrast(true);
        let again = Settings {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert!(!again.glass());
        assert_eq!(again.glass_intensity(), 42);
        assert_eq!(
            again.glass_surface(crate::glass::Surface::Sidebar),
            Some(60)
        );
        assert_eq!(again.glass_surface(crate::glass::Surface::Menus), None);
        assert_eq!(again.kv.get("glass_menus"), Some("off"));
        assert!(again.glass_tint());
        assert_eq!(again.glass_tint_color(), 0xff8800);
        assert_eq!(again.kv.get("glass_tint_color"), Some("ff8800"));
        assert_eq!(again.glass_tint_strength(), 25);
        assert_eq!(again.glass_text_opacity(), 80);
        assert!(again.glass_text_contrast());

        // Garbage and out-of-range values fall back to defaults/clamps.
        let bad = Settings {
            kv: KeyVals::parse(
                "glass_intensity = banana\nglass_tint_color = ffffff00\nglass_sidebar = 300\n",
            ),
        };
        assert_eq!(bad.glass_intensity(), 100);
        assert_eq!(bad.glass_tint_color(), crate::glass::DEFAULT_TINT_COLOR);
        // Out-of-range is unparseable, so the surface default wins.
        assert_eq!(bad.glass_surface(crate::glass::Surface::Sidebar), Some(75));
        // A material saved on another OS parses to one valid here.
        let foreign = Settings {
            kv: KeyVals::parse("glass_material = mica\n"),
        };
        assert!(crate::glass::Material::options().contains(&foreign.glass_material()));
    }
}
