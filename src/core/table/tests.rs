use super::*;

#[test]
fn cell_spans_and_aligns() {
    let row = "| a  | b \\| c |  |";
    let spans: Vec<&str> = cell_spans(row).into_iter().map(|r| &row[r]).collect();
    assert_eq!(spans, ["a", "b \\| c", ""]);
    assert_eq!(
        aligns("|:--|:-:|--:|---|"),
        [Align::Left, Align::Center, Align::Right, Align::None]
    );
    assert_eq!(aligns("a | b"), [Align::None, Align::None]);
}

#[test]
fn pads_columns_and_keeps_alignment() {
    let t = "| a | long header | c |\n|:-|:-:|-:|\n| wide cell | x | 1 |";
    assert_eq!(
        format(t).unwrap(),
        "| a         | long header |   c |\n\
             | :-------- | :---------: | --: |\n\
             | wide cell |      x      |   1 |"
    );
}

#[test]
fn missing_cells_and_unicode_width() {
    let t = "a | ação\n--- | ---\n日本 |";
    assert_eq!(
        format(t).unwrap(),
        "| a    | ação |\n| ---- | ---- |\n| 日本 |      |"
    );
}

#[test]
fn escaped_pipes_stay_in_cells() {
    let t = r"| a \| b | `x\|y` |
| --- | --- |";
    assert_eq!(columns(t.lines().next().unwrap()), 2);
    assert_eq!(
        format(t).unwrap(),
        format!("{}\n| ------ | ------ |", t.lines().next().unwrap())
    );
}

#[test]
fn rejects_non_tables() {
    assert_eq!(format("| a |\n| b |"), None);
    assert_eq!(format("> | a |\n> | - |"), None);
}

#[test]
fn locates_cells() {
    let line = "| one | two |";
    assert_eq!(column_at(line, 0), 0);
    assert_eq!(column_at(line, 3), 0);
    assert_eq!(column_at(line, 8), 1);
    assert_eq!(column_at(line, line.len()), 1);
    assert_eq!(caret_in(line, 1), 11);
    let empty = "|     |     |";
    assert_eq!(caret_in(empty, 1), 8);
    assert_eq!(empty_row("| one | two |\n| --- | --- |"), empty);
}

#[test]
fn template_is_formatted() {
    assert_eq!(
        template(&["Column 1".into(), "Column 2".into()]),
        "| Column 1 | Column 2 |\n| -------- | -------- |\n|          |          |\n"
    );
}

fn cells_of(line: &str) -> Vec<String> {
    cell_spans(line)
        .into_iter()
        .map(|r| line[r].to_string())
        .collect()
}

#[test]
fn inserts_rows() {
    let t = "| a | b |\n| --- | --- |\n| 1 | 2 |";
    // Below the body row: an empty row lands under it.
    let Some(Edit::Keep(new, line, col)) = edit(t, 2, 1, Op::RowBelow) else {
        panic!()
    };
    let rows: Vec<&str> = new.split('\n').collect();
    assert_eq!(rows.len(), 4);
    assert_eq!((line, col), (3, 1));
    assert_eq!(rows[3], empty_row(&new));
    // Above lands right after the delimiter; from the header both
    // directions insert the first body row.
    let Some(Edit::Keep(new, line, _)) = edit(t, 2, 0, Op::RowAbove) else {
        panic!()
    };
    assert_eq!(line, 2);
    assert_eq!(new.split('\n').nth(2), Some(empty_row(&new).as_str()));
    for op in [Op::RowAbove, Op::RowBelow] {
        let Some(Edit::Keep(_, line, _)) = edit(t, 0, 0, op) else {
            panic!()
        };
        assert_eq!(line, 2);
    }
}

#[test]
fn inserts_columns_and_copies_alignment() {
    let t = "| a | b |\n| :- | -: |\n| 1 | 2 |";
    // Right of the first column: the new one borrows its alignment.
    let Some(Edit::Keep(new, line, col)) = edit(t, 0, 0, Op::ColRight) else {
        panic!()
    };
    let rows: Vec<&str> = new.split('\n').collect();
    assert_eq!(aligns(rows[1]), [Align::Left, Align::Left, Align::Right]);
    assert_eq!((line, col), (0, 1));
    assert_eq!(cells_of(rows[0]), ["a", "", "b"]);
    let Some(Edit::Keep(new, _, col)) = edit(t, 2, 1, Op::ColLeft) else {
        panic!()
    };
    assert_eq!(col, 1);
    let rows: Vec<&str> = new.split('\n').collect();
    assert_eq!(aligns(rows[1]), [Align::Left, Align::Right, Align::Right]);
    assert_eq!(cells_of(rows[2]), ["1", "", "2"]);
}

#[test]
fn deletes_rows_and_columns() {
    let t = "| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |";
    let Some(Edit::Keep(new, line, col)) = edit(t, 2, 1, Op::DeleteRow) else {
        panic!()
    };
    assert!(!new.contains('1'));
    assert_eq!((line, col), (2, 1));
    // The last row clamps the caret to the row above.
    let Some(Edit::Keep(_, line, _)) = edit(t, 3, 0, Op::DeleteRow) else {
        panic!()
    };
    assert_eq!(line, 2);
    // Header and delimiter rows take the whole table with them.
    assert_eq!(edit(t, 0, 0, Op::DeleteRow), Some(Edit::Removed));
    assert_eq!(edit(t, 1, 0, Op::DeleteRow), Some(Edit::Removed));
    // So does the last body row or column.
    let one = "| a |\n| - |\n| x |";
    assert_eq!(edit(one, 2, 0, Op::DeleteRow), Some(Edit::Removed));
    assert_eq!(edit(one, 2, 0, Op::DeleteCol), Some(Edit::Removed));
    let Some(Edit::Keep(new, _, col)) = edit(t, 2, 1, Op::DeleteCol) else {
        panic!()
    };
    assert_eq!(columns(new.split('\n').next().unwrap()), 1);
    assert_eq!(col, 0);
    assert_eq!(cells_of(new.split('\n').next().unwrap()), ["a"]);
}

#[test]
fn edits_keep_indent_and_escaped_pipes() {
    let t = "  | a \\| b | c |\n  | --- | --- |\n  | 1 | 2 |";
    let Some(Edit::Keep(new, ..)) = edit(t, 2, 0, Op::RowBelow) else {
        panic!()
    };
    assert!(new.lines().all(|l| l.starts_with("  |")));
    assert!(new.contains("a \\| b"));
}

#[test]
fn edits_fill_missing_cells() {
    let t = "| a | b |\n| - | - |\n| 1 |";
    let Some(Edit::Keep(new, ..)) = edit(t, 2, 0, Op::RowBelow) else {
        panic!()
    };
    assert_eq!(cells_of(new.split('\n').nth(2).unwrap()), ["1", ""]);
}

#[test]
fn edits_reject_non_tables() {
    assert!(edit("| a |\n| b |", 0, 0, Op::RowBelow).is_none());
    assert!(edit("> | a |\n> | - |", 0, 0, Op::DeleteRow).is_none());
}

#[test]
fn structure_edit_is_one_undo_step() {
    let doc = "intro\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\noutro";
    let start = doc.find("| a |").unwrap();
    let end = doc.find("\n\noutro").unwrap();
    let mut b = crate::core::buffer::Buffer::new();
    b.set_text(doc.into());
    let Some(Edit::Keep(new, ..)) = edit(&doc[start..end], 2, 0, Op::RowBelow) else {
        panic!()
    };
    b.edit(start..end, &new, None);
    assert!(b.restore(true));
    assert_eq!(b.text(), doc);
}
