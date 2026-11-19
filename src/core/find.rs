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
        if matches!(c, 'ß' | 'ẞ') {
            lower.push_str("ss");
        } else {
            lower.extend(c.to_lowercase());
        }
        origin.resize(origin.len() + lower.len() - before, i);
    }
    let q = query.replace(['ß', 'ẞ'], "ss").to_lowercase();
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
mod tests;
