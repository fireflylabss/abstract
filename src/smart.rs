//! Typographic substitution while typing: curly quotes and the `--` → `—`
//! fold, Word-style. Pure decisions — `LiveEditor` calls `substitute` on
//! the typed-text path only (paste never converts) and `literal_at` is the
//! single funnel for forbidden contexts (code, frontmatter; math plugs in
//! here when `$…$` lands).

use crate::md::{Analysis, CODE, Kind};

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
/// indented code block, `code` span, frontmatter. Extend here when math
/// (`$…$`) lands — callers stay unchanged.
pub fn literal_at(a: &Analysis, off: usize) -> bool {
    if matches!(a.lines[a.line_of(off)].1, Kind::Code) {
        return true;
    }
    if a.metadata.iter().any(|r| r.start < off && off <= r.end) {
        return true;
    }
    // Inside a code span is strictly between two CODE-flagged bytes; a
    // caret right at either edge is still prose.
    off > 0
        && a.flags.get(off - 1).is_some_and(|f| f & CODE != 0)
        && a.flags.get(off).is_some_and(|f| f & CODE != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::md::Analyzer;

    fn sub(line: &str, before: &str, typed: char) -> Option<String> {
        substitute(line, before, typed, true, false).map(|s| s.text.to_string())
    }

    fn analysis(text: &str) -> Analysis {
        Analyzer::new().analyze(text)
    }

    #[test]
    fn double_quote_opens_and_closes() {
        assert_eq!(sub("", "", '"').unwrap(), "“");
        assert_eq!(sub("diga ", "diga ", '"').unwrap(), "“");
        for open in ['(', '[', '{', '—', '–'] {
            let b = format!("a{open}");
            assert_eq!(sub(&b, &b, '"').unwrap(), "“", "after {open}");
        }
        assert_eq!(sub("diga", "diga", '"').unwrap(), "”");
        assert_eq!(sub("n = 2", "n = 2", '"').unwrap(), "”");
        assert_eq!(sub("a)b", "a)", '"').unwrap(), "”");
    }

    #[test]
    fn single_quote_and_apostrophe() {
        assert_eq!(sub("", "", '\'').unwrap(), "‘");
        assert_eq!(sub("a ", "a ", '\'').unwrap(), "‘");
        // After a letter or digit it's always the apostrophe ’.
        assert_eq!(sub("don't stop", "dont", '\'').unwrap(), "’");
        assert_eq!(sub("d'água", "d", '\'').unwrap(), "’");
        assert_eq!(sub("x'80", "x'8", '\'').unwrap(), "’");
        assert_eq!(sub("say 'hi'", "say 'hi", '\'').unwrap(), "’");
    }

    #[test]
    fn double_hyphen_folds_to_dash() {
        let s = substitute("era uma vez --", "era uma vez -", '-', true, false).unwrap();
        assert_eq!(s.text, "—");
        assert_eq!(s.orig, "--");
        assert_eq!(s.back, 1);
        // A lone `-` does nothing.
        assert!(substitute("a -", "a ", '-', true, false).is_none());
        // `a--` folds too.
        assert_eq!(sub("a--", "a-", '-').unwrap(), "—");
    }

    #[test]
    fn hyphen_rules_and_tables_stay_literal() {
        // A `---` in the making (rule / frontmatter) never folds.
        assert!(substitute("-", "-", '-', true, false).is_none());
        assert!(substitute("--", "--", '-', true, false).is_none());
        assert!(substitute(" - - ", " - -", '-', true, false).is_none());
        // Any `|` in the line keeps `|` delimiters intact.
        assert!(substitute("|---|", "|-", '-', true, false).is_none());
        assert!(substitute("a | b --", "a | b -", '-', true, false).is_none());
    }

    #[test]
    fn off_or_literal_never_substitutes() {
        // Toggle off.
        assert!(substitute("a ", "a ", '"', false, false).is_none());
        assert!(substitute("a --", "a -", '-', false, false).is_none());
        // Forbidden context.
        assert!(substitute("a ", "a ", '"', true, true).is_none());
        assert!(substitute("a ", "a ", '\'', true, true).is_none());
        assert!(substitute("a --", "a -", '-', true, true).is_none());
        // Other chars never substitute.
        assert!(substitute("a ", "a ", 'x', true, false).is_none());
    }

    #[test]
    fn literal_at_code_and_frontmatter() {
        // Fenced code block.
        let a = analysis("```\nfoo\n```");
        assert!(literal_at(&a, 5));
        let a = analysis("a `b c` d");
        // Inside the code span (between `b` and ` `).
        assert!(literal_at(&a, 4));
        // At the span's left edge and right after it: prose.
        assert!(!literal_at(&a, 2));
        assert!(!literal_at(&a, 7));
        assert!(!literal_at(&a, 8));
        // Frontmatter.
        let a = analysis("---\ntitle: x\n---\nbody");
        assert!(literal_at(&a, 8));
        assert!(literal_at(&a, 16));
        assert!(!literal_at(&a, 18));
        // Plain prose.
        let a = analysis("hello \"x\"");
        assert!(!literal_at(&a, 6));
    }
}
