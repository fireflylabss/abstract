//! Line-grained editing: move or duplicate the lines a selection touches,
//! and select the word under the caret or the next occurrence of the
//! selection. Pure text ops — the editor applies the returned
//! `(range, replacement, selection)` in a single undoable step.

use std::ops::Range;

/// Byte ranges of `text`'s lines, line endings excluded. A trailing
/// newline leaves a final empty line.
fn lines(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            let end = if i > start && text.as_bytes()[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            out.push(start..end);
            start = i + 1;
        }
    }
    out.push(start..text.len());
    out
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Index of the line containing `off` — `Analysis::line_of`'s rule.
fn line_of(lines: &[Range<usize>], off: usize) -> usize {
    lines.partition_point(|r| r.start <= off).saturating_sub(1)
}

/// Lines `sel` touches, as line indices; a selection ending at a line
/// start leaves that line out (`LiveEditor::selected_lines`'s rule).
fn touched(lines: &[Range<usize>], sel: &Range<usize>) -> Range<usize> {
    let first = line_of(lines, sel.start);
    let last = line_of(
        lines,
        if sel.end > sel.start {
            sel.end - 1
        } else {
            sel.end
        },
    );
    first..last + 1
}

/// `(range, replacement, selection)` moving the lines `sel` touches one
/// line `down` (or up). `None` against the document's edge.
pub fn move_sel(
    text: &str,
    sel: &Range<usize>,
    down: bool,
) -> Option<(Range<usize>, String, Range<usize>)> {
    let lines = lines(text);
    let b = touched(&lines, sel);
    let (first, last) = (lines[b.start].clone(), lines[b.end - 1].clone());
    let block = &text[first.start..last.end];
    let newline = newline(text);
    if down {
        let next = lines.get(b.end)?.clone();
        // A trailing '\n' leaves a final empty line — moving onto it would
        // look like inserting a blank line above the last visible one.
        if next.start == text.len() {
            return None;
        }
        let shift = next.len() + newline.len();
        let end = first.start + shift + block.len();
        let sel = (sel.start + shift).min(end)..(sel.end + shift).min(end);
        Some((
            first.start..next.end,
            format!("{}{newline}{block}", &text[next]),
            sel,
        ))
    } else {
        let prev = b.start.checked_sub(1).map(|i| lines[i].clone())?;
        let shift = first.start - prev.start;
        Some((
            prev.start..last.end,
            format!("{block}{newline}{}", &text[prev]),
            sel.start - shift..sel.end - shift,
        ))
    }
}

/// `(range, replacement, selection)` copying the lines `sel` touches right
/// below themselves — an insert at `last.end` selecting the copy.
pub fn duplicate_sel(text: &str, sel: &Range<usize>) -> (Range<usize>, String, Range<usize>) {
    let lines = lines(text);
    let b = touched(&lines, sel);
    let (first, last) = (lines[b.start].clone(), lines[b.end - 1].clone());
    let block = &text[first.start..last.end];
    let newline = newline(text);
    let copy_end = last.end + newline.len() + block.len();
    let shift = last.end + newline.len() - first.start;
    (
        last.end..last.end,
        format!("{newline}{block}"),
        (sel.start + shift).min(copy_end)..(sel.end + shift).min(copy_end),
    )
}

/// The `is_alphanumeric` run containing `off`; inside a run of non-word
/// chars, the next word forward. `None` when no word exists at/after it.
fn word_at(text: &str, off: usize) -> Option<Range<usize>> {
    let start = text[..off]
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphanumeric())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let end = off
        + text[off..]
            .find(|c: char| !c.is_alphanumeric())
            .unwrap_or(text.len() - off);
    if start < end {
        return Some(start..end);
    }
    let s = off + text[off..].find(|c: char| c.is_alphanumeric())?;
    let e = s + text[s..]
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(text.len() - s);
    Some(s..e)
}

/// Cmd-D target: the word at a collapsed caret, else the next
/// case-sensitive occurrence of the selected text — after it, wrapping to
/// the top. The only occurrence selects itself.
pub fn select_next(text: &str, sel: &Range<usize>) -> Option<Range<usize>> {
    if sel.is_empty() {
        return word_at(text, sel.start);
    }
    let needle = &text[sel.clone()];
    let at = text[sel.end..]
        .find(needle)
        .map(|i| sel.end + i)
        .or_else(|| text[..sel.end].find(needle))?;
    Some(at..at + needle.len())
}

#[cfg(test)]
mod tests;
