use super::*;

#[test]
fn commands_are_single_undo_steps_and_restore_original_selection() {
    for (source, anchor, focus, command) in [
        ("- first", 7, 7, Command::Enter),
        ("  - ", 4, 4, Command::Enter),
        ("- [ ] done", 9, 3, Command::ToggleTask),
        ("- one\n- two", 0, 11, Command::Indent { outdent: false }),
        ("  - one\n  - two", 15, 2, Command::Indent { outdent: true }),
        ("café", 5, 0, Command::Wrap("**")),
        ("text", 4, 0, Command::Wrap("*")),
        ("text", 0, 4, Command::Wrap("`")),
        ("text", 0, 4, Command::Wrap("~~")),
        ("text", 0, 4, Command::Link),
        ("", 0, 0, Command::Wrap("**")),
        ("", 0, 0, Command::Link),
        ("a\nb", 2, 2, Command::MoveLines { down: false }),
        ("a", 1, 1, Command::DuplicateLines),
    ] {
        let mut editor = TextEdit::new(source);
        editor.set_selection(anchor, focus);
        let change = edit::command(source, Selection { anchor, focus }, command).unwrap();
        assert!(apply(&mut editor, change));
        let after = (editor.text.clone(), editor.anchor, editor.focus);
        editor.undo();
        assert_eq!(
            (&*editor.text, editor.anchor, editor.focus),
            (source, anchor, focus)
        );
        editor.redo();
        assert_eq!((editor.text.clone(), editor.anchor, editor.focus), after);
    }
}

#[test]
fn command_undo_is_separate_from_preceding_typing() {
    let mut editor = TextEdit::new("");
    editor.insert("word");
    let change = edit::command(
        &editor.text,
        Selection {
            anchor: 0,
            focus: 4,
        },
        Command::Wrap("**"),
    )
    .unwrap();
    apply(&mut editor, change);
    editor.undo();
    assert_eq!(editor.text, "word");
    editor.undo();
    assert_eq!(editor.text, "");
}

#[test]
fn selection_only_command_does_not_add_a_text_undo_step() {
    let mut editor = TextEdit::new("");
    editor.insert("word word");
    let change = edit::command(
        &editor.text,
        Selection {
            anchor: 0,
            focus: 4,
        },
        Command::SelectNext,
    )
    .unwrap();
    assert!(!apply(&mut editor, change));
    assert_eq!((editor.anchor, editor.focus), (5, 9));
    editor.undo();
    assert_eq!(editor.text, "");
}
