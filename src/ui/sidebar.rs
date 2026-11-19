use rune::{El, ScrollSpec, TextStyle, col, row, text};

use crate::core::{
    i18n::{Key, t},
    spaces,
    vault::{self, NodeKind},
};

use super::{
    app::State,
    chrome,
    theme::{Palette, rgb},
};

pub fn view(state: &State, p: Palette, inset: f32) -> El {
    let inactive = if super::app::settings::dark_sidebar(&state.settings, p.panel) {
        rgb(p.dim).mix(rgb(p.fg), 0.15)
    } else {
        rgb(p.dim)
    };
    let mut tree = col()
        .key("tree")
        .role("tree")
        .label(t(Key::Notes))
        .w_full()
        .gap(2.);
    for item in vault::flatten(&state.tree, &state.expanded) {
        let path = item.path.clone();
        let folder = item.kind == NodeKind::Folder;
        if let Some(edit) = super::app::file_ops::rename_view(state, &path, p) {
            tree = tree.child(edit);
            continue;
        }
        let context = path.clone();
        let keyboard = path.clone();
        let selected = state.active.as_ref() == Some(&path);
        tree = tree.child(
            row()
                .key(format!("note:{}", item.path.display()))
                .role("treeitem")
                .label(item.name.clone())
                .focusable(true)
                .disabled(state.files_busy())
                .on_secondary_press(move |s: &mut State, press, tx| {
                    super::app::file_ops::secondary(s, context.clone(), folder, press, tx)
                })
                .on_key(move |s: &mut State, key, tx| {
                    s.sidebar_file_key(&keyboard, folder, key, tx)
                })
                .focus_selected(selected)
                .w_full()
                .h(28.)
                .padding(6.)
                .items_center()
                .gap(6.)
                .clip_rounded(6.)
                .fill(if selected {
                    rgb(p.active)
                } else {
                    rgb(p.panel).alpha(0.)
                })
                .hover(rgb(p.hover))
                .child(row().w(12. + item.depth as f32 * 14.))
                .child(
                    rune::icon(
                        if folder {
                            &chrome::CHEVRON
                        } else {
                            &chrome::NOTE
                        },
                        14.,
                    )
                    .icon_rotation(if folder && item.expanded {
                        std::f32::consts::FRAC_PI_2
                    } else {
                        0.
                    })
                    .color(rgb(p.dim)),
                )
                .child(
                    text(item.name)
                        .truncate()
                        .min_w(0.)
                        .font(TextStyle::new(12., 400.))
                        .color(if selected { rgb(p.fg) } else { inactive }),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    if folder {
                        s.toggle_folder(path.clone(), tx);
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
        .padding(8.)
        .gap(4.)
        .child(
            row()
                .h(32.)
                .w_full()
                .items_center()
                .gap(4.)
                .window_drag()
                .child(row().w(if inset > 0. { inset + 12. } else { 0. }))
                .child(rune::icon(&chrome::FOLDER, 15.).color(rgb(p.dim)))
                .child(
                    text(spaces::name_of(&state.root))
                        .key("space-name")
                        .truncate()
                        .min_w(0.)
                        .grow(1.)
                        .font(TextStyle::new(12., 600.))
                        .color(rgb(p.fg)),
                )
                .child(rune::icon(&chrome::SWITCH, 12.).color(rgb(p.faint)))
                .child(
                    chrome::button("new-folder", t(Key::NewFolder), &chrome::FOLDER_ADD, p)
                        .disabled(state.files_busy())
                        .on_press_tx(|s: &mut State, tx| {
                            s.rename_file(s.root.clone(), true, true, tx)
                        }),
                )
                .child(
                    chrome::button("new-note", t(Key::NewNote), &chrome::ADD, p)
                        .disabled(state.files_busy())
                        .on_press_tx(|s: &mut State, tx| s.new_note(tx)),
                ),
        )
        .child(
            row()
                .w_full()
                .h(24.)
                .items_center()
                .justify_between()
                .padding(6.)
                .child(
                    text(t(Key::Notes).to_uppercase())
                        .font(TextStyle::new(10., 500.))
                        .color(rgb(p.dim)),
                )
                .child(
                    text(note_count(&state.tree).to_string())
                        .font(TextStyle::new(10., 400.))
                        .color(rgb(p.dim)),
                ),
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

fn note_count(nodes: &[vault::Node]) -> usize {
    nodes
        .iter()
        .map(|n| {
            if n.is_folder() {
                note_count(&n.children)
            } else {
                1
            }
        })
        .sum()
}
