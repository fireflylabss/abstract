use super::*;

fn apply(source: &str, anchor: usize, focus: usize, cmd: Command) -> (String, Selection) {
    let edit = command(source, Selection { anchor, focus }, cmd).expect("edit");
    let mut text = source.to_owned();
    text.replace_range(edit.range, &edit.text);
    assert!(text.is_char_boundary(edit.selection.anchor));
    assert!(text.is_char_boundary(edit.selection.focus));
    (text, edit.selection)
}

fn at_end(source: &str, cmd: Command) -> String {
    apply(source, source.len(), source.len(), cmd).0
}

#[test]
fn enter_continues_markers_and_indentation() {
    for (source, expected) in [
        ("- item", "- item\n- "),
        ("  * item", "  * item\n  * "),
        ("\t+ item", "\t+ item\n\t+ "),
        ("9. item", "9. item\n10. "),
        ("  09) item", "  09) item\n  10) "),
        ("- [ ] todo", "- [ ] todo\n- [ ] "),
        ("  * [x] done", "  * [x] done\n  * [ ] "),
        ("+ [X] done", "+ [X] done\n+ [ ] "),
        ("3. [x] done", "3. [x] done\n4. [ ] "),
        ("> quote", "> quote\n> "),
        ("  > > quote", "  > > quote\n  > > "),
        ("  prose", "  prose\n  "),
        ("", "\n"),
    ] {
        assert_eq!(at_end(source, Command::Enter), expected, "{source:?}");
    }
}

#[test]
fn enter_exits_empty_items_but_not_similar_text() {
    for source in [
        "- ", "  -   ", "\t* [x] ", "+ [ ]", "12. ", "> ", "  > > ", "-",
    ] {
        assert_eq!(at_end(source, Command::Enter), "", "{source:?}");
    }
    for source in [
        "-word",
        "1.not a list",
        "18446744073709551615. item",
        "999999999999999999999999. item",
    ] {
        assert_eq!(at_end(source, Command::Enter), format!("{source}\n"));
    }
}

#[test]
fn enter_splits_and_replaces_selection_without_touching_other_lines() {
    assert_eq!(
        apply("- café\nend", 5, 5, Command::Enter).0,
        "- caf\n- é\nend"
    );
    assert_eq!(apply("- one two", 8, 5, Command::Enter).0, "- one\n- o");
    assert_eq!(apply("- item", 0, 0, Command::Enter).0, "\n- item");
    assert_eq!(
        at_end("- one\r\n- two", Command::Enter),
        "- one\r\n- two\r\n- "
    );
    assert_eq!(apply("- \r\nrest", 2, 2, Command::Enter).0, "\r\nrest");
}

#[test]
fn fenced_code_stays_literal_with_indent_and_matching_fences() {
    for source in [
        "```md\n  - item",
        "~~~\n\t- [x] item",
        "````\n```\n  > quote",
        "~~~\n```\n  1. item",
    ] {
        let pad = if source.ends_with("item") && source.contains('\t') {
            "\t"
        } else {
            "  "
        };
        assert_eq!(at_end(source, Command::Enter), format!("{source}\n{pad}"));
        assert!(
            command(
                source,
                Selection {
                    anchor: source.len(),
                    focus: source.len()
                },
                Command::ToggleTask
            )
            .is_none()
        );
    }
    assert_eq!(
        at_end("```\n- \n```\n- item", Command::Enter),
        "```\n- \n```\n- item\n- "
    );
    assert_eq!(at_end("```\n  - ", Command::Enter), "```\n  - \n  ");
    assert_eq!(
        at_end("```\r\n\t- item", Command::Enter),
        "```\r\n\t- item\r\n\t"
    );
}

#[test]
fn task_toggle_preserves_selection_and_accepts_all_list_markers() {
    for source in [
        "- [ ] café",
        "  * [ ] café",
        "\t+ [ ] café",
        "2. [ ] café",
        "- [ ]\r\n",
    ] {
        let focus = source.find(']').unwrap();
        let (checked, sel) = apply(source, 0, focus, Command::ToggleTask);
        assert_eq!(checked, source.replacen("[ ]", "[x]", 1));
        assert_eq!(sel, Selection { anchor: 0, focus });
        assert_eq!(apply(&checked, 0, focus, Command::ToggleTask).0, source);
    }
    assert_eq!(at_end("- [X] done", Command::ToggleTask), "- [ ] done");
    for source in ["", "[ ] text", "- ordinary", "- [no] text"] {
        assert!(
            command(
                source,
                Selection {
                    anchor: 0,
                    focus: 0
                },
                Command::ToggleTask
            )
            .is_none()
        );
    }
}

