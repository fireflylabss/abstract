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
mod tests {
    use super::*;

    fn labels(text: &str) -> Vec<(&str, bool)> {
        parse(text, |_| false)
            .into_iter()
            .map(|f| (&text[f.label], f.def))
            .collect()
    }

    #[test]
    fn references_and_definitions() {
        let text = "One[^1] and [^note].\n\n[^1]: First.\n   [^note]: Second [^1]:";
        assert_eq!(
            labels(text),
            [
                ("1", false),
                ("note", false),
                ("1", true),
                ("note", true),
                ("1", false)
            ]
        );
        let def = &parse(text, |_| false)[2];
        assert_eq!(&text[def.range.clone()], "[^1]:");
    }

    #[test]
    fn rejects_bad_labels_escapes_and_code() {
        assert!(labels("[^] [^a b] [^x \\[^y]").is_empty());
        let text = "`[^1]` [^2]";
        let code = |o: usize| o < 6;
        let found: Vec<&str> = parse(text, code)
            .iter()
            .map(|f| &text[f.label.clone()])
            .collect();
        assert_eq!(found, ["2"]);
    }

    #[test]
    fn insertion_numbers_and_appends() {
        let text = "Hello world.\n";
        let notes = parse(text, |_| false);
        let (r, new, caret) = insertion(text, &notes, 5);
        let mut out = text.to_string();
        out.replace_range(r.clone(), &new);
        assert_eq!(out, "Hello[^1] world.\n\n[^1]: ");
        assert_eq!(caret, out.len());

        let text = "A[^1] b[^7].\n\n[^1]: x\n[^7]: y\n";
        let notes = parse(text, |_| false);
        let (r, new, _) = insertion(text, &notes, 12);
        let mut out = text.to_string();
        out.replace_range(r, &new);
        assert_eq!(out, "A[^1] b[^7].[^8]\n\n[^1]: x\n[^7]: y\n[^8]: ");

        let (r, new, _) = insertion("", &[], 0);
        assert_eq!((r, new.as_str()), (0..0, "[^1]\n\n[^1]: "));
    }

    #[test]
    fn jumps_between_reference_and_definition() {
        let text = "See[^a].\n\n[^a]: Note.";
        let notes = parse(text, |_| false);
        assert_eq!(
            target(text, &notes, &notes[0]),
            Some(text.find("Note").unwrap())
        );
        assert_eq!(target(text, &notes, &notes[1]), Some(5));
        assert_eq!(
            target(
                "[^x]",
                &parse("[^x]", |_| false),
                &parse("[^x]", |_| false)[0]
            ),
            None
        );
    }
}
