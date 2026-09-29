//! Find/replace within one note: literal matches as byte ranges of the
//! original text, case-folded unless `case` is set.

use std::ops::Range;

/// Non-overlapping matches of `query` in `text`, left to right.
pub fn matches(text: &str, query: &str, case: bool) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    if case {
        return text
            .match_indices(query)
            .map(|(i, m)| i..i + m.len())
            .collect();
    }
    // Lowercasing can change byte lengths, so each lowered byte remembers
    // the original char it came from.
    let mut lower = String::with_capacity(text.len());
    let mut origin = Vec::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        let before = lower.len();
        lower.extend(c.to_lowercase());
        origin.resize(origin.len() + lower.len() - before, i);
    }
    let q: String = query.chars().flat_map(char::to_lowercase).collect();
    let start_of = |b: usize| b == 0 || origin[b - 1] != origin[b];
    let end_of = |b: usize| b == lower.len() || origin[b - 1] != origin[b];
    let orig_end = |b: usize| origin.get(b).copied().unwrap_or(text.len());
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(&q) {
        let (s, e) = (from + i, from + i + q.len());
        if start_of(s) && end_of(e) {
            out.push(origin[s]..orig_end(e));
            from = e;
        } else {
            from = s + lower[s..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Index of the first match starting at or after `at`, wrapping to the top.
pub fn nearest(matches: &[Range<usize>], at: usize) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let i = matches.partition_point(|m| m.start < at);
    Some(if i == matches.len() { 0 } else { i })
}

/// `text` with every match replaced by `with`.
pub fn replace_all(text: &str, matches: &[Range<usize>], with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in matches {
        out.push_str(&text[last..m.start]);
        out.push_str(with);
        last = m.end;
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found<'a>(text: &'a str, q: &str, case: bool) -> Vec<&'a str> {
        matches(text, q, case)
            .into_iter()
            .map(|r| &text[r])
            .collect()
    }

    #[test]
    fn folds_case_unless_asked() {
        assert_eq!(
            found("Note note NOTE", "note", false),
            ["Note", "note", "NOTE"]
        );
        assert_eq!(found("Note note NOTE", "note", true), ["note"]);
        assert!(matches("abc", "", false).is_empty());
    }

    #[test]
    fn ranges_index_the_original_text() {
        let text = "Ação e AÇÃO";
        assert_eq!(found(text, "ação", false), ["Ação", "AÇÃO"]);
        // 'İ' lowercases to two chars; a match can't split it.
        assert_eq!(found("İi", "i", false), ["i"]);
        assert_eq!(found("İi", "i̇", false), ["İ"]);
    }

    #[test]
    fn matches_do_not_overlap() {
        assert_eq!(matches("aaaa", "aa", false), [0..2, 2..4]);
    }

    #[test]
    fn nearest_wraps() {
        let m = [2..4, 8..10];
        assert_eq!(nearest(&m, 0), Some(0));
        assert_eq!(nearest(&m, 2), Some(0));
        assert_eq!(nearest(&m, 3), Some(1));
        assert_eq!(nearest(&m, 9), Some(0));
        assert_eq!(nearest(&[], 0), None);
    }

    #[test]
    fn replace_all_keeps_the_rest() {
        let text = "Cat cat scatter";
        let m = matches(text, "cat", false);
        assert_eq!(replace_all(text, &m, "dog"), "dog dog sdogter");
    }
}
