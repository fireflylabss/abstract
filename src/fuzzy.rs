//! Fuzzy subsequence matching for the palette: case- and accent-insensitive,
//! so `acao` finds `Ação` whether the file name is stored composed or
//! decomposed. Pure — no gpui.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

fn folded(text: &str) -> Vec<char> {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(|c| match c {
            'ß' | 'ẞ' => vec!['s', 's'],
            'ς' => vec!['σ'],
            _ => c.to_lowercase().collect(),
        })
        .filter(|c| !is_combining_mark(*c))
        .collect()
}

pub(crate) struct Query(Vec<char>);

impl Query {
    pub(crate) fn new(query: &str) -> Self {
        Self(folded(query.trim()))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// None when `text` does not contain the query's letters in order;
    /// otherwise higher is better: consecutive letters, word starts and
    /// matches near the beginning score more.
    pub(crate) fn score(&self, text: &str) -> Option<i64> {
        if self.0.is_empty() {
            return Some(0);
        }
        score_folded(&folded(text), &self.0)
    }
}

fn score_folded(text: &[char], query: &[char]) -> Option<i64> {
    if query.len() > text.len() {
        return None;
    }
    // previous[i]: best score with the last matched letter sitting at `i`.
    let mut previous: Vec<Option<i64>> = vec![None; text.len()];
    for (step, letter) in query.iter().enumerate() {
        let mut current = vec![None; text.len()];
        // Best `previous[j] + j` over j < i - 1: a gap costs its length.
        let mut best: Option<i64> = None;
        let mut matched = false;
        for (i, c) in text.iter().enumerate() {
            if i > 0
                && let Some(value) = previous[i - 1]
            {
                let v = value + i as i64 - 1;
                best = Some(best.map_or(v, |b| b.max(v)));
            }
            if c != letter {
                continue;
            }
            let boundary = if i == 0 || !text[i - 1].is_alphanumeric() {
                12
            } else {
                0
            };
            current[i] = if step == 0 {
                Some(40 + boundary - i as i64)
            } else {
                let spread = best.map(|v| v + 40 + boundary - i as i64);
                let adjacent = i
                    .checked_sub(1)
                    .and_then(|j| previous[j])
                    .map(|v| v + 60 + boundary);
                spread.into_iter().chain(adjacent).max()
            };
            matched |= current[i].is_some();
        }
        if !matched {
            return None;
        }
        previous = current;
    }
    previous.into_iter().flatten().max()
}

#[cfg(test)]
mod tests {
    use super::Query;

    fn score(text: &str, query: &str) -> Option<i64> {
        Query::new(query).score(text)
    }

    #[test]
    fn empty_missing_and_ordered_subsequences() {
        assert_eq!(score("", ""), Some(0));
        assert_eq!(score("anything", " "), Some(0));
        assert_eq!(score("", "x"), None);
        assert_eq!(score("abc", "ca"), None);
        assert_eq!(score("x", "xxxx"), None);
        assert!(score("Command palette", "cmdp").is_some());
        assert!(score("save note", "save") > score("s a v e note", "save"));
        assert!(score("save", "sa") > score("autosave", "sa"));
    }

    #[test]
    fn accents_case_and_normalization_forms_are_ignored() {
        for (text, query) in [
            ("Café", "Cafe\u{301}"),
            ("Cafe\u{301}", "Café"),
            ("Ação", "acao"),
            ("acao", "AÇÃO"),
            ("İstanbul", "ist"),
            ("Straße", "strasse"),
            ("ΟΣ", "ος"),
            ("💡 Ideias", "💡id"),
        ] {
            assert!(score(text, query).is_some(), "{text}: {query}");
        }
        assert_eq!(score("ação", "🦀"), None);
    }
}
