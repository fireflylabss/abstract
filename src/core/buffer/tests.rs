use super::*;

fn buf_with(text: &str) -> Buffer {
    let mut b = Buffer::new();
    b.set_text(text.to_string());
    b
}

#[test]
fn insert_advances_cursor() {
    let mut b = buf_with("ab");
    b.move_to(1);
    b.insert("x");
    assert_eq!(b.text(), "axb");
    assert_eq!(b.cursor(), 2);
}

#[test]
fn typing_coalesces_into_one_undo() {
    let mut b = Buffer::new();
    b.insert("a");
    b.insert("b");
    assert_eq!(b.undo.len(), 1);
    assert!(b.restore(true));
    assert_eq!(b.text(), "");
    assert!(!b.restore(true));
}

#[test]
fn newline_breaks_coalescing() {
    let mut b = Buffer::new();
    b.insert("a");
    b.insert("\n");
    assert_eq!(b.undo.len(), 2);
    assert!(b.restore(true));
    assert_eq!(b.text(), "a");
    assert!(b.restore(true));
    assert_eq!(b.text(), "");
}

#[test]
fn undo_restores_text_and_selection_redo_reapplies() {
    let mut b = buf_with("hello");
    b.move_to(5);
    b.insert(" world");
    b.move_to(2);
    assert!(b.restore(true));
    assert_eq!(b.text(), "hello");
    assert_eq!(b.sel(), 5..5);
    assert!(b.restore(false));
    assert_eq!(b.text(), "hello world");
    // Redo restores the sel captured at undo time (2..2), not the post-edit one.
    assert_eq!(b.sel(), 2..2);
}

#[test]
fn new_edit_clears_redo() {
    let mut b = buf_with("x");
    b.move_to(1);
    b.insert("y");
    assert!(b.restore(true));
    b.insert("z");
    assert!(!b.restore(false));
    assert_eq!(b.text(), "xz");
}

#[test]
fn undo_stack_capped_at_300() {
    let mut b = Buffer::new();
    for _ in 0..301 {
        b.insert("ab");
    }
    assert_eq!(b.undo.len(), 300);
}

#[test]
fn wrap_marks_and_resleects() {
    let mut b = buf_with("abc");
    b.move_to(0);
    b.select_to(3);
    b.wrap("**");
    assert_eq!(b.text(), "**abc**");
    assert_eq!(b.sel(), 2..5);

    let mut b = buf_with("ab");
    b.move_to(1);
    b.wrap("**");
    assert_eq!(b.text(), "a****b");
    assert_eq!(b.cursor(), 3);
}

#[test]
fn backspace_and_delete_remove_grapheme_clusters() {
    let mut b = buf_with("a\u{1F44D}\u{1F3FD}b"); // "a👍🏽b": cluster is bytes 1..9
    b.move_to(9);
    b.backspace();
    assert_eq!(b.text(), "ab");

    let mut b = buf_with("a\u{1F44D}\u{1F3FD}b");
    b.move_to(1);
    b.delete();
    assert_eq!(b.text(), "ab");
}

#[test]
fn backspace_with_selection_deletes_only_selection() {
    let mut b = buf_with("abc");
    b.move_to(0);
    b.select_to(1);
    b.backspace();
    assert_eq!(b.text(), "bc");
}

#[test]
fn word_movement_and_delete() {
    let b = buf_with("foo  bar-baz qux");
    assert_eq!(b.word_left(b.text().len()), 13);
    assert_eq!(b.word_right(0), 3);
    assert_eq!(b.word_right(3), 8);

    let mut b = buf_with("foo  bar-baz qux");
    b.move_to(8);
    b.delete_word_left();
    assert_eq!(b.text(), "foo  -baz qux");
}

#[test]
fn selection_reversal() {
    let mut b = buf_with("abcdef");
    b.move_to(3);
    b.select_to(1);
    assert_eq!(b.sel(), 1..3);
    assert!(b.reversed());
    assert_eq!(b.cursor(), 1);
    b.select_to(5);
    assert_eq!(b.sel(), 3..5);
    assert!(!b.reversed());
    assert_eq!(b.cursor(), 5);
}

#[test]
fn select_all_covers_buffer() {
    let mut b = buf_with("hello");
    b.select_all();
    assert_eq!(b.sel(), 0..5);
    assert!(!b.reversed());
}

#[test]
fn utf16_bridging() {
    let b = buf_with("aé\u{1F600}b"); // "aé😀b": utf16 units 1+1+2+1 = 5
    assert_eq!(b.to_utf16(b.text().len()), 5);
    assert_eq!(b.offset_from_utf16(2), 3); // utf16 2 = before 😀
    assert_eq!(b.offset_from_utf16(4), 7); // utf16 4 = after 😀 / before b
    assert_eq!(b.offset_from_utf16(5), b.text().len());
    assert_eq!(b.offset_from_utf16(99), b.text().len());
    assert_eq!(b.range_from_utf16(&(1..4)), 1..7);
}

#[test]
fn restore_cursor_clamps_to_char_boundary() {
    let mut b = buf_with("é"); // 2 bytes
    b.restore_cursor(1);
    assert_eq!(b.cursor(), 0);
    b.restore_cursor(99);
    assert_eq!(b.cursor(), 2);
}

#[test]
fn enter_continues_and_ends_lists() {
    let mut b = buf_with("- item");
    b.move_to(6);
    b.enter(0..6);
    assert_eq!(b.text(), "- item\n- ");

    let mut b = buf_with("- ");
    b.move_to(2);
    b.enter(0..2);
    assert_eq!(b.text(), "");

    let mut b = buf_with("abc");
    b.move_to(3);
    b.enter(0..3);
    assert_eq!(b.text(), "abc\n");
}

#[test]
fn enter_after_checked_task_continues_unchecked() {
    let mut b = buf_with("- [x] foo");
    b.move_to(9);
    b.enter(0..9);
    assert_eq!(b.text(), "- [x] foo\n- [ ] ");
}

#[test]
fn set_text_resets_state() {
    let mut b = buf_with("abc");
    b.move_to(1);
    b.insert("x");
    b.set_text("new".to_string());
    assert_eq!(b.sel(), 0..0);
    assert!(b.undo.is_empty());
    assert!(b.redo.is_empty());
}
