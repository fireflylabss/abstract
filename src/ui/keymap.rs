use rune::{Key, KeyEvent, Tx};

use super::app::{State, commands};

pub fn shortcut(key: &KeyEvent, character: char) -> bool {
    !key.mods.alt
        && (key.is_command(character)
            || ((key.mods.cmd || key.mods.ctrl)
                && matches!(&key.key, Key::Text(text) if text.eq_ignore_ascii_case(&character.to_string()))))
}

pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    crate::core::crash::action("keyboard");
    if state.update_restarting() {
        return true;
    }
    if state.palette_key(key, tx) {
        return true;
    }
    if matches!(key.key, Key::Escape) && state.close_spell(tx) {
        return true;
    }
    if state.file_key(key, tx) {
        return true;
    }
    if state.files_busy() && !shortcut(key, 'q') {
        return true;
    }
    if matches!(key.key, Key::Escape) {
        if state.settings_open() {
            state.toggle_settings(tx);
        } else if state.close_tools(tx) {
        } else if state.focus_mode {
            state.focus_mode = false;
        } else {
            return false;
        }
        tx.instant();
        return true;
    }
    if !key.mods.shift
        && let Some(digit) = (1..=9).find(|d| shortcut(key, char::from(b'0' + *d as u8)))
    {
        state.select_tab(digit, tx);
        return true;
    }
    commands::handle(state, key, tx)
}
