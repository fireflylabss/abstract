use rune::{Key, KeyEvent, TextEdit, Tx};

use crate::core::md::edit::{self, Command, Edit, Selection};
use crate::ui::{app::State, keymap};

pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    if state.files_busy() {
        return true;
    }
    if keymap::handle(state, key, tx) {
        return true;
    }
    let pairs = state.settings.enabled("smart_pairs");
    let quotes = state.settings.smart_quotes();
    let Some(doc) = state.document_mut() else {
        return false;
    };
    if !key.mods.cmd
        && !key.mods.ctrl
        && !key.mods.alt
        && let Key::Text(text) = &key.key
        && text.chars().count() == 1
    {
        let typed = text.chars().next().unwrap();
        let selection = Selection {
            anchor: doc.edit.anchor,
            focus: doc.edit.focus,
        };
        let offset = selection.anchor.min(selection.focus);
        let start = doc.edit.text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let end = doc.edit.text[offset..]
            .find('\n')
            .map_or(doc.edit.text.len(), |i| offset + i);
        let before = &doc.edit.text[start..offset];
        let literal = doc
            .analysis
            .get(doc.revision)
            .is_none_or(|a| crate::core::smart::literal_at(a, offset, before));
        if pairs
            && !literal
            && let Some(change) = edit::pair(&doc.edit.text, selection, typed)
        {
            if apply(&mut doc.edit, change) {
                state.edited(tx);
            }
            return true;
        }
        if doc.edit.anchor == doc.edit.focus
            && let Some(subst) = crate::core::smart::substitute(
                &doc.edit.text[start..end],
                before,
                typed,
                quotes,
                literal,
            )
        {
            let back = before
                .chars()
                .rev()
                .take(subst.back)
                .map(char::len_utf8)
                .sum::<usize>();
            let range = offset - back..offset;
            doc.edit.undo_group(|e| {
                e.set_selection(range.start, range.end);
                e.insert(subst.text);
            });
            state.edited(tx);
            return true;
        }
    }
    let command = match key.key {
        Key::Enter if !key.mods.alt && (key.mods.cmd || key.mods.ctrl) => Command::ToggleTask,
        Key::Enter if !key.mods.alt => Command::Enter,
        Key::Tab if !key.mods.alt && !key.mods.cmd && !key.mods.ctrl => Command::Indent {
            outdent: key.mods.shift,
        },
        Key::Up | Key::Down
            if key.mods.alt && !key.mods.cmd && !key.mods.ctrl && !key.mods.shift =>
        {
            Command::MoveLines {
                down: matches!(key.key, Key::Down),
            }
        }
        _ => return false,
    };
    self::command(state, command, tx);
    true
}

pub(crate) fn restore(state: &mut State, redo: bool, tx: &mut Tx) {
    let Some(doc) = state.document_mut() else {
        return;
    };
    let before = doc.edit.text.clone();
    if redo {
        doc.edit.redo();
    } else {
        doc.edit.undo();
    }
    if before != doc.edit.text {
        state.edited(tx);
    }
}

pub(crate) fn command(state: &mut State, command: Command, tx: &mut Tx) {
    let Some(doc) = state.document_mut() else {
        return;
    };
    let selection = Selection {
        anchor: doc.edit.anchor,
        focus: doc.edit.focus,
    };
    if let Some(change) = edit::command(&doc.edit.text, selection, command)
        && apply(&mut doc.edit, change)
    {
        state.edited(tx);
    }
}

fn apply(edit: &mut TextEdit, change: Edit) -> bool {
    let changed = edit.text[change.range.clone()] != change.text;
    if changed {
        edit.undo_group(|edit| {
            edit.set_selection(change.range.start, change.range.end);
            edit.insert(&change.text);
            edit.set_selection(change.selection.anchor, change.selection.focus);
        });
    } else {
        edit.set_selection(change.selection.anchor, change.selection.focus);
    }
    changed
}

#[cfg(test)]
mod tests;
