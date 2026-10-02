//! GFM pipe tables: cell splitting and column alignment for the editor's
//! Tab navigation.

use std::ops::Range;

use unicode_width::UnicodeWidthStr;

/// Column alignment, from the delimiter row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// Byte ranges of the cells of `line`, between separator pipes (outer pipes
/// excluded, whitespace kept). As in GFM, only a `\`-escaped pipe doesn't
/// separate cells, even inside a code span.
fn cells(line: &str) -> Vec<Range<usize>> {
    let b = line.as_bytes();
    let mut seps = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'|' => seps.push(i),
            _ => {}
        }
        i += 1;
    }
    let mut out = Vec::new();
    let mut start = 0;
    for &s in &seps {
        out.push(start..s);
        start = s + 1;
    }
    out.push(start..b.len());
    if line.trim_start().starts_with('|') {
        out.remove(0);
    }
    let trailing = line.trim_end().ends_with('|')
        && seps.last().is_some_and(|s| line[s + 1..].trim().is_empty());
    if trailing && out.len() > 1 {
        out.pop();
    }
    out
}

fn trimmed(line: &str, r: Range<usize>) -> Range<usize> {
    let s = &line[r.clone()];
    let lead = s.len() - s.trim_start().len();
    r.start + lead..r.start + lead + s.trim().len()
}

fn align_of(cell: &str) -> Option<Align> {
    let c = cell.trim();
    let body = c.trim_start_matches(':').trim_end_matches(':');
    if body.is_empty() || !body.bytes().all(|b| b == b'-') {
        return None;
    }
    Some(match (c.starts_with(':'), c.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        (false, false) => Align::None,
    })
}

/// Byte ranges of the trimmed cell contents of `line`.
pub fn cell_spans(line: &str) -> Vec<Range<usize>> {
    cells(line).into_iter().map(|r| trimmed(line, r)).collect()
}

/// Per-column alignment read from delimiter row `line`.
pub fn aligns(line: &str) -> Vec<Align> {
    cells(line)
        .into_iter()
        .map(|r| align_of(&line[r]).unwrap_or(Align::None))
        .collect()
}

/// The table in `block` (rows separated by `\n`, the delimiter row second)
/// with every column padded to one width. `None` if `block` isn't a plain
/// table (e.g. it sits inside a block quote).
pub fn format(block: &str) -> Option<String> {
    let lines: Vec<&str> = block.split('\n').collect();
    if lines.len() < 2 || lines.iter().any(|l| l.trim_start().starts_with('>')) {
        return None;
    }
    let indent = &lines[0][..lines[0].len() - lines[0].trim_start().len()];
    let rows: Vec<Vec<&str>> = lines
        .iter()
        .map(|l| {
            cells(l)
                .into_iter()
                .map(|r| l[trimmed(l, r)].as_ref())
                .collect()
        })
        .collect();
    let align: Vec<Align> = rows[1].iter().map(|c| align_of(c)).collect::<Option<_>>()?;
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![3; ncols];
    for (i, row) in rows.iter().enumerate() {
        if i == 1 {
            continue;
        }
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(cell.width());
        }
    }
    let mut out = String::with_capacity(block.len() * 2);
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(indent);
        out.push('|');
        for (c, &w) in widths.iter().enumerate() {
            let a = align.get(c).copied().unwrap_or(Align::None);
            out.push(' ');
            if i == 1 {
                let dashes = w
                    - usize::from(matches!(a, Align::Left | Align::Right))
                    - 2 * usize::from(a == Align::Center);
                if matches!(a, Align::Left | Align::Center) {
                    out.push(':');
                }
                out.extend(std::iter::repeat_n('-', dashes));
                if matches!(a, Align::Right | Align::Center) {
                    out.push(':');
                }
            } else {
                let cell = row.get(c).copied().unwrap_or("");
                let pad = w - cell.width();
                let left = match a {
                    Align::Right => pad,
                    Align::Center => pad / 2,
                    _ => 0,
                };
                out.extend(std::iter::repeat_n(' ', left));
                out.push_str(cell);
                out.extend(std::iter::repeat_n(' ', pad - left));
            }
            out.push_str(" |");
        }
    }
    Some(out)
}

/// An empty row shaped like `formatted`'s header.
pub fn empty_row(formatted: &str) -> String {
    let header = formatted.split('\n').next().unwrap_or_default();
    let indent = &header[..header.len() - header.trim_start().len()];
    let mut out = format!("{indent}|");
    for r in cells(header) {
        out.extend(std::iter::repeat_n(' ', r.len()));
        out.push('|');
    }
    out
}

pub fn columns(line: &str) -> usize {
    cells(line).len()
}

/// Column of the cell containing byte `at` of `line`.
pub fn column_at(line: &str, at: usize) -> usize {
    let cs = cells(line);
    cs.iter()
        .position(|r| at <= r.end)
        .unwrap_or(cs.len())
        .min(cs.len().saturating_sub(1))
}

