use super::*;

#[test]
fn greek_sigma_folds_independently_of_word_position() {
    let source = "ΟΣ ος οσ";
    for query in ["ΟΣ", "ος", "οσ"] {
        assert_eq!(found(source, query, false), ["ΟΣ", "ος", "οσ"]);
    }
    assert_eq!(found(source, "ΟΣ", true), ["ΟΣ"]);
}

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
    assert_eq!(
        found("Straße STRAẞE", "strasse", false),
        ["Straße", "STRAẞE"]
    );
    assert!(matches("ß", "s", false).is_empty());
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
