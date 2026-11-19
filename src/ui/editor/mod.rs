pub mod analysis;
mod spans;

use rune::{El, TextStyle, text_input};

use crate::core::i18n::{Key, t};
use crate::ui::{
    app::State,
    theme::{Palette, rgb, rgba},
};

pub fn view(state: &State, palette: Palette) -> El {
    let Some(doc) = state.document() else {
        return rune::text(t(Key::NoNotesFound))
            .color(rgb(palette.dim))
            .padding(28.);
    };
    let decorations = doc
        .analysis
        .get(doc.revision)
        .map(|a| spans::decorate(&doc.edit.text, doc.edit.anchor, doc.edit.focus, a, palette))
        .unwrap_or_default();
    text_input(&doc.edit, TextStyle::new(17., 400.))
        .key("editor")
        .label(t(Key::EditorAria))
        .placeholder(t(Key::EditorPlaceholder), rgb(palette.faint))
        .w_full()
        .min_h(420.)
        .spans(decorations)
        .color(rgb(palette.body))
        .caret(rgb(palette.caret), rgba(palette.selection))
        .on_edit(|s: &mut State| &mut s.document_mut().expect("mounted editor").edit)
        .on_input_change(|s: &mut State, tx| s.edited(tx))
        .on_submit(|s: &mut State, tx| {
            if let Some(doc) = s.document_mut() {
                doc.edit.insert("\n");
            }
            s.edited(tx);
        })
}
