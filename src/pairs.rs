//! Pure typing helpers: auto-closing brackets/quotes and Tab/Shift-Tab list
//! indentation. Each returns `(range, replacement, selection)` for one
//! `Buffer::edit`, or `None` when the key keeps its plain behaviour.

use std::ops::Range;

pub type Edit = (Range<usize>, String, Range<usize>);

fn closer(open: char) -> Option<char> {
    Some(match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '"' => '"',
        '\'' => '\'',
        _ => return None,
    })
}

/// Typing `typed` over `sel`: an opener gains its closer (wrapping the
/// selection), and a closer typed right before the same closer steps over
/// it. Escaped (`\(`) characters and openers glued to the next word stay
/// plain; so does `'` after a letter (an apostrophe).
pub fn pair(text: &str, sel: Range<usize>, typed: char) -> Option<Edit> {
    let before = &text[..sel.start];
    if before.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1 {
        return None;
    }
    let next = text[sel.end..].chars().next();
    if sel.is_empty() && matches!(typed, ')' | ']' | '}' | '"' | '\'') && next == Some(typed) {
        let caret = sel.end + typed.len_utf8();
        return Some((sel.end..sel.end, String::new(), caret..caret));
    }
    let close = closer(typed)?;
    if sel.is_empty()
        && (typed == '\'' && before.chars().last().is_some_and(char::is_alphanumeric)
            || next.is_some_and(|c| {
                !c.is_whitespace() && !matches!(c, ')' | ']' | '}' | ',' | '.' | ':' | ';')
            }))
    {
        return None;
    }
    let n = typed.len_utf8();
    let new = format!("{typed}{}{close}", &text[sel.clone()]);
    Some((sel.clone(), new, sel.start + n..sel.end + n))
}

/// The caret sits between an opener and its closer (`(|)`).
pub fn inside_empty(text: &str, caret: usize) -> bool {
    let (Some(open), Some(close)) = (text[..caret].chars().last(), text[caret..].chars().next())
    else {
        return false;
    };
    closer(open) == Some(close)
}

fn is_list(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t', '>']);
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    ["- ", "* ", "+ "].iter().any(|m| rest.starts_with(m))
        || matches!(rest.trim_end(), "-" | "*" | "+")
        || digits > 0 && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") "))
}

/// Tab / Shift-Tab over `sel`: every touched line moves two spaces in or
/// out when the block has a list item (Shift-Tab also eats one tab).
/// `None` when the block has no list item or nothing would change.
pub fn indent(text: &str, sel: Range<usize>, outdent: bool) -> Option<Edit> {
    let start = text[..sel.start].rfind('\n').map_or(0, |i| i + 1);
    let last = if sel.is_empty() { sel.end } else { sel.end - 1 };
    let end = text[last..].find('\n').map_or(text.len(), |i| last + i);
    if !text[start..end].split('\n').any(is_list) {
        return None;
    }
    let mut new = String::new();
    let mut mapped = sel.clone();
    let mut at = start;
    let mut changed = false;
    for (i, line) in text[start..end].split('\n').enumerate() {
        if i > 0 {
            new.push('\n');
        }
        let (add, remove) = if !outdent {
            (2, 0)
        } else if line.starts_with('\t') {
            (0, 1)
        } else {
            (0, line.bytes().take_while(|b| *b == b' ').count().min(2))
        };
        new.push_str(&"  "[..add]);
        new.push_str(&line[remove..]);
        changed |= add + remove > 0;
        let shift = |pos: usize, out: &mut usize| {
            if pos >= at {
                *out = *out + add - remove.min(pos - at);
            }
        };
        shift(sel.start, &mut mapped.start);
        shift(sel.end, &mut mapped.end);
        at += line.len() + 1;
    }
    changed.then_some((start..end, new, mapped))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `|` marks the selection ends (one `|` = caret).
    fn parse(s: &str) -> (String, Range<usize>) {
        let a = s.find('|').unwrap();
        let rest = format!("{}{}", &s[..a], &s[a + 1..]);
        match rest[a..].find('|') {
            Some(b) => (
                format!("{}{}", &rest[..a + b], &rest[a + b + 1..]),
                a..a + b,
            ),
            None => (rest, a..a),
        }
    }

    fn show(text: &str, e: Edit) -> String {
        let mut out = format!("{}{}{}", &text[..e.0.start], e.1, &text[e.0.end..]);
        out.insert(e.2.end, '|');
        if !e.2.is_empty() {
            out.insert(e.2.start, '|');
        }
        out
    }

    fn typed(s: &str, c: char) -> Option<String> {
        let (text, sel) = parse(s);
        pair(&text, sel, c).map(|e| show(&text, e))
    }

    fn tab(s: &str, outdent: bool) -> Option<String> {
        let (text, sel) = parse(s);
        indent(&text, sel, outdent).map(|e| show(&text, e))
    }

    #[test]
    fn opener_gains_closer_and_wraps_selection() {
        assert_eq!(typed("a |", '(').as_deref(), Some("a (|)"));
        assert_eq!(typed("|", '[').as_deref(), Some("[|]"));
        assert_eq!(typed("[|]", '[').as_deref(), Some("[[|]]"));
        assert_eq!(typed("say |", '"').as_deref(), Some("say \"|\""));
        assert_eq!(typed("a |bç| c", '{').as_deref(), Some("a {|bç|} c"));
        assert_eq!(typed("f(|, x)", '(').as_deref(), Some("f((|), x)"));
    }

    #[test]
    fn closer_steps_over_itself() {
        assert_eq!(typed("(a|)", ')').as_deref(), Some("(a)|"));
        assert_eq!(typed("\"a|\"", '"').as_deref(), Some("\"a\"|"));
        assert_eq!(typed("(a|", ')'), None);
    }

    #[test]
    fn plain_when_glued_escaped_or_apostrophe() {
        assert_eq!(typed("|word", '('), None);
        assert_eq!(typed("\\|", '('), None);
        assert_eq!(typed("\\\\|", '(').as_deref(), Some("\\\\(|)"));
        assert_eq!(typed("don|", '\''), None);
        assert_eq!(typed("a|", 'x'), None);
    }

    #[test]
    fn inside_empty_pair() {
        assert!(inside_empty("a()", 2));
        assert!(inside_empty("\"\"", 1));
        assert!(!inside_empty("(a)", 1));
        assert!(!inside_empty("(", 1));
    }

    #[test]
    fn tab_indents_and_outdents_list_lines() {
        assert_eq!(tab("- a|", false).as_deref(), Some("  - a|"));
        assert_eq!(tab("  - a|", true).as_deref(), Some("- a|"));
        assert_eq!(tab("\t- a|", true).as_deref(), Some("- a|"));
        assert_eq!(tab(" 1. a|", true).as_deref(), Some("1. a|"));
        assert_eq!(tab("- [ ] a|", false).as_deref(), Some("  - [ ] a|"));
        assert_eq!(
            tab("x\n- |a\n- b|\ny", false).as_deref(),
            Some("x\n  - |a\n  - b|\ny")
        );
        assert_eq!(
            tab("  - a|\n  - b\n|", true).as_deref(),
            Some("- a|\n- b\n|")
        );
        assert_eq!(tab("| - a", true).as_deref(), Some("|- a"));
    }

    #[test]
    fn tab_leaves_prose_and_flush_lists_alone() {
        assert_eq!(tab("prose|", false), None);
        assert_eq!(tab("prose|", true), None);
        assert_eq!(tab("- a|", true), None);
    }
}
