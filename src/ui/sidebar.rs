use rune::{El, ScrollSpec, TextStyle, col, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit};

use crate::core::{
    i18n::{Key, t, tf},
    spaces,
    vault::{self, NodeKind},
};

use super::{
    app::State,
    theme::{Palette, rgb},
};

pub fn view(state: &State, p: Palette, dark: bool) -> El {
    let kit = Kit::neutral(dark);
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
                .fill(rgb(if selected { p.active } else { p.panel }))
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
    col()
        .key("sidebar")
        .w(238.)
        .h_full()
        .fill(rgb(p.panel))
        .padding(12.)
        .gap(14.)
        .child(
            text(spaces::name_of(&state.root))
                .font(TextStyle::new(14., 600.))
                .color(rgb(p.fg))
                .padding(6.),
        )
        .child(
            Button::new(tf(Key::NewNote, &[]))
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Regular)
                .on_press_tx(|s: &mut State, tx| s.new_note(tx))
                .build(&kit)
                .key("new-note"),
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
        )
}
