use rune::{El, Icon, TextStyle, col, icon, row, text};

use super::{
    app::{State, commands},
    theme::{Palette, rgb},
};
use crate::core::i18n::{Key, t, tf};
use crate::io::save::State as SaveState;

macro_rules! asset {
    ($name:ident, $file:literal) => {
        pub static $name: Icon = Icon {
            name: $file,
            view: 24.,
            body: include_str!(concat!("../../assets/icons/", $file, ".svg")),
        };
    };
}
asset!(NOTE, "note-01");
asset!(FOLDER, "folder-01");
asset!(FOLDER_ADD, "folder-add");
asset!(ADD, "add-01");
asset!(SEARCH, "search");
asset!(SIDEBAR, "view-sidebar-left");
asset!(THEME, "monitor");
asset!(TRASH, "delete-02");
asset!(SWITCH, "unfold-more");
pub static CHEVRON: Icon = Icon {
    name: "chevron",
    view: 24.,
    body: r#"<path d="m9 6 6 6-6 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static MOON: Icon = Icon {
    name: "moon",
    view: 24.,
    body: r#"<path d="M20 15.5A8.5 8.5 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>"#,
};

pub fn button(key: &str, label: &str, glyph: &'static Icon, p: Palette) -> El {
    row()
        .key(key)
        .role("button")
        .label(label.replace("{MOD}", crate::core::prefs::MOD))
        .focusable(true)
        .size(28., 28.)
        .items_center()
        .justify_center()
        .clip_rounded(6.)
        .color(rgb(p.dim))
        .hover(rgb(p.hover))
        .child(icon(glyph, 16.).color(rgb(p.dim)))
}

pub fn toolbar(s: &State, p: Palette, inset: f32) -> El {
    let words = s
        .document()
        .map_or(0, |d| d.edit.text.split_whitespace().count())
        .to_string();
    let mut bar = row()
        .key("editor-toolbar")
        .w_full()
        .h(48.)
        .padding(8.)
        .items_center()
        .gap(4.)
        .window_drag();
    if !s.sidebar_open || s.focus_mode {
        bar = bar.child(col().w(inset));
    }
    let status = s.document().map_or(String::new(), |doc| match &doc.save {
        SaveState::Clean { .. } => String::new(),
        SaveState::Failed { error, .. } => format!("{}: {error}", t(Key::SaveFailed)),
        _ => t(Key::Saving).to_string(),
    });
    bar.child(
        button("sidebar-toggle", t(Key::Sidebar), &SIDEBAR, p).on_press_tx(|s: &mut State, tx| {
            commands::run(s, "sidebar", tx);
        }),
    )
    .child(
        button("search-open", t(Key::Search), &SEARCH, p).on_press_tx(|s: &mut State, tx| {
            commands::run(s, "search", tx);
        }),
    )
    .child(col().grow(1.))
    .child(
        text(status)
            .key("save-status")
            .font(TextStyle::new(11., 400.))
            .color(rgb(p.dim)),
    )
    .child(
        text(tf(Key::Words, &[("n", &words)]))
            .font(TextStyle::new(11., 400.))
            .color(rgb(p.dim)),
    )
    .child(
        button(
            "theme-toggle",
            t(Key::ThemeCommand),
            if p.bg == super::theme::DARK.bg {
                &MOON
            } else {
                &THEME
            },
            p,
        )
        .on_press_tx(|s: &mut State, tx| {
            commands::run(s, "theme", tx);
        }),
    )
    .child(
        button("note-trash", t(Key::MoveToTrash), &TRASH, p)
            .disabled(s.document().is_none() || s.files_busy())
            .on_press_tx(|s: &mut State, tx| {
                commands::run(s, "trash", tx);
            }),
    )
}
