pub mod analysis;
mod blocks;
mod keys;
pub(crate) use keys::{command, restore};
mod spans;

use rune::{Cx, El, col, text_input};

use crate::core::i18n::{Key, t};
use crate::ui::{
    app::State,
    theme::{Palette, rgb, rgba},
};

pub fn view(state: &State, cx: &Cx<State>, palette: Palette) -> El {
    if state.files_busy() || state.update_restarting() {
        return rune::text(t(Key::Saving))
            .key("file-operation-busy")
            .color(rgb(palette.dim))
            .padding(28.);
    }
    let Some(doc) = state.document() else {
        return rune::text(t(Key::NoNotesFound))
            .color(rgb(palette.dim))
            .padding(28.);
    };
    let mut decorations = doc
        .analysis
        .get(doc.revision)
        .map(|a| {
            spans::decorate_with(
                &doc.edit.text,
                doc.edit.anchor,
                doc.edit.focus,
                a,
                palette,
                &state.settings,
            )
        })
        .unwrap_or_default();
    decorations.extend(state.find_spans(palette));
    decorations.extend(state.spell_spans(palette));
    let opacity = if state.settings.glass() {
        state.settings.glass_text_opacity() as f32 / 100.
    } else {
        1.
    };
    for span in &mut decorations {
        if let Some(color) = span.color {
            span.color = Some(color.alpha(color.a * opacity));
        }
    }
    let (back, front, mut highlights) = blocks::layers(state, cx, palette);
    highlights.extend(
        state
            .current_find()
            .map(|range| {
                rune::TextHighlight::new(range, rune::Color::TRANSPARENT).key("find-current")
            })
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let mut editor = text_input(&doc.edit, state.editor_style())
        .key("editor")
        .label(t(Key::EditorAria))
        .placeholder(t(Key::EditorPlaceholder), rgb(palette.faint))
        .w_full()
        .min_h(420.)
        .spans(decorations)
        .text_highlights(highlights)
        .color(rgb(palette.body).alpha(opacity))
        .caret(
            if state.custom_caret() {
                rune::Color::TRANSPARENT
            } else {
                rgb(palette.caret)
            },
            rgba(palette.selection),
        )
        .on_edit(|s: &mut State| &mut s.document_mut().expect("mounted editor").edit)
        .on_input_change(|s: &mut State, tx| s.edited(tx))
        .on_input_paste(State::paste_attachment)
        .on_drop_files(State::drop_images)
        .accepts_files(|path| crate::core::md::is_image_path(&path.to_string_lossy()))
        .on_secondary_press(|s: &mut State, press, tx| {
            if let Some(path) = s.active.clone() {
                crate::ui::app::file_ops::secondary(s, path, false, press, tx);
            }
        })
        .on_input_key(keys::handle);
    if let Some(a) = doc.analysis.get(doc.revision) {
        for task in a.tasks.iter().filter(|task| task.checked) {
            let range = blocks::task_text(&doc.edit.text, &task.marker);
            editor = editor.strike(range.start, range.end, Some(rgb(palette.muted)));
        }
    }
    let mut content = col().w_full().instant_changes();
    for layer in back {
        content = content.child(layer);
    }
    content = content.child(editor);
    for layer in front {
        content = content.child(layer);
    }
    content
}
