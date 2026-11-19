use rune::{El, ScrollSpec, TextStyle, col, row, text};

use crate::core::{
    i18n::{Key, t},
    spaces,
    vault::{self, NodeKind},
};

use super::{
    app::State,
    theme::{Palette, rgb},
};

pub fn view(state: &State, p: Palette) -> El {
    let mut tree = col()
        .key("tree")
        .role("tree")
        .label(t(Key::Notes))
        .w_full()
        .gap(2.);
    for item in vault::flatten(&state.tree, &state.expanded) {
        let path = item.path.clone();
        let folder = item.kind == NodeKind::Folder;
        let selected = state.active.as_ref() == Some(&path);
        let prefix = if folder {
            if item.expanded { "▾ " } else { "▸ " }
        } else {
            ""
        };
        tree = tree.child(
            row()
                .key(format!("note:{}", item.path.display()))
                .role("treeitem")
                .label(item.name.clone())
                .focusable(true)
                .focus_selected(selected)
                .w_full()
                .padding(7.)
                .fill(if selected {
                    rgb(p.active)
                } else {
                    rgb(p.panel).alpha(0.)
                })
                .hover(rgb(p.hover))
                .child(row().w(item.depth as f32 * 14.))
                .child(
                    text(format!("{prefix}{}", item.name))
                        .font(TextStyle::new(13., 400.))
                        .color(rgb(if selected { p.fg } else { p.dim })),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    if folder {
                        if !s.expanded.remove(&path) {
                            s.expanded.insert(path.clone());
                        }
                    } else {
                        s.open(path.clone(), tx);
                    }
                }),
        );
    }
    let content = col()
        .key("sidebar")
        .w(state.settings.number("sidebar_width"))
        .h_full()
        .padding(12.)
        .gap(14.)
        .child(
            text(spaces::name_of(&state.root))
                .key("space-name")
                .font(TextStyle::new(14., 600.))
                .color(rgb(p.fg))
                .padding(6.),
        )
        .child(
            row()
                .key("new-note")
                .role("button")
                .focusable(true)
                .w_full()
                .items_center()
                .justify_between()
                .padding(7.)
                .hover(rgb(p.hover))
                .child(
                    text(
                        t(Key::NewNote)
                            .split(" (")
                            .next()
                            .unwrap_or(t(Key::NewNote)),
                    )
                    .font(TextStyle::new(13., 400.))
                    .color(rgb(p.fg)),
                )
                .child(
                    text(format!("{}+N", crate::core::prefs::MOD))
                        .font(TextStyle::new(11., 400.))
                        .color(rgb(p.faint)),
                )
                .on_press_tx(|s: &mut State, tx| s.new_note(tx)),
        )
        .child(
            col()
                .grow(1.)
                .min_h(0.)
                .w_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(tree),
        );
    super::app::settings::surface(
        content,
        &state.settings,
        crate::core::prefs::Surface::Sidebar,
        p.panel,
    )
}
