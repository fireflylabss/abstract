use super::*;
use crate::core::md::Analyzer;

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
fn literal_at_math() {
    let a = analysis("x $a b$ y");
    assert!(literal_at(&a, 4, "x $a"));
    assert!(!literal_at(&a, 2, "x "));
    assert!(!literal_at(&a, 8, "x $a b$ "));
}

#[test]
fn literal_at_code_and_frontmatter() {
    // Fenced code block.
    let a = analysis("```\nfoo\n```");
    assert!(literal_at(&a, 5, "f"));
    let a = analysis("a `b c` d");
    // Inside the code span (between `b` and ` `).
    assert!(literal_at(&a, 4, "a `b"));
    // At the span's left edge and right after it: prose.
    assert!(!literal_at(&a, 2, "a "));
    assert!(!literal_at(&a, 7, "a `b c`"));
    assert!(!literal_at(&a, 8, "a `b c` "));
    // An unclosed span counts: typing `` `x --` `` linearly is code
    // even though the tree only flags the closed span.
    let a = analysis("`tail --");
    assert!(literal_at(&a, 5, "`tail"));
    assert!(literal_at(&a, 6, "`tail -"));
    // …and a closed one leaves prose outside.
    assert!(!literal_at(&a, 0, ""));
    let a = analysis("");
    assert!(literal_at(&a, 0, "`x"));
    assert!(!literal_at(&a, 0, "`x` "));
    assert!(literal_at(&a, 0, "``x `"));
    assert!(!literal_at(&a, 0, "\\`x "));
    // Frontmatter.
    let a = analysis("---\ntitle: x\n---\nbody");
    assert!(literal_at(&a, 8, "itle"));
    assert!(literal_at(&a, 16, "-"));
    assert!(!literal_at(&a, 18, ""));
    // Plain prose.
    let a = analysis("hello \"x\"");
    assert!(!literal_at(&a, 6, "hello "));
}