/// Where the caret goes in cell `col` of `line`: after its text, or one
/// space in when the cell is empty.
pub fn caret_in(line: &str, col: usize) -> usize {
    let Some(r) = cells(line).get(col).cloned() else {
        return line.len();
    };
    let t = trimmed(line, r.clone());
    if t.is_empty() {
        (r.start + 1).min(r.end)
    } else {
        t.end
    }
}

/// A new `cols`-column table with `header` labels, followed by one empty row.
pub fn template(header: &[String]) -> String {
    let head = format!("| {} |", header.join(" | "));
    let delim = format!("|{}", " --- |".repeat(header.len()));
    let formatted = format(&format!("{head}\n{delim}")).unwrap_or_default();
    let row = empty_row(&formatted);
    format!("{formatted}\n{row}\n")
}

/// Table-structure edit: insert or delete a row or column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    RowAbove,
    RowBelow,
    ColLeft,
    ColRight,
    DeleteRow,
    DeleteCol,
}

/// Outcome of [`edit`].
#[derive(Debug, PartialEq, Eq)]
pub enum Edit {
    /// Replacement block text; the caret goes to (line, column) of it,
    /// counting the delimiter row (line 0 = header, 1 = delimiter).
    Keep(String, usize, usize),
    /// The op removed the whole table.
    Removed,
}

/// Apply `op` at `(row, col)` — `row` counts block lines (0 header,
/// 1 delimiter) — rebuilding `block` through [`format`] so columns stay
/// aligned. `None` when `block` isn't a plain table.
pub fn edit(block: &str, row: usize, col: usize, op: Op) -> Option<Edit> {
    let lines: Vec<&str> = block.split('\n').collect();
    if lines.len() < 2 || lines.iter().any(|l| l.trim_start().starts_with('>')) {
        return None;
    }
    let indent = &lines[0][..lines[0].len() - lines[0].trim_start().len()];
    let mut aligns: Vec<Align> = cell_spans(lines[1])
        .into_iter()
        .map(|r| align_of(&lines[1][r]))
        .collect::<Option<_>>()?;
    // Rows without the delimiter line; index 0 is the header.
    let mut rows: Vec<Vec<String>> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, l)| {
            cell_spans(l)
                .into_iter()
                .map(|r| l[r].to_string())
                .collect()
        })
        .collect();
    let ncols = rows.iter().map(Vec::len).max()?.max(aligns.len());
    if ncols == 0 {
        return None;
    }
    for r in rows.iter_mut() {
        r.resize(ncols, String::new());
    }
    aligns.resize(ncols, Align::None);
    let col = col.min(ncols - 1);
    // `row` as a `rows` index (the delimiter line counts as the header).
    let rr = row.saturating_sub(1);
    let (rr, tc) = match op {
        Op::RowAbove | Op::RowBelow => {
            let at = (rr + usize::from(op == Op::RowBelow)).clamp(1, rows.len());
            rows.insert(at, vec![String::new(); ncols]);
            (at, col)
        }
        Op::ColLeft | Op::ColRight => {
            let at = col + usize::from(op == Op::ColRight);
            for r in rows.iter_mut() {
                r.insert(at, String::new());
            }
            aligns.insert(at, aligns[col]);
            (rr, at)
        }
        Op::DeleteRow => {
            // Header (or the delimiter line) and the last body row take the
            // whole table with them.
            if rr == 0 || rows.len() <= 2 {
                return Some(Edit::Removed);
            }
            rows.remove(rr);
            (rr.min(rows.len() - 1), col)
        }
        Op::DeleteCol => {
            if ncols <= 1 {
                return Some(Edit::Removed);
            }
            for r in rows.iter_mut() {
                r.remove(col);
            }
            aligns.remove(col);
            (rr, col.min(ncols - 2))
        }
    };
    let row_line = |cells: &[String]| format!("| {} |", cells.join(" | "));
    let mut loose = format!("{indent}{}", row_line(&rows[0]));
    loose.push_str("\n|");
    for a in &aligns {
        let d = match a {
            Align::Left => ":---",
            Align::Center => ":---:",
            Align::Right => "---:",
            Align::None => "---",
        };
        loose.push_str(&format!(" {d} |"));
    }
    for r in &rows[1..] {
        loose.push('\n');
        loose.push_str(&row_line(r));
    }
    let new = format(&loose)?;
    let line = if rr == 0 { 0 } else { rr + 1 };
    Some(Edit::Keep(new, line, tc))
}

#[cfg(test)]
mod tests {
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
        let mut b = crate::buffer::Buffer::new();
        b.set_text(doc.into());
        let Some(Edit::Keep(new, ..)) = edit(&doc[start..end], 2, 0, Op::RowBelow) else {
            panic!()
        };
        b.edit(start..end, &new, None);
        assert!(b.restore(true));
        assert_eq!(b.text(), doc);
    }
}
