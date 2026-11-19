use super::*;
use crate::core::prefs::{self, Preference, Surface};
use rune::{Family, KeyEvent};

pub struct Customization {
    pub open: bool,
    pub group: usize,
    pub editor_family: &'static str,
    pub caret_visible: bool,
    phase: u64,
}

impl Customization {
    pub fn new(settings: &Settings) -> Self {
        Self {
            open: false,
            group: 0,
            editor_family: Box::leak(settings.preference("editor_font").into_boxed_str()),
            caret_visible: true,
            phase: 0,
        }
    }
}

impl State {
    pub fn settings_open(&self) -> bool {
        self.customization.open
    }

    pub fn toggle_settings(&mut self, tx: &mut Tx) {
        self.customization.open = !self.customization.open;
        self.find = None;
        self.search = None;
        if self.customization.open {
            self.focus_mode = false;
            tx.focus("settings-group-0", false);
        } else {
            tx.focus("editor", false);
        }
    }

    pub fn change_preference(&mut self, key: &str, forward: bool, tx: &mut Tx) {
        let Some(p) = prefs::preferences().into_iter().find(|p| p.key == key) else {
            return;
        };
        let value = p.step(Some(&self.settings.preference(key)), forward);
        self.set_preference(p, &value, tx);
    }

    fn set_preference(&mut self, p: Preference, value: &str, tx: &mut Tx) {
        self.settings.kv.set(p.key, &p.value(Some(value)));
        if p.key == "editor_font" {
            self.customization.editor_family = prefs::FONTS
                .iter()
                .copied()
                .find(|v| *v == value)
                .unwrap_or(prefs::SANS);
        }
        if p.key == "ui_font" {
            tx.set_font_families(self.settings.preference("ui_font"), "Noto Sans Mono");
        }
        if p.key == "lang" {
            i18n::set(match self.settings.lang() {
                LangPref::System => i18n::detect(),
                LangPref::En => Lang::En,
                LangPref::PtBr => Lang::PtBr,
            });
        }
        if p.key == "autosave_delay" {
            for doc in self.documents.values_mut() {
                if let SaveState::Pending { due, .. } = &mut doc.save {
                    *due = tx.clock() + self.settings.number("autosave_delay") as f64;
                    tx.wake_at(*due);
                }
            }
        }
        self.save_settings(tx);
    }

    pub fn custom_caret(&self) -> bool {
        self.settings.preference("caret_style") != "bar" || !self.settings.enabled("caret_blink")
    }

    pub fn poll_caret(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.settings_open()
            || !self.custom_caret()
            || !self.settings.enabled("caret_blink")
            || tx.focused() != Some("editor")
        {
            return false;
        }
        let phase = (now * 2.) as u64;
        let changed = phase != self.customization.phase;
        self.customization.phase = phase;
        self.customization.caret_visible = phase.is_multiple_of(2);
        tx.wake_at((phase + 1) as f64 / 2.);
        changed
    }

    pub fn editor_style(&self) -> TextStyle {
        TextStyle::new(self.settings.number("editor_size"), 400.)
            .family(Family::Named(self.customization.editor_family))
            .lh(self.settings.number("editor_size") * self.settings.number("line_height"))
    }
}

pub fn panel_key(s: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    keymap::handle(s, key, tx)
}

pub fn resize_handle(p: theme::Palette) -> El {
    row()
        .key("sidebar-resize")
        .label(t(Key::SidebarWidth))
        .w(4.)
        .h_full()
        .fill(rgb(p.line))
        .hover(rgb(p.callout[0]))
        .on_drag(|s: &mut State, drag, tx| match drag.phase {
            rune::Phase::Down => drag.data = s.settings.number("sidebar_width"),
            rune::Phase::Move => {
                let width = (drag.data + drag.dx).clamp(160., 480.);
                s.settings.kv.set("sidebar_width", &width.to_string());
            }
            rune::Phase::Up => {
                if drag.cancelled() {
                    s.settings.kv.set("sidebar_width", &drag.data.to_string());
                } else {
                    s.save_settings(tx);
                }
            }
        })
}

fn display(value: &str) -> String {
    match value {
        "on" => t(Key::Enabled),
        "off" => t(Key::Disabled),
        "system" => t(Key::ThemeSystem),
        "light" => t(Key::ThemeLight),
        "dark" => t(Key::ThemeDark),
        "bar" => t(Key::CaretBar),
        "block" => t(Key::CaretBlock),
        "underline" => t(Key::CaretUnderline),
        "narrow" => t(Key::TextWidthNarrow),
        "medium" => t(Key::TextWidthMedium),
        "wide" => t(Key::TextWidthWide),
        "theme" => t(Key::GlassTintTheme),
        "custom" => t(Key::GlassTintCustom),
        "blur" => t(Key::GlassMaterialBlur),
        "compositor" => t(Key::GlassMaterialCompositor),
        other => other,
    }
    .into()
}

fn button(label: &str, key: &str, p: theme::Palette) -> El {
    text(label)
        .key(key)
        .role("button")
        .label(label)
        .focusable(true)
        .font(TextStyle::new(13., 500.))
        .padding(7.)
        .color(rgb(p.fg))
        .hover(rgb(p.hover))
        .on_key(panel_key)
}

