mod app;
mod assets;
mod buffer;
mod chrome;
mod editor;
mod keymap;
mod md;
mod spaces;
mod store;
mod theme;
mod tour;
mod vault;

use gpui_kit::component::Root;
use gpui_kit::*;

use app::AbstractApp;
use assets::AppAssets;
use keymap::{Quit, bind_keys};
use store::{Session, Settings};

fn main() {
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            editor::bind_keys(cx);
            tour::bind_keys(cx);
            bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus([Menu::new("abstract").items([MenuItem::action("Quit abstract", Quit)])]);

            let settings = Settings::load();
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
                            traffic_light_position: None,
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
                        theme::apply(settings.theme(), window.appearance(), cx);
                        let view = cx.new(|cx| {
                            AbstractApp::new(window, cx, settings.clone(), session.clone())
                        });
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                )
                .expect("failed to open abstract window");
            })
            .detach();
        });
}
