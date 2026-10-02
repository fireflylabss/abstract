#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod attach;
mod buffer;
mod chrome;
mod code;
mod crash;
mod editor;
mod find;
mod footnote;
mod i18n;
mod keymap;
mod links;
mod md;
#[cfg(test)]
mod perf;
mod search;
mod spaces;
mod store;
mod table;
mod tags;
mod theme;
mod tour;
mod update;
mod vault;
mod watch;

use std::borrow::Cow;

use gpui_kit::component::Root;
use gpui_kit::*;

use app::AbstractApp;
use assets::AppAssets;
use keymap::{Quit, bind_keys};
use store::{Session, Settings};

fn main() {
    update::clean_up();
    crash::install();
    let crash = crash::take_pending(&crash::dir());
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            cx.text_system()
                .add_fonts(assets::FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
                .expect("bundled fonts");
            editor::bind_keys(cx);
            tour::bind_keys(cx);
            bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.observe_keystrokes(|e, _, _| {
                if let Some(a) = &e.action {
                    crash::action(a.name());
                }
            })
            .detach();
            cx.set_menus([Menu::new("abstract").items([MenuItem::action("Quit abstract", Quit)])]);

            let settings = Settings::load();
            i18n::set(match settings.lang() {
                i18n::LangPref::System => i18n::detect(),
                i18n::LangPref::En => i18n::Lang::En,
                i18n::LangPref::PtBr => i18n::Lang::PtBr,
            });
            let session = Session::load();
            let window_bounds = session.window().map(|w| {
                let b = Bounds {
                    origin: point(px(w.x), px(w.y)),
                    size: size(px(w.w), px(w.h)),
                };
                if w.maximized {
                    WindowBounds::Maximized(b)
                } else {
                    WindowBounds::Windowed(b)
                }
            });
            cx.spawn(async move |cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds,
                        titlebar: Some(TitlebarOptions {
                            title: Some("abstract".into()),
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(12.), px(18.))),
                        }),
                        window_decorations: Some(WindowDecorations::Client),
                        app_owns_titlebar_drag: true,
                        kind: WindowKind::Normal,
                        is_movable: true,
                        is_resizable: true,
                        is_minimizable: true,
                        focus: true,
                        show: true,
                        app_id: Some("abstract".into()),
                        window_min_size: Some(size(px(560.), px(360.))),
                        ..Default::default()
                    },
                    |window, cx| {
                        theme::apply(&settings, window.appearance(), cx);
                        let view = cx.new(|cx| {
                            let mut app =
                                AbstractApp::new(window, cx, settings.clone(), session.clone());
                            app.crash = crash.clone();
                            app
                        });
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                )
                .expect("failed to open abstract window");
            })
            .detach();
        });
}
