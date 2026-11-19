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
mod tests;
