use rune::{Key, KeyEvent, Tx};

use super::app::State;

pub fn shortcut(key: &KeyEvent, character: char) -> bool {
    !key.mods.alt
        && (key.is_command(character)
            || ((key.mods.cmd || key.mods.ctrl)
                && matches!(&key.key, Key::Text(text) if text.eq_ignore_ascii_case(&character.to_string()))))
}

pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    if state.file_key(key, tx) {
        return true;
    }
    if state.files_busy() && !shortcut(key, 'q') {
        return true;
    }
    let plain = !key.mods.shift;
    if (plain && shortcut(key, ',')) || (matches!(key.key, Key::Escape) && state.settings_open()) {
        state.toggle_settings(tx);
    } else if key.mods.shift && shortcut(key, 'e') {
        if let Some(path) = state.active.clone() {
            state.export_note(path, crate::io::export::Format::Html, tx);
        }
    } else if plain
        && key.mods.alt
        && (key.mods.cmd || key.mods.ctrl)
        && (key.is_command('e')
            || matches!(&key.key, Key::Text(text) if text.eq_ignore_ascii_case("e")))
    {
        if let Some(path) = state.active.clone() {
            state.export_note(path, crate::io::export::Format::Pdf, tx);
        }
    } else if key.mods.shift && shortcut(key, 'f') {
        state.open_search(tx);
    } else if plain && shortcut(key, 'f') {
        state.open_find(false, tx);
    } else if (plain && shortcut(key, 'h'))
        || (plain
            && key.mods.alt
            && (key.mods.cmd || key.mods.ctrl)
            && (key.is_command('f')
                || matches!(&key.key, Key::Text(text) if text.eq_ignore_ascii_case("f"))))
    {
        state.open_find(true, tx);
    } else if shortcut(key, 'g') || matches!(key.key, Key::Function(3)) {
        state.find_step(plain, tx);
    } else if key.mods.alt && !key.mods.cmd && !key.mods.ctrl && matches!(key.key, Key::Enter) {
        state.follow_link(tx);
    } else if matches!(key.key, Key::Escape) && state.close_tools(tx) {
    } else if plain && (shortcut(key, 'n') || shortcut(key, 't')) {
        state.new_note(tx);
    } else if plain && shortcut(key, 'w') {
        if let Some(path) = state.active.clone() {
            state.close_tab(&path, tx);
        }
    } else if plain
        && let Some(digit) = (1..=9).find(|digit| shortcut(key, char::from(b'0' + *digit as u8)))
    {
        state.select_tab(digit, tx);
    } else if !key.mods.alt && key.mods.ctrl && matches!(key.key, Key::Tab) {
        state.cycle_tab(!key.mods.shift, tx);
    } else if !key.mods.alt && key.mods.ctrl && matches!(key.key, Key::PageUp | Key::PageDown) {
        state.cycle_tab(matches!(key.key, Key::PageDown), tx);
    } else if key.mods.shift && (shortcut(key, ']') || shortcut(key, '}')) {
        state.cycle_tab(true, tx);
    } else if key.mods.shift && (shortcut(key, '[') || shortcut(key, '{')) {
        state.cycle_tab(false, tx);
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
