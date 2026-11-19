//! Footnotes: `[^label]` references and `[^label]: text` definitions.

use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footnote {
    /// `[^label]`, plus the `:` for a definition.
    pub range: Range<usize>,
    pub label: Range<usize>,
    pub def: bool,
}

/// References and definitions in buffer order. A definition starts its line
/// (up to three spaces in) and is followed by `:`. Labels are non-empty and
/// contain no whitespace or brackets. Offsets where `skip` is true (code)
/// don't start a footnote.
pub fn parse(text: &str, skip: impl Fn(usize) -> bool) -> Vec<Footnote> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 < b.len() {
        if b[i] != b'[' || b[i + 1] != b'^' || skip(i) || (i > 0 && b[i - 1] == b'\\') {
            i += 1;
            continue;
        }
        let start = i + 2;
        let len = text[start..]
            .find(|c: char| c == ']' || c == '[' || c.is_whitespace())
            .unwrap_or(text.len() - start);
        let close = start + len;
        if len == 0 || b.get(close) != Some(&b']') {
            i += 1;
            continue;
        }
        let line_start = text[..i].rfind('\n').map_or(0, |n| n + 1);
        let def = b.get(close + 1) == Some(&b':')
            && i - line_start <= 3
            && b[line_start..i].iter().all(|c| *c == b' ');
        out.push(Footnote {
            range: i..close + 1 + usize::from(def),
            label: start..close,
            def,
        });
        i = close + 1;
    }
    out
}

/// Inserting a new footnote at `at`: the edit range (`at..end`), its
/// replacement and the caret, which lands in the new definition. The label
/// is one past the highest numeric label.
pub fn insertion(text: &str, notes: &[Footnote], at: usize) -> (Range<usize>, String, usize) {
    let n = notes
        .iter()
        .filter_map(|f| text[f.label.clone()].parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let end = text.trim_end_matches('\n').len();
    let last_line = text[..end].rfind('\n').map_or(0, |i| i + 1);
    let after_def = notes.iter().any(|f| f.def && f.range.start >= last_line);
    let sep = if after_def { "\n" } else { "\n\n" };
    let kept = &text[at..end.max(at)];
    let new = format!("[^{n}]{kept}{sep}[^{n}]: ");
    let caret = at + new.len();
    (at..text.len(), new, caret)
}

/// Where Cmd/Ctrl+click on `f` goes: a reference jumps to its definition's
/// text, a definition to its first reference.
pub fn target(text: &str, notes: &[Footnote], f: &Footnote) -> Option<usize> {
    let label = &text[f.label.clone()];
    let other = notes
        .iter()
        .find(|o| o.def != f.def && text[o.label.clone()] == *label)?;
    Some(if other.def {
        other.range.end + usize::from(text.as_bytes().get(other.range.end) == Some(&b' '))
    } else {
        other.label.start
    })
}

#[cfg(test)]
mod tests;
