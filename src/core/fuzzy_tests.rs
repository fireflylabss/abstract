use super::*;

#[test]
fn canonically_equivalent_filenames_match_in_both_directions() {
    for (text, query) in [
        ("Café.md", "Cafe\u{301}"),
        ("Cafe\u{301}.md", "Café"),
        ("Ação.md", "Ac\u{327}a\u{303}o"),
    ] {
        assert!(score(text, query).is_some(), "{text}: {query}");
    }
}

#[test]
fn oversized_queries_do_not_visit_candidate_positions() {
    VISITS.with(|visits| visits.set(0));
    assert_eq!(score(&"x".repeat(128), &"x".repeat(4096)), None);
    assert_eq!(VISITS.with(|visits| visits.get()), 0);
}

#[test]
fn empty_missing_and_ordered_subsequences() {
    assert_eq!(score("", ""), Some(0));
    assert_eq!(score("anything", " "), Some(0));
    assert_eq!(score("", "x"), None);
    assert_eq!(score("abc", "ca"), None);
    assert!(score("Command palette", "cmdp").is_some());
    assert!(score("save note", "save") > score("s a v e note", "save"));
    assert!(score("save", "sa") > score("autosave", "sa"));
}

#[test]
fn unicode_expansions_accents_and_non_ascii_are_safe() {
    for (text, query) in [
        ("Ações", "aç"),
        ("İstanbul", "İst"),
        ("Straße", "strasse"),
        ("ΟΣ", "ος"),
        ("💡 Ideias", "💡id"),
    ] {
        assert!(score(text, query).is_some(), "{text}: {query}");
    }
    assert_eq!(score("ação", "🦀"), None);
}