#[test]
fn indent_whole_selected_lines_and_restore_reversed_unicode_selection() {
    let source = "- café\n  * ação\n+ last";
    let end = source.find('+').unwrap();
    let (text, selection) = apply(source, end, 2, Command::Indent { outdent: false });
    assert_eq!(text, "  - café\n    * ação\n+ last");
    assert_eq!(
        selection,
        Selection {
            anchor: end + 4,
            focus: 4
        }
    );
    assert_eq!(
        apply(
            &text,
            selection.anchor,
            selection.focus,
            Command::Indent { outdent: true }
        ),
        (
            source.into(),
            Selection {
                anchor: end,
                focus: 2
            }
        )
    );
    assert_eq!(at_end("\t- a", Command::Indent { outdent: true }), "- a");
    assert_eq!(at_end(" - a", Command::Indent { outdent: true }), "- a");
    assert!(
        command(
            "- a",
            Selection {
                anchor: 0,
                focus: 0
            },
            Command::Indent { outdent: true }
        )
        .is_none()
    );
}

#[test]
fn tab_elsewhere_inserts_two_spaces_and_shift_tab_does_nothing() {
    for source in ["", "prose", "```\n- code", "> quote"] {
        assert_eq!(
            at_end(source, Command::Indent { outdent: false }),
            format!("{source}  ")
        );
        assert!(
            command(
                source,
                Selection {
                    anchor: 0,
                    focus: 0
                },
                Command::Indent { outdent: true }
            )
            .is_none()
        );
    }
    assert_eq!(
        apply("prose", 1, 4, Command::Indent { outdent: false }).0,
        "p  e"
    );
    assert_eq!(
        apply("- a\r\n- b", 0, 8, Command::Indent { outdent: false }).0,
        "  - a\r\n  - b"
    );
}

#[test]
fn formatting_wraps_unwraps_and_retains_selection_direction() {
    for marker in ["**", "*", "`", "~~", "==", "%%"] {
        let (text, selected) = apply("café", 5, 0, Command::Wrap(marker));
        assert_eq!(text, format!("{marker}café{marker}"));
        assert_eq!(
            selected,
            Selection {
                anchor: marker.len() + 5,
                focus: marker.len()
            }
        );
        assert_eq!(
            apply(
                &text,
                selected.anchor,
                selected.focus,
                Command::Wrap(marker)
            ),
            (
                "café".into(),
                Selection {
                    anchor: 5,
                    focus: 0
                }
            )
        );
        assert_eq!(apply(&text, 0, text.len(), Command::Wrap(marker)).0, "café");
        let (pair, selected) = apply("", 0, 0, Command::Wrap(marker));
        assert_eq!(pair, marker.repeat(2));
        assert_eq!(
            selected,
            Selection {
                anchor: marker.len(),
                focus: marker.len()
            }
        );
        assert_eq!(
            apply(
                &pair,
                selected.anchor,
                selected.focus,
                Command::Wrap(marker)
            )
            .0,
            ""
        );
    }
}

#[test]
fn links_wrap_and_toggle_labels_or_whole_links_without_navigation() {
    let (text, sel) = apply("café", 0, 5, Command::Link);
    assert_eq!(text, "[café]()");
    assert_eq!(
        sel,
        Selection {
            anchor: 1,
            focus: 6
        }
    );
    assert_eq!(apply(&text, sel.anchor, sel.focus, Command::Link).0, "café");
    assert_eq!(apply("[café](url)", 0, 12, Command::Link).0, "café");
    assert_eq!(apply("[café](url)", 1, 6, Command::Link).0, "café");
    assert_eq!(
        apply("", 0, 0, Command::Link),
        (
            "[]()".into(),
            Selection {
                anchor: 1,
                focus: 1
            }
        )
    );
}

#[test]
fn legacy_line_commands_are_pure_edits_and_edge_moves_are_noops() {
    assert_eq!(
        apply("a\nb", 0, 0, Command::MoveLines { down: true }).0,
        "b\na"
    );
    assert_eq!(
        apply("a\nb", 2, 2, Command::MoveLines { down: false }).0,
        "b\na"
    );
    assert_eq!(at_end("a", Command::DuplicateLines), "a\na");
    assert_eq!(
        apply("abc abc", 0, 3, Command::SelectNext),
        (
            "abc abc".into(),
            Selection {
                anchor: 4,
                focus: 7
            }
        )
    );
    assert!(
        command(
            "",
            Selection {
                anchor: 0,
                focus: 0
            },
            Command::SelectNext
        )
        .is_none()
    );
    assert!(
        command(
            "a",
            Selection {
                anchor: 0,
                focus: 0
            },
            Command::MoveLines { down: false }
        )
        .is_none()
    );
}

#[test]
fn invalid_offsets_and_empty_delimiters_are_rejected() {
    for selection in [
        Selection {
            anchor: 1,
            focus: 0,
        },
        Selection {
            anchor: 0,
            focus: usize::MAX,
        },
    ] {
        assert!(command("é", selection, Command::Enter).is_none());
    }
    assert!(
        command(
            "",
            Selection {
                anchor: 0,
                focus: 0
            },
            Command::Wrap("")
        )
        .is_none()
    );
}
