//! Typographic substitution while typing: curly quotes and the `--` → `—`
//! fold, Word-style. Pure decisions — `LiveEditor` calls `substitute` on
//! the typed-text path only (paste never converts) and `literal_at` is the
//! single funnel for forbidden contexts (code, frontmatter; math plugs in
//! here when `$…$` lands).

use crate::core::md::{Analysis, CODE, Kind};

/// One substitution: `text` replaces the typed char plus `back` chars
/// before the caret; `orig` is what a Backspace right after restores.
#[derive(Debug, PartialEq, Eq)]
pub struct Subst {
    pub text: &'static str,
    pub orig: &'static str,
    pub back: usize,
}

/// What `typed` becomes at the caret. `line` is the whole current line and
/// `before` its prefix up to the caret. `on` is the setting, `literal` the
/// forbidden-context flag from `literal_at`.
pub fn substitute(line: &str, before: &str, typed: char, on: bool, literal: bool) -> Option<Subst> {
    if !on || literal {
        return None;
    }
    // Opens at line start or after space, `(`, `[`, `{`, `—`, `–`;
    // everything else closes. For `'` the "everything else" arm covers the
    // apostrophe rule (after a letter or digit → ’).
    let opens = |c: Option<char>| {
        c.is_none_or(|c| c == ' ' || c == '\t' || matches!(c, '(' | '[' | '{' | '—' | '–'))
    };
    let prev = before.chars().last();
    match typed {
        '"' => Some(Subst {
            text: if opens(prev) { "“" } else { "”" },
            orig: "\"",
            back: 0,
        }),
        '\'' => Some(Subst {
            text: if opens(prev) { "‘" } else { "’" },
            orig: "'",
            back: 0,
        }),
        '-' => {
            // `---` (rule, frontmatter) and `|---|` delimiters stay literal:
            // no fold when the line so far is only hyphens/spaces or the
            // line is a table row.
            if prev != Some('-')
                || line.contains('|')
                || before.chars().all(|c| c == '-' || c == ' ')
            {
                return None;
            }
            Some(Subst {
                text: "—",
                orig: "--",
                back: 1,
            })
        }
        _ => None,
    }
}

/// Whether `off` sits in a context that must stay literal: fenced or
/// indented code block, `code` span, frontmatter. `before` is the line
/// prefix up to `off`, so an as-yet-unclosed `code` span counts too —
/// the tree only flags closed spans, but someone typing `` `x --` ``
/// linearly is already inside one. Closed math (`$…$`) is literal too.
pub fn literal_at(a: &Analysis, off: usize, before: &str) -> bool {
    if matches!(a.lines[a.line_of(off)].1, Kind::Code) {
        return true;
    }
    if a.metadata.iter().any(|r| r.start < off && off <= r.end) {
        return true;
    }
    if a.maths
        .iter()
        .any(|m| m.range.start < off && off < m.range.end)
    {
        return true;
    }
    // Inside a closed code span is strictly between two CODE-flagged
    // bytes; a caret right at either edge is still prose.
    (off > 0
        && a.flags.get(off - 1).is_some_and(|f| f & CODE != 0)
        && a.flags.get(off).is_some_and(|f| f & CODE != 0))
        || unclosed_span(before)
}

/// Is `before` inside an unclosed backtick run? A run of `n` opens a
/// span that a run of exactly `n` closes; `\`-escaped runs don't count.
fn unclosed_span(before: &str) -> bool {
    let mut open: Vec<usize> = Vec::new();
    let bytes = before.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' && (i == 0 || bytes[i - 1] != b'\\') {
            let mut n = 1;
            while i + n < bytes.len() && bytes[i + n] == b'`' {
                n += 1;
            }
            if open.last() == Some(&n) {
                open.pop();
            } else {
                open.push(n);
            }
            i += n;
        } else {
            i += 1;
        }
    }
    !open.is_empty()
}

#[cfg(test)]
mod tests;
