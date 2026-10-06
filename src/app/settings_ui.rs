use super::*;

use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState, SliderValue};

const REPO: &str = env!("CARGO_PKG_REPOSITORY");
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Which glass setting a slider writes.
#[derive(Clone, Copy)]
enum GlassSlider {
    Intensity,
    Surface(Surface),
    TintStrength,
    TextOpacity,
}

/// Slider and colour-picker entities behind the Glass settings. Created
/// once so thumb positions and subscriptions survive across renders.
pub(crate) struct GlassControls {
    pub intensity: Entity<SliderState>,
    pub surfaces: [Entity<SliderState>; 6],
    pub tint_strength: Entity<SliderState>,
    pub text_opacity: Entity<SliderState>,
    pub tint_color: Entity<ColorPickerState>,
    _subs: Vec<Subscription>,
}

impl GlassControls {
    pub fn new(settings: &Settings, window: &mut Window, cx: &mut Context<AbstractApp>) -> Self {
        let mut slider = |v: u8| {
            cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(100.)
                    .step(1.)
                    .default_value(f32::from(v))
            })
        };
        let mut ctl = GlassControls {
            intensity: slider(settings.glass_intensity()),
            surfaces: Surface::ALL.map(|s| {
                slider(
                    settings
                        .glass_surface(s)
                        .or_else(|| s.default_percent())
                        .unwrap_or(60),
                )
            }),
            tint_strength: slider(settings.glass_tint_strength()),
            text_opacity: slider(settings.glass_text_opacity()),
            tint_color: cx.new(|cx| {
                ColorPickerState::new(window, cx).default_value(rgb(settings.glass_tint_color()))
            }),
            _subs: Vec::new(),
        };
        for s in Surface::ALL {
            let key = GlassSlider::Surface(s);
            ctl._subs.push(
                cx.subscribe(ctl.surface(s), move |this, _, ev: &SliderEvent, cx| {
                    this.glass_slider(key, ev, cx)
                }),
            );
        }
        for (entity, key) in [
            (&ctl.intensity, GlassSlider::Intensity),
            (&ctl.tint_strength, GlassSlider::TintStrength),
            (&ctl.text_opacity, GlassSlider::TextOpacity),
        ] {
            ctl._subs
                .push(cx.subscribe(entity, move |this, _, ev: &SliderEvent, cx| {
                    this.glass_slider(key, ev, cx)
                }));
        }
        ctl._subs.push(
            cx.subscribe(&ctl.tint_color, |this, _, ev: &ColorPickerEvent, cx| {
                this.glass_tint_changed(ev, cx)
            }),
        );
        ctl
    }

    fn surface(&self, s: Surface) -> &Entity<SliderState> {
        &self.surfaces[s as usize]
    }
}

