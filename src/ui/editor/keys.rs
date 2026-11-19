use rune::{Key, KeyEvent, TextEdit, Tx};

use crate::core::md::edit::{self, Command, Edit, Selection};
use crate::ui::{
    app::State,
    keymap::{self, shortcut},
};

pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    if keymap::handle(state, key, tx) {
        return true;
    }
    let Some(doc) = state.document_mut() else {
        return false;
    };
    if shortcut(key, 'z') || (!key.mods.shift && shortcut(key, 'y')) {
        let before = doc.edit.text.clone();
        if key.mods.shift || shortcut(key, 'y') {
            doc.edit.redo();
        } else {
            doc.edit.undo();
        }
        if before != doc.edit.text {
            state.edited(tx);
        }
        return true;
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
        _ if !key.mods.shift && shortcut(key, 'b') => Command::Wrap("**"),
        _ if !key.mods.shift && shortcut(key, 'i') => Command::Wrap("*"),
        _ if !key.mods.shift && shortcut(key, 'e') => Command::Wrap("`"),
        _ if key.mods.shift && shortcut(key, 'x') => Command::Wrap("~~"),
        _ if key.mods.shift && shortcut(key, 'h') => Command::Wrap("=="),
        _ if !key.mods.shift && shortcut(key, '/') => Command::Wrap("%%"),
        _ if !key.mods.shift && shortcut(key, 'k') => Command::Link,
        _ if key.mods.shift && shortcut(key, 'd') => Command::DuplicateLines,
        _ if !key.mods.shift && shortcut(key, 'd') => Command::SelectNext,
        _ => return false,
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
    true
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
