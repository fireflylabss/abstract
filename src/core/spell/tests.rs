use super::*;
use crate::core::md::Analyzer;

fn engine() -> Engine {
    Engine {
        en: None,
        pt: None,
        user: RwLock::new(BTreeSet::new()),
        epoch: AtomicU32::new(0),
    }
}

/// Everything is "misspelled" without dictionaries — drives the zone
/// tests; with dicts, only real typos would surface.
fn bad_words(text: &str, ix: usize) -> Vec<String> {
    let mut an = Analyzer::new();
    let a = an.analyze(text);
    let eng = engine();
    scan_line(&a, text, ix, &eng, Langs::EN | Langs::PT)
        .iter()
        .map(|r| text[a.lines[ix].0.start + r.start..a.lines[ix].0.start + r.end].to_string())
        .collect()
}

#[test]
fn plain_words_flagged() {
    let t = "hello world";
    assert_eq!(bad_words(t, 0), vec!["hello", "world"]);
}

#[test]
fn code_spans_and_fences_skipped() {
    let t = "helo `wrng wordd` end\n\n```\nwrng codee\n```";
    assert_eq!(bad_words(t, 0), vec!["helo", "end"]);
    assert!(bad_words(t, 2).is_empty());
}

#[test]
fn urls_and_link_targets_skipped() {
    let t = "see https://exampel.com/x and [labell](https://tg-url.io) endd";
    assert_eq!(bad_words(t, 0), vec!["see", "and", "endd"]);
}

#[test]
fn wiki_link_target_skipped() {
    let t = "see [[noott targget]] andd [[ok|aliiass]] finee";
    assert_eq!(bad_words(t, 0), vec!["see", "andd", "finee"]);
}

#[test]
fn frontmatter_skipped() {
    let t = "---\ntitlee: badd\n---\nreal helo";
    assert!(bad_words(t, 0).is_empty());
    assert_eq!(bad_words(t, 3), vec!["real", "helo"]);
}

#[test]
fn digits_caps_camel_skipped() {
    let t = "abc123 HTTP camelCase iPhone helo";
    assert_eq!(bad_words(t, 0), vec!["helo"]);
}

#[test]
fn image_src_skipped() {
    // Image syntax (src and alt) is flagged MUTED/MARK, so it is never
    // checked — only plain words around it are.
    let t = "text ![[foto-bad.png]] andd ![altt](imagge.png) moree";
    assert_eq!(bad_words(t, 0), vec!["text", "andd", "moree"]);
}

#[test]
fn user_word_not_flagged() {
    let mut an = Analyzer::new();
    let t = "helo wrld";
    let a = an.analyze(t);
    let eng = engine();
    eng.add_user_word("helo");
    let bad = scan_line(&a, t, 0, &eng, Langs::EN | Langs::PT);
    assert_eq!(bad, vec![5..9]);
}

#[test]
fn lang_mask_limits_dicts() {
    // Without dicts everything flags; this asserts mask plumbing reaches
    // Engine::check (a word in `user` always passes regardless of mask).
    let eng = engine();
    eng.add_user_word("Quokka");
    assert!(eng.check("quokka", Langs::EN));
    assert!(eng.check("QUOKKA", Langs::PT));
}