impl AbstractApp {
    pub(crate) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.spaces_open = false;
            self.status_open = false;
            self.settings_focus.focus(window, cx);
        } else {
            self.editor.update(cx, |ed, cx| ed.focus(window, cx));
        }
        cx.notify();
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.toggle_settings(window, cx);
        }
    }

    fn set_raw_tables(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_raw_tables(on);
        for tab in &self.tabs {
            tab.editor.update(cx, |ed, cx| ed.set_raw_tables(on, cx));
        }
        // The blank editor shown when nothing is open gets it too.
        if self.tabs.is_empty() {
            self.editor.update(cx, |ed, cx| ed.set_raw_tables(on, cx));
        }
        self.save_settings(window, cx);
    }

    // ── Glass ─────────────────────────────────────────────────────────────

    fn set_glass(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_glass(on);
        self.save_settings(window, cx);
    }

    fn set_glass_material(
        &mut self,
        m: glass::Material,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.set_glass_material(m);
        self.save_settings(window, cx);
    }

    fn set_glass_tint(&mut self, custom: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_glass_tint(custom);
        self.save_settings(window, cx);
    }

    fn set_glass_surface_on(
        &mut self,
        s: Surface,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Re-enabling restores the slider's last position.
        let v = on.then(|| {
            self.glass_controls
                .surface(s)
                .read(cx)
                .value()
                .start()
                .round()
                .clamp(0., 100.) as u8
        });
        self.settings.set_glass_surface(s, v);
        self.save_settings(window, cx);
    }

    fn set_glass_text_contrast(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_glass_text_contrast(on);
        self.save_settings(window, cx);
    }

    /// A glass slider moved: apply live on `Change`, persist on `Release`.
    fn glass_slider(&mut self, key: GlassSlider, ev: &SliderEvent, cx: &mut Context<Self>) {
        let (v, commit) = match ev {
            SliderEvent::Change(SliderValue::Single(v)) => (*v, false),
            SliderEvent::Release(SliderValue::Single(v)) => (*v, true),
            _ => return,
        };
        let v = v.round().clamp(0., 100.) as u8;
        match key {
            GlassSlider::Intensity => self.settings.set_glass_intensity(v),
            GlassSlider::Surface(s) => self.settings.set_glass_surface(s, Some(v)),
            GlassSlider::TintStrength => self.settings.set_glass_tint_strength(v),
            GlassSlider::TextOpacity => self.settings.set_glass_text_opacity(v),
        }
        glass::apply(&self.settings, cx);
        if commit {
            let s = self.settings.clone();
            cx.background_spawn(async move { s.save() }).detach();
        }
        cx.notify();
    }

    fn glass_tint_changed(&mut self, ev: &ColorPickerEvent, cx: &mut Context<Self>) {
        let ColorPickerEvent::Change(Some(c)) = ev else {
            return;
        };
        // `u32::from(Rgba)` packs RRGGBBAA; settings store RRGGBB.
        self.settings
            .set_glass_tint_color((u32::from(c.to_rgb()) >> 8) & 0xff_ff_ff);
        glass::apply(&self.settings, cx);
        let s = self.settings.clone();
        cx.background_spawn(async move { s.save() }).detach();
        cx.notify();
    }

    fn set_discord(&mut self, on: bool, cx: &mut Context<Self>) {
        self.settings.set_discord(on);
        self.presence.set_enabled(on);
        if on {
            let title = self
                .current
                .as_ref()
                .map(|_| title_of(self.editor.read(cx).text()).to_string());
            self.presence.set(title, spaces::name_of(&self.dir));
        }
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    fn pick_palette(
        &mut self,
        dark: bool,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if dark {
            self.settings.set_dark_theme(id);
        } else {
            self.settings.set_light_theme(id);
        }
        self.save_settings(window, cx);
    }

    /// Sidebar footer: opens the settings dialog.
    pub(crate) fn render_settings_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pal = cx.palette();
        let tip = SharedString::from(tf(Key::SettingsTip, &[]));
        div()
            .flex_none()
            .p(z(8.))
            .border_t_1()
            .border_color(rgb(pal.line))
            .child(hover_bg(
                div()
                    .id("settings-open")
                    .role(Role::Button)
                    .aria_label(tip.clone())
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
                    .h(z(30.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(13.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_settings(window, cx)))
                    .child(icon("icons/settings.svg", pal.dim).size(z(15.)))
                    .child(t(Key::Settings)),
                "settings-open",
                self.settings_open.then_some(pal.active),
                pal.hover,
                window,
                cx,
            ))
    }

    pub(crate) fn render_settings(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pal = cx.palette();
        let section = |label: &'static str| {
            div()
                .pt(z(14.))
                .pb(z(6.))
                .text_size(z(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(pal.faint))
                .child(label)
        };
        let sub = |label: &'static str| {
            div()
                .pt(z(10.))
                .pb(z(6.))
                .text_size(z(12.))
                .text_color(rgb(pal.dim))
                .child(label)
        };

        let modes = div().flex().gap(z(4.)).children(
            [ThemePref::System, ThemePref::Light, ThemePref::Dark].map(|pref| {
                let on = self.theme_pref == pref;
                hover_bg(
                    div()
                        .id(SharedString::from(format!("theme-mode-{}", pref.as_str())))
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(z(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(z(6.))
                        .rounded(z(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(z(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_theme_pref(pref, window, cx)
                        }))
                        .child(icon(pref.icon(), if on { pal.fg } else { pal.dim }).size(z(14.)))
                        .child(t(match pref {
                            ThemePref::System => Key::ThemeSystem,
                            ThemePref::Light => Key::ThemeLight,
                            ThemePref::Dark => Key::ThemeDark,
                        })),
                    SharedString::from(format!("theme-mode-{}", pref.as_str())),
                    on.then_some(pal.active),
                    pal.hover,
                    window,
                    cx,
                )
            }),
        );

        let swatches = |dark: bool, window: &mut Window, cx: &mut Context<Self>| {
            let (list, current) = if dark {
                (&theme::DARKS, self.settings.dark_theme())
            } else {
                (&theme::LIGHTS, self.settings.light_theme())
            };
            let current = theme::named(list, current).id;
            div().flex().gap(z(8.)).children(list.iter().map(|n| {
                let on = n.id == current;
                let p = n.palette;
                let id = n.id;
                hover_bg(
                    div()
                        .id(SharedString::from(format!(
                            "{}-theme-{id}",
                            if dark { "dark" } else { "light" }
                        )))
                        .role(Role::RadioButton)
                        .aria_label(n.name)
                        .aria_selected(on)
                        .flex_1()
                        .min_w_0()
                        .p(z(4.))
                        .flex()
                        .flex_col()
                        .gap(z(6.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pick_palette(dark, id, window, cx)
                        }))
                        .child(
                            div()
                                .h(z(46.))
                                .p(z(7.))
                                .flex()
                                .flex_col()
                                .gap(z(4.))
                                .rounded(z(4.))
                                .bg(rgb(p.bg))
                                .border_1()
                                .border_color(rgb(p.line))
                                .child(div().w(z(34.)).h(z(5.)).rounded(z(2.)).bg(rgb(p.head)))
                                .child(div().w_full().h(z(3.)).rounded(z(2.)).bg(rgb(p.dim)))
                                .child(
                                    div()
                                        .flex()
                                        .gap(z(3.))
                                        .child(
                                            div().w(z(18.)).h(z(3.)).rounded(z(2.)).bg(rgb(p.dim)),
                                        )
                                        .child(
                                            div()
                                                .w(z(12.))
                                                .h(z(3.))
                                                .rounded(z(2.))
                                                .bg(rgb(p.callout[0])),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .px(z(2.))
                                .pb(z(2.))
                                .truncate()
                                .text_size(z(12.))
                                .text_color(rgb(if on { pal.fg } else { pal.body }))
                                .child(n.name),
                        ),
                    SharedString::from(format!(
                        "{}-theme-{id}",
                        if dark { "dark" } else { "light" }
                    )),
                    None,
                    pal.hover,
                    window,
                    cx,
                )
            }))
        };

        let toggle = |id: &'static str,
                      label: &'static str,
                      hint: Option<&'static str>,
                      on: bool,
                      window: &mut Window,
                      cx: &mut App| {
            hover_bg(
                div()
                    .id(id)
                    .role(Role::Switch)
                    .aria_label(label)
                    .aria_toggled(on.into())
                    .px(z(8.))
                    .py(z(7.))
                    .mx(z(-8.))
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .active(|s| s.bg(rgb(pal.active)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(z(2.))
                            .child(
                                div()
                                    .text_size(z(13.))
                                    .text_color(rgb(pal.body))
                                    .child(label),
                            )
                            .when_some(hint, |el, h| {
                                el.child(
                                    div()
                                        .text_size(z(12.))
                                        .line_height(z(17.))
                                        .text_color(rgb(pal.dim))
                                        .child(h),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(z(30.))
                            .h(z(18.))
                            .p(z(2.))
                            .flex()
                            .items_center()
                            .when(on, |s| s.justify_end())
                            .rounded_full()
                            .bg(rgb(if on { pal.fg } else { pal.active }))
                            .child(div().size(z(14.)).rounded_full().bg(rgb(if on {
                                pal.bg
                            } else {
                                pal.dim
                            }))),
                    ),
                id,
                None,
                pal.hover,
                window,
                cx,
            )
        };

        let link = |id: &'static str,
                    glyph: &'static str,
                    label: &'static str,
                    url: String,
                    window: &mut Window,
                    cx: &mut App| {
            hover_bg(
                div()
                    .id(id)
                    .role(Role::Link)
                    .aria_label(label)
                    .h(z(28.))
                    .px(z(8.))
                    .flex()
                    .items_center()
                    .gap(z(6.))
                    .rounded(z(6.))
                    .border_1()
                    .border_color(rgb(pal.line))
                    .cursor_pointer()
                    .text_size(z(12.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .child(icon(glyph, pal.dim).size(z(13.)))
                    .child(label),
                id,
                None,
                pal.hover,
                window,
                cx,
            )
        };

        let raw = self.settings.raw_tables();
        let updates = self.settings.updates();
        let smart_quotes = self.settings.smart_quotes();
        let font_names = cx.text_system().all_font_names();
        let current_font = fonts::resolve(self.settings.font(), &font_names);
        let font_rows =
            div()
                .flex()
                .flex_col()
                .children(fonts::choices(&font_names).into_iter().map(|family| {
                    let on = current_font.eq_ignore_ascii_case(family);
                    let label: SharedString = if family == fonts::SYSTEM {
                        t(Key::ThemeSystem).into()
                    } else {
                        family.into()
                    };
                    hover_bg(
                        div()
                            .id(SharedString::from(format!("font-{family}")))
                            .role(Role::RadioButton)
                            .aria_label(label.clone())
                            .aria_selected(on)
                            .px(z(8.))
                            .py(z(6.))
                            .mx(z(-8.))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .rounded(z(6.))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_font(family, window, cx)
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(family)
                                    .text_size(z(13.))
                                    .text_color(rgb(if on { pal.fg } else { pal.body }))
                                    .child(label),
                            )
                            .when(on, |s| {
                                s.child(icon("icons/check.svg", pal.dim).size(z(14.)))
                            }),
                        SharedString::from(format!("font-{family}")),
                        None,
                        pal.hover,
                        window,
                        cx,
                    )
                }));
        let widths = div()
            .flex()
            .gap(z(4.))
            .children(TextWidth::ALL.map(|width| {
                let on = self.settings.text_width() == width;
                hover_bg(
                    div()
                        .id(SharedString::from(format!("text-width-{}", width.as_str())))
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(z(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(z(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_text_width(width, window, cx)
                        }))
                        .child(t(match width {
                            TextWidth::Narrow => Key::TextWidthNarrow,
                            TextWidth::Medium => Key::TextWidthMedium,
                            TextWidth::Wide => Key::TextWidthWide,
                        })),
                    SharedString::from(format!("text-width-{}", width.as_str())),
                    on.then_some(pal.active),
                    pal.hover,
                    window,
                    cx,
                )
            }));
        let discord = self.settings.discord();
        let spell_on = self.settings.spellcheck();

        // ── Glass section ───────────────────────────────────────────────
        let glass_on = self.settings.glass();
        let glass_mat = self.settings.glass_material();
        let tint_custom = self.settings.glass_tint();
        let ctl = &self.glass_controls;
        let mat_label = |m: glass::Material| match m {
            glass::Material::Blur => Key::GlassMaterialBlur,
            glass::Material::Acrylic => Key::GlassMaterialAcrylic,
            glass::Material::Mica => Key::GlassMaterialMica,
            glass::Material::MicaAlt => Key::GlassMaterialMicaAlt,
            glass::Material::Compositor => Key::GlassMaterialCompositor,
        };
        let surf_label = |s: Surface| match s {
            Surface::Sidebar => Key::GlassSidebar,
            Surface::Tabs => Key::GlassTabs,
            Surface::Toolbar => Key::GlassToolbar,
            Surface::Menus => Key::GlassMenus,
            Surface::Panel => Key::GlassPanel,
            Surface::Editor => Key::GlassEditor,
        };
        let switch_dot = |on: bool| {
            div()
                .flex_none()
                .w(z(30.))
                .h(z(18.))
                .p(z(2.))
                .flex()
                .items_center()
                .when(on, |s| s.justify_end())
                .rounded_full()
                .bg(rgb(if on { pal.fg } else { pal.active }))
                .child(
                    div()
                        .size(z(14.))
                        .rounded_full()
                        .bg(rgb(if on { pal.bg } else { pal.dim })),
                )
        };
        let slider_pct = |state: &Entity<SliderState>, cx: &App| {
            state.read(cx).value().start().round().clamp(0., 100.) as u32
        };
        let slider_row = |id: &'static str,
                          label: SharedString,
                          state: &Entity<SliderState>,
                          disabled: bool,
                          cx: &mut App| {
            div()
                .px(z(8.))
                .py(z(8.))
                .mx(z(-8.))
                .id(id)
                .flex()
                .items_center()
                .gap(z(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(z(13.))
                        .text_color(rgb(pal.body))
                        .child(label),
                )
                .child(
                    Slider::new(state)
                        .horizontal()
                        .disabled(disabled)
                        .w(z(110.))
                        .bg(rgb(pal.fg))
                        .text_color(rgb(pal.fg)),
                )
                .child(
                    div()
                        .w(z(30.))
                        .flex()
                        .justify_end()
                        .text_size(z(12.))
                        .text_color(rgb(pal.dim))
                        .child(format!("{}%", slider_pct(state, cx))),
                )
        };
        let surface_row = |s: Surface, window: &mut Window, cx: &mut Context<Self>| {
            let on = self.settings.glass_surface(s).is_some();
            let st = ctl.surface(s).clone();
            let id = SharedString::from(format!("glass-surf-{}", s.key()));
            div()
                .px(z(4.))
                .py(z(3.))
                .mx(z(-4.))
                .flex()
                .items_center()
                .gap(z(6.))
                .child(
                    hover_bg(
                        div()
                            .id(id.clone())
                            .role(Role::Switch)
                            .aria_label(t(surf_label(s)))
                            .aria_toggled(on.into())
                            .pl(z(4.))
                            .pr(z(6.))
                            .py(z(4.))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .rounded(z(6.))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_glass_surface_on(s, !on, window, cx)
                            }))
                            .child(switch_dot(on))
                            .child(
                                div()
                                    .text_size(z(13.))
                                    .text_color(rgb(pal.body))
                                    .child(t(surf_label(s))),
                            ),
                        id,
                        None,
                        pal.hover,
                        window,
                        cx,
                    )
                    .flex_1()
                    .min_w_0(),
                )
                .child(
                    Slider::new(&st)
                        .horizontal()
                        .disabled(!on)
                        .w(z(96.))
                        .bg(rgb(pal.fg))
                        .text_color(rgb(pal.fg)),
                )
                .child(
                    div()
                        .w(z(30.))
                        .flex()
                        .justify_end()
                        .text_size(z(12.))
                        .text_color(rgb(pal.dim))
                        .child(format!("{}%", slider_pct(&st, cx))),
                )
        };
        let material_row = div()
            .flex()
            .gap(z(4.))
            .children(glass::Material::options().iter().map(|m| {
                let m = *m;
                let on = glass_mat == m;
                let id = SharedString::from(format!("glass-mat-{}", m.as_str()));
                hover_bg(
                    div()
                        .id(id.clone())
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(z(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(z(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_glass_material(m, window, cx)
                        }))
                        .child(t(mat_label(m))),
                    id,
                    on.then_some(pal.active),
                    pal.hover,
                    window,
                    cx,
                )
            }));
        // Linux-only footnote under Material.
        let glass_hint = if cfg!(target_os = "linux") {
            Some(
                div()
                    .px(z(8.))
                    .py(z(5.))
                    .mx(z(-8.))
                    .text_size(z(12.))
                    .line_height(z(17.))
                    .text_color(rgb(pal.dim))
                    .child(t(if glass::is_gnome() {
                        Key::GlassNoBlurHint
                    } else {
                        Key::GlassCompositorHint
                    })),
            )
        } else {
            None
        };
        let tint_row = div().flex().gap(z(4.)).children(
            [(false, Key::GlassTintTheme), (true, Key::GlassTintCustom)].map(|(custom, key)| {
                let on = tint_custom == custom;
                let id = SharedString::from(format!("glass-tint-{custom}"));
                hover_bg(
                    div()
                        .id(id.clone())
                        .role(Role::RadioButton)
                        .aria_selected(on)
                        .flex_1()
                        .h(z(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(6.))
                        .border_1()
                        .border_color(rgb(if on { pal.fg } else { pal.line }))
                        .cursor_pointer()
                        .text_size(z(12.))
                        .text_color(rgb(if on { pal.fg } else { pal.body }))
                        .active(|s| s.bg(rgb(pal.active)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_glass_tint(custom, window, cx)
                        }))
                        .child(t(key)),
                    id,
                    on.then_some(pal.active),
                    pal.hover,
                    window,
                    cx,
                )
            }),
        );
        let adv_ps = presence("glass-adv", self.glass_adv, window, cx);
        let contrast_on = self.settings.glass_text_contrast();

        let body = div()
            .id("settings-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(z(20.))
            .pb(z(20.))
            .child(section(t(Key::Appearance)))
            .child(modes)
            .child(sub(t(Key::LightTheme)))
            .child(swatches(false, window, cx))
            .child(sub(t(Key::DarkTheme)))
            .child(swatches(true, window, cx))
            .child(sub(t(Key::GlassSection)))
            .child(
                toggle(
                    "glass-toggle",
                    t(Key::GlassEnable),
                    Some(t(Key::GlassHint)),
                    glass_on,
                    window,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_glass(!glass_on, window, cx)),
                ),
            )
            .when(glass_on, |el| {
                el.child(sub(t(Key::GlassMaterial)))
                    .child(material_row)
                    .when_some(glass_hint, |el, h| el.child(h))
                    .child(slider_row(
                        "glass-intensity",
                        t(Key::GlassIntensity).into(),
                        &ctl.intensity,
                        false,
                        cx,
                    ))
                    .child(hover_bg(
                        div()
                            .id("glass-adv")
                            .role(Role::Button)
                            .aria_label(t(Key::GlassAdvanced))
                            .aria_expanded(self.glass_adv)
                            .px(z(8.))
                            .py(z(7.))
                            .mx(z(-8.))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .rounded(z(6.))
                            .cursor_pointer()
                            .active(|s| s.bg(rgb(pal.active)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.glass_adv = !this.glass_adv;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(z(13.))
                                    .text_color(rgb(pal.body))
                                    .child(t(Key::GlassAdvanced)),
                            )
                            .child(
                                icon(
                                    if self.glass_adv {
                                        "icons/chevron-down.svg"
                                    } else {
                                        "icons/chevron-right.svg"
                                    },
                                    pal.faint,
                                )
                                .size(z(13.)),
                            ),
                        "glass-adv",
                        None,
                        pal.hover,
                        window,
                        cx,
                    ))
                    .when(adv_ps.should_render(), |el| {
                        el.child(
                            div()
                                .opacity(adv_ps.progress)
                                .children(Surface::ALL.map(|s| surface_row(s, window, cx)))
                                .child(sub(t(Key::GlassTint)))
                                .child(tint_row)
                                .when(tint_custom, |el| {
                                    el.child(
                                        div()
                                            .px(z(8.))
                                            .py(z(6.))
                                            .mx(z(-8.))
                                            .flex()
                                            .items_center()
                                            .gap(z(10.))
                                            .child(ColorPicker::new(&ctl.tint_color))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_size(z(13.))
                                                    .text_color(rgb(pal.body))
                                                    .child(t(Key::GlassTintStrength)),
                                            )
                                            .child(
                                                Slider::new(&ctl.tint_strength)
                                                    .horizontal()
                                                    .w(z(80.))
                                                    .bg(rgb(pal.fg))
                                                    .text_color(rgb(pal.fg)),
                                            )
                                            .child(
                                                div()
                                                    .w(z(30.))
                                                    .flex()
                                                    .justify_end()
                                                    .text_size(z(12.))
                                                    .text_color(rgb(pal.dim))
                                                    .child(format!(
                                                        "{}%",
                                                        slider_pct(&ctl.tint_strength, cx)
                                                    )),
                                            ),
                                    )
                                })
                                .child(slider_row(
                                    "glass-text-opacity",
                                    t(Key::GlassTextOpacity).into(),
                                    &ctl.text_opacity,
                                    false,
                                    cx,
                                ))
                                .child(
                                    toggle(
                                        "glass-text-contrast",
                                        t(Key::GlassTextContrast),
                                        None,
                                        contrast_on,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.set_glass_text_contrast(!contrast_on, window, cx)
                                        },
                                    )),
                                ),
                        )
                    })
            })
            .child(section(t(Key::EditorSection)))
            .child(
                toggle(
                    "raw-tables",
                    t(Key::RawTables),
                    Some(t(Key::RawTablesHint)),
                    raw,
                    window,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_raw_tables(!raw, window, cx)),
                ),
            )
            .child(sub(t(Key::TextWidth)))
            .child(widths)
            .child(
                toggle(
                    "smart-quotes",
                    t(Key::SmartQuotes),
                    Some(t(Key::SmartQuotesHint)),
                    smart_quotes,
                    window,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_smart_quotes(!smart_quotes, window, cx)
                })),
            )
            .child(section(t(Key::General)))
            .child(hover_bg(
                div()
                    .id("lang-cycle")
                    .role(Role::Button)
                    .aria_label(t(Key::Language))
                    .px(z(8.))
                    .py(z(7.))
                    .mx(z(-8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(13.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_lang(cx)))
                    .child(div().flex_1().child(t(Key::Language)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .text_size(z(12.))
                            .text_color(rgb(pal.dim))
                            .child(self.lang_label())
                            .child(icon("icons/chevrons.svg", pal.faint).size(z(13.))),
                    ),
                "lang-cycle",
                None,
                pal.hover,
                window,
                cx,
            ))
            .child(
                toggle(
                    "updates-toggle",
                    t(Key::CheckUpdates),
                    None,
                    updates,
                    window,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_updates(!updates, cx))),
            )
            .child(sub(t(Key::Font)))
            .child(
                div()
                    .pb(z(4.))
                    .text_size(z(12.))
                    .line_height(z(17.))
                    .text_color(rgb(pal.dim))
                    .child(t(Key::FontHint)),
            )
            .child(font_rows)
            .child(
                toggle(
                    "discord-presence",
                    t(Key::DiscordPresence),
                    Some(t(Key::DiscordPresenceHint)),
                    discord,
                    window,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_discord(!discord, cx))),
            )
            .child(
                toggle(
                    "spellcheck-toggle",
                    t(Key::Spellcheck),
                    Some(t(Key::SpellcheckHint)),
                    spell_on,
                    window,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_spellcheck(!spell_on, cx))),
            )
            .child(hover_bg(
                div()
                    .id("spell-lang-cycle")
                    .role(Role::Button)
                    .aria_label(t(Key::SpellLang))
                    .px(z(8.))
                    .py(z(7.))
                    .mx(z(-8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .rounded(z(6.))
                    .cursor_pointer()
                    .text_size(z(13.))
                    .text_color(rgb(pal.body))
                    .active(|s| s.bg(rgb(pal.active)))
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_spell_lang(cx)))
                    .child(div().flex_1().child(t(Key::SpellLang)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .text_size(z(12.))
                            .text_color(rgb(pal.dim))
                            .child(self.spell_lang_label())
                            .child(icon("icons/chevrons.svg", pal.faint).size(z(13.))),
                    ),
                "spell-lang-cycle",
                None,
                pal.hover,
                window,
                cx,
            ))
            .child(section(t(Key::About)))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(z(8.))
                    .child(
                        div()
                            .text_size(z(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(pal.fg))
                            .child("Abstract"),
                    )
                    .child(
                        div()
                            .text_size(z(12.))
                            .text_color(rgb(pal.dim))
                            .child(tf(Key::Version, &[("v", VERSION)])),
                    ),
            )
            .child(
                div()
                    .pt(z(4.))
                    .text_size(z(12.))
                    .line_height(z(18.))
                    .text_color(rgb(pal.dim))
                    .child(t(Key::AboutTagline)),
            )
            .child(
                div()
                    .pt(z(12.))
                    .flex()
                    .flex_wrap()
                    .gap(z(6.))
                    .child(link(
                        "about-source",
                        "icons/github.svg",
                        t(Key::SourceCode),
                        REPO.to_string(),
                        window,
                        cx,
                    ))
                    .child(link(
                        "about-release",
                        "icons/external-link.svg",
                        t(Key::ReleaseNotes),
                        format!("{REPO}/releases/tag/v{VERSION}"),
                        window,
                        cx,
                    ))
                    .child(link(
                        "about-issue",
                        "icons/external-link.svg",
                        t(Key::ReportIssue),
                        format!("{REPO}/issues/new"),
                        window,
                        cx,
                    ))
                    .child(link(
                        "about-license",
                        "icons/external-link.svg",
                        t(Key::License),
                        format!("{REPO}/blob/main/LICENSE"),
                        window,
                        cx,
                    )),
            );

        let card = div()
            .id("settings")
            .role(Role::Dialog)
            .aria_label(t(Key::Settings))
            .track_focus(&self.settings_focus)
            .w(z(480.))
            .max_h(relative(0.86))
            .flex()
            .flex_col()
            .bg(glass::bg(pal.menu_bg, Surface::Panel, cx))
            .border_1()
            .border_color(rgb(pal.menu_border))
            .rounded(z(8.))
            .shadow_lg()
            .occlude()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.close_settings(window, cx);
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_settings(window, cx)))
            .child(
                div()
                    .flex_none()
                    .h(z(48.))
                    .pl(z(20.))
                    .pr(z(10.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(pal.line))
                    .child(
                        div()
                            .flex_1()
                            .text_size(z(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(pal.fg))
                            .child(t(Key::Settings)),
                    )
                    .child(
                        icon_btn(
                            "settings-close",
                            "icons/close.svg",
                            t(Key::Close).into(),
                            false,
                            &pal,
                            window,
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.close_settings(window, cx)),
                        ),
                    ),
            )
            .child(body);

        let ps = presence("settings", self.settings_open, window, cx);
        if !ps.should_render() {
            return div().into_any_element();
        }
        div()
            .id("settings-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x0000_0059))
            .opacity(ps.progress)
            .occlude()
            .child(card)
            .into_any_element()
    }
    /// `font` picker in General: applies the family live, then persists it.
    fn set_font(&mut self, family: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_font(family);
        fonts::apply(&self.settings, cx);
        self.save_settings(window, cx);
    }

    /// Spellcheck toggle in General: applies to the editor live, then persists.
    fn set_spellcheck(&mut self, on: bool, cx: &mut Context<Self>) {
        self.settings.set_spellcheck(on);
        let lang = self.settings.spell_lang();
        for editor in self.editors() {
            editor.update(cx, |ed, cx| ed.set_spell(on, lang, cx));
        }
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    /// Language row in General: Automático → English → Português → Ambos.
    fn cycle_spell_lang(&mut self, cx: &mut Context<Self>) {
        let lang = self.settings.spell_lang().next();
        self.settings.set_spell_lang(lang);
        let on = self.settings.spellcheck();
        for editor in self.editors() {
            editor.update(cx, |ed, cx| ed.set_spell(on, lang, cx));
        }
        let settings = self.settings.clone();
        cx.background_spawn(async move { settings.save() }).detach();
        cx.notify();
    }

    pub(crate) fn spell_lang_label(&self) -> &'static str {
        match self.settings.spell_lang() {
            crate::spell::SpellLang::Auto => t(Key::SpellLangAuto),
            crate::spell::SpellLang::En => "English",
            crate::spell::SpellLang::PtBr => "Português (Brasil)",
            crate::spell::SpellLang::Both => t(Key::SpellLangBoth),
        }
    }

    /// `text_width` picker in Editor: applies the column cap live, then
    /// persists it.
    fn set_text_width(&mut self, width: TextWidth, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_text_width(width);
        for editor in self.editors() {
            self.apply_text_width(&editor, cx);
        }
        self.save_settings(window, cx);
    }

    /// Pushes the `text_width` column cap onto one `LiveEditor` — called at
    /// startup and whenever the setting changes; each new editor needs it.
    pub(crate) fn apply_text_width(&self, editor: &Entity<LiveEditor>, cx: &mut App) {
        let col = self.settings.text_width().max_col();
        editor.update(cx, |ed, cx| ed.set_max_col(col, cx));
    }

    /// Every live editor: one per tab, or the blank one when nothing is open.
    pub(crate) fn editors(&self) -> Vec<Entity<LiveEditor>> {
        if self.tabs.is_empty() {
            vec![self.editor.clone()]
        } else {
            self.tabs.iter().map(|t| t.editor.clone()).collect()
        }
    }

    /// Pushes every per-editor setting onto a fresh `LiveEditor`.
    pub(crate) fn configure_editor(&self, editor: &Entity<LiveEditor>, cx: &mut App) {
        let s = &self.settings;
        let (raw, spell, lang, smart) = (
            s.raw_tables(),
            s.spellcheck(),
            s.spell_lang(),
            s.smart_quotes(),
        );
        let focus = self.focus_mode;
        editor.update(cx, |ed, cx| {
            ed.set_raw_tables(raw, cx);
            ed.set_spell(spell, lang, cx);
            ed.set_smart_quotes(smart, cx);
            ed.set_focus_mode(focus, cx);
        });
        self.apply_text_width(editor, cx);
    }

    fn set_smart_quotes(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_smart_quotes(on);
        for editor in self.editors() {
            editor.update(cx, |ed, cx| ed.set_smart_quotes(on, cx));
        }
        self.save_settings(window, cx);
    }
}
