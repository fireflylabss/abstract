use rune::{Key, KeyEvent, Tx};

use super::app::State;

pub fn shortcut(key: &KeyEvent, character: char) -> bool {
    !key.mods.alt
        && (key.is_command(character)
            || ((key.mods.cmd || key.mods.ctrl)
                && matches!(&key.key, Key::Text(text) if text.eq_ignore_ascii_case(&character.to_string()))))
}

pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    let plain = !key.mods.shift;
    if plain && shortcut(key, 'n') {
        state.new_note(tx);
    } else if plain && shortcut(key, 's') {
        state.save_now(tx);
    } else if plain && shortcut(key, 'q') {
        tx.quit();
    } else if shortcut(key, '=') || shortcut(key, '+') {
        state
            .settings
            .set_zoom((state.settings.zoom() + 0.1).clamp(0.5, 2.));
        state.save_settings(tx);
    } else if plain && shortcut(key, '-') {
        state
            .settings
            .set_zoom((state.settings.zoom() - 0.1).clamp(0.5, 2.));
        state.save_settings(tx);
    } else if plain && shortcut(key, '0') {
        state.settings.set_zoom(1.);
        state.save_settings(tx);
    } else if plain && shortcut(key, '\\') {
        state.sidebar_open = !state.sidebar_open;
    } else if key.mods.shift && shortcut(key, 'l') {
        state.settings.set_theme(state.settings.theme().next());
        state.save_settings(tx);
    } else if key.mods.shift
        && matches!(key.key, Key::Enter)
        && (key.mods.cmd || key.mods.ctrl)
        && !key.mods.alt
    {
        state.focus_mode = !state.focus_mode;
        tx.focus("editor", false);
    } else if matches!(key.key, Key::Escape) && state.focus_mode {
        state.focus_mode = false;
    } else {
        return false;
    }
    true
}
