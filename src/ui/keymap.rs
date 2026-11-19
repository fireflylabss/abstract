use rune::{KeyEvent, Tx};

use super::app::State;

pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    if key.is_command('n') {
        state.new_note(tx);
    } else if key.is_command('s') {
        state.save_now(tx);
    } else if key.is_command('q') {
        tx.quit();
    } else {
        return false;
    }
    true
}
