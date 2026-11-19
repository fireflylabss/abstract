pub mod analysis;
mod keys;
mod spans;

use rune::{El, text_input};

use crate::core::i18n::{Key, t};
use crate::ui::{
    app::State,
    theme::{Palette, rgb, rgba},
};

pub fn view(state: &State, palette: Palette) -> El {
    if state.files_busy() {
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
    let highlights = state
        .current_find()
        .map(|range| rune::TextHighlight::new(range, rune::Color::TRANSPARENT).key("find-current"))
        .into_iter()
        .collect::<Vec<_>>();
    text_input(&doc.edit, state.editor_style())
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
        .on_input_key(keys::handle)
}