pub fn view(s: &State, p: theme::Palette) -> El {
    let groups = [Key::Appearance, Key::EditorSection, Key::FilesSection];
    let mut tabs = row().w_full().items_center().gap(8.);
    for (i, group) in groups.iter().enumerate() {
        tabs = tabs.child(
            button(t(*group), &format!("settings-group-{i}"), p)
                .focus_selected(i == s.customization.group)
                .fill(rgb(if i == s.customization.group {
                    p.active
                } else {
                    p.bg
                }))
                .on_press_tx(move |s: &mut State, tx| {
                    s.customization.group = i;
                    tx.scroll_to_y("settings-scroll", 0.);
                }),
        );
    }
    tabs = tabs
        .child(row().grow(1.))
        .child(button(t(Key::Close), "settings-close", p).on_press_tx(State::toggle_settings));
    let mut rows = col().w_full().gap(2.);
    for field in prefs::preferences()
        .into_iter()
        .filter(|f| f.group == s.customization.group)
    {
        let value = s.settings.preference(field.key);
        let label = t(if field.key == "zoom" {
            Key::ZoomLabel
        } else {
            field.label
        })
        .split(':')
        .next()
        .unwrap_or_default()
        .to_owned();
        let mut control = row().items_center().gap(4.);
        if field.key == "accent" || field.key == "glass_tint_color" {
            if let prefs::Input::Color(options) = field.input {
                for option in options {
                    let ink = u32::from_str_radix(option, 16).unwrap_or(p.callout[0]);
                    control = control.child(
                        button(
                            if *option == "system" { "◉" } else { "●" },
                            &format!("setting-{}-{option}", field.key),
                            p,
                        )
                        .label(display(option))
                        .color(rgb(ink))
                        .fill(rgb(if value == *option { p.active } else { p.bg }))
                        .focus_selected(value == *option)
                        .on_press_tx(move |s: &mut State, tx| s.set_preference(field, option, tx)),
                    );
                }
            }
        } else {
            control = control
                .child(
                    button("−", &format!("setting-{}-prev", field.key), p)
                        .label(format!("{label} −"))
                        .on_press_tx(move |s: &mut State, tx| {
                            s.change_preference(field.key, false, tx)
                        }),
                )
                .child(
                    text(if field.key == "zoom" {
                        format!("{:.0}%", s.settings.zoom() * 100.)
                    } else {
                        display(&value)
                    })
                    .font(TextStyle::new(13., 400.))
                    .color(rgb(p.fg))
                    .min_w(100.),
                )
                .child(
                    button("+", &format!("setting-{}-next", field.key), p)
                        .label(format!("{label} +"))
                        .on_press_tx(move |s: &mut State, tx| {
                            s.change_preference(field.key, true, tx)
                        }),
                );
        }
        rows = rows.child(
            row()
                .w_full()
                .items_center()
                .justify_between()
                .padding(3.)
                .child(
                    text(label)
                        .font(TextStyle::new(13., 400.))
                        .color(rgb(p.body)),
                )
                .child(control),
        );
    }
    surface(
        col()
            .key("settings")
            .role("region")
            .label(t(Key::Settings))
            .w_full()
            .grow(1.)
            .min_h(0.)
            .padding(24.)
            .gap(16.)
            .child(
                text(t(Key::Settings))
                    .font(TextStyle::new(21., 600.))
                    .color(rgb(p.head)),
            )
            .child(tabs)
            .child(
                col()
                    .key("settings-scroll")
                    .w_full()
                    .grow(1.)
                    .min_h(0.)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(rows),
            ),
        &s.settings,
        Surface::Panel,
        p.bg,
    )
}

pub fn surface(el: El, settings: &Settings, surface: Surface, color: u32) -> El {
    let Some(percent) = settings.glass_surface(surface).filter(|_| settings.glass()) else {
        return el.fill(rgb(color));
    };
    let mut alpha = percent as f32 / 100. * settings.glass_intensity() as f32 / 100.;
    if surface == Surface::Menus {
        alpha = alpha.max(0.88);
    }
    let color = if settings.glass_tint() {
        let t = settings.glass_tint_strength() as f32 / 100.;
        let custom = settings.glass_tint_color();
        let channel = |shift: u32| {
            ((((color >> shift) & 255u32) as f32 * (1. - t)
                + ((custom >> shift) & 255u32) as f32 * t) as u32)
                << shift
        };
        channel(16) | channel(8) | channel(0)
    } else {
        color
    };
    let el = el.fill(rgb(color).alpha(alpha));
    #[cfg(target_os = "macos")]
    {
        let wrapper = match surface {
            Surface::Sidebar => row().w(settings.number("sidebar_width")).h_full(),
            Surface::Tabs => row().w_full().h(34.),
            Surface::Toolbar => row().w_full().h(30.),
            _ => row().w_full().grow(1.).min_h(0.),
        };
        wrapper
            .fill(rgb(color))
            .system_material(rune::SystemMaterial::Popover)
            .child(el)
    }
    #[cfg(not(target_os = "macos"))]
    el.fill(rgb(color))
}

pub fn caret(s: &State, cx: &Cx<State>, p: theme::Palette) -> Option<El> {
    if !s.custom_caret()
        || s.settings_open()
        || (s.settings.enabled("caret_blink") && !s.customization.caret_visible)
    {
        return None;
    }
    if s.document().is_none_or(|d| d.edit.anchor != d.edit.focus) {
        return None;
    }
    let [x, y, _, h] = cx.caret("editor")?;
    let [left, top, width, height] = cx.layout("editor-scroll")?;
    if x < left || x >= left + width || y < top || y + h > top + height {
        return None;
    }
    let size = s.settings.number("editor_size");
    let (w, height, dy, alpha) = match s.settings.preference("caret_style").as_str() {
        "block" => (size * 0.6, h, 0., 0.35),
        "underline" => (size * 0.6, 2., h - 2., 1.),
        _ => (1.5, h, 0., 1.),
    };
    Some(
        row()
            .key("custom-caret")
            .w(w)
            .h(height)
            .absolute(x, y + dy)
            .fill(rgb(p.caret).alpha(alpha))
            .inert(),
    )
}
