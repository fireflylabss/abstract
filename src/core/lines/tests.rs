use super::*;

fn moved(text: &str, sel: Range<usize>, down: bool) -> Option<(String, Range<usize>)> {
    move_sel(text, &sel, down).map(|(r, new, s)| {
        let mut t = text.to_string();
        t.replace_range(r, &new);
        (t, s)
    })
}

fn duped(text: &str, sel: Range<usize>) -> (String, Range<usize>) {
    let (r, new, s) = duplicate_sel(text, &sel);
    let mut t = text.to_string();
    t.replace_range(r, &new);
    (t, s)
}

#[test]
fn move_line_down_and_up() {
    let (t, s) = moved("a\nb\nc", 2..3, true).unwrap();
    assert_eq!(t, "a\nc\nb");
    assert_eq!(s, 4..5);
    let (t, s) = moved("a\nc\nb", 4..5, false).unwrap();
    assert_eq!(t, "a\nb\nc");
    assert_eq!(s, 2..3);
}

#[test]
fn move_selection_block() {
    // Selection over lines b–c moves both.
    let (t, s) = moved("a\nb\nc\nd", 2..6, true).unwrap();
    assert_eq!(t, "a\nd\nb\nc");
    assert_eq!(s, 4..7);
    let (t, s) = moved("a\nb\nc\nd", 2..6, false).unwrap();
    assert_eq!(t, "b\nc\na\nd");
    assert_eq!(s, 0..4);
}

#[test]
fn move_at_edges_is_noop() {
    assert_eq!(moved("a\nb", 0..1, false), None); // first line up
    assert_eq!(moved("a\nb", 2..3, true), None); // last line down
    assert_eq!(moved("", 0..0, true), None);
}

#[test]
fn move_last_line_with_newline_is_noop() {
    // "b" is the last visible line; the empty tail a trailing '\n'
    // leaves isn't a line it can move onto.
    assert_eq!(moved("a\nb\n", 2..3, true), None);
    // A caret on that empty tail moves it up like any other line.
    let (t, s) = moved("a\nb\n", 4..4, false).unwrap();
    assert_eq!(t, "a\n\nb");
    assert_eq!(s, 2..2);
}

#[test]
fn move_last_line_without_newline() {
    let (t, s) = moved("a\nb", 2..3, false).unwrap();
    assert_eq!(t, "b\na");
    assert_eq!(s, 0..1);
    // Moving a line over the last one keeps no trailing '\n'.
    let (t, s) = moved("a\nb", 0..1, true).unwrap();
    assert_eq!(t, "b\na");
    assert_eq!(s, 2..3);
}

#[test]
fn move_with_cursor_in_middle_of_line() {
    let (t, s) = moved("one\ntwo\nthree", 5..5, true).unwrap();
    assert_eq!(t, "one\nthree\ntwo");
    assert_eq!(s, 11..11);
}

#[test]
fn move_selection_ending_at_line_start() {
    // "b\n" selected (ends on c's start): only b moves.
    let (t, s) = moved("a\nb\nc", 2..4, true).unwrap();
    assert_eq!(t, "a\nc\nb");
    assert_eq!(s, 4..5);
}

#[test]
fn duplicate_single_line() {
    let (t, s) = duped("a\nb\nc", 2..3);
    assert_eq!(t, "a\nb\nb\nc");
    assert_eq!(s, 4..5);
}

#[test]
fn duplicate_block_and_cursor() {
    let (t, s) = duped("a\nb\nc", 0..3);
    assert_eq!(t, "a\nb\na\nb\nc");
    assert_eq!(s, 4..7);
    // A collapsed caret lands in the copy.
    let (t, s) = duped("a\nb\nc", 0..0);
    assert_eq!(t, "a\na\nb\nc");
    assert_eq!(s, 2..2);
}

#[test]
fn duplicate_last_line_no_newline() {
    let (t, s) = duped("a\nb", 2..3);
    assert_eq!(t, "a\nb\nb");
    assert_eq!(s, 4..5);
}

#[test]
fn select_next_word_at_caret() {
    assert_eq!(select_next("hello world", &(0..0)), Some(0..5));
    assert_eq!(select_next("hello world", &(5..5)), Some(0..5)); // word end
    assert_eq!(select_next("hello world", &(7..7)), Some(6..11));
    // Inside a run of non-word chars: the next word forward.
    assert_eq!(select_next("a  --  b", &(3..3)), Some(7..8));
    assert_eq!(select_next("   ", &(1..1)), None);
}

#[test]
fn select_next_occurrence_wraps() {
    let t = "aa bb aa";
    assert_eq!(select_next(t, &(0..0)), Some(0..2));
    assert_eq!(select_next(t, &(0..2)), Some(6..8));
    assert_eq!(select_next(t, &(6..8)), Some(0..2)); // wraps to the top
    // The only occurrence selects itself.
    assert_eq!(select_next("solo", &(0..4)), Some(0..4));
    // Case-sensitive: "AA" isn't a match for "aa".
    assert_eq!(select_next("aa AA aa", &(0..2)), Some(6..8));
}

#[test]
fn select_next_unicode() {
    assert_eq!(select_next("açúcar açúcar", &(0..8)), Some(9..17));
}
