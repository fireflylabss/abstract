use unicode_normalization::UnicodeNormalization;

#[cfg(test)]
thread_local! {
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn folded(text: &str) -> Vec<char> {
    text.nfc()
        .flat_map(|c| match c {
            'ß' | 'ẞ' => "ss".chars().collect::<Vec<_>>(),
            'ς' => vec!['σ'],
            _ => c.to_lowercase().collect(),
        })
        .nfc()
        .collect()
}

pub struct Query(Vec<char>);

impl Query {
    pub fn new(query: &str) -> Self {
        Self(folded(query.trim()))
    }

    pub fn score(&self, text: &str) -> Option<i64> {
        score_folded(&folded(text), &self.0)
    }
}

#[cfg(test)]
pub fn score(text: &str, query: &str) -> Option<i64> {
    Query::new(query).score(text)
}

fn score_folded(text: &[char], query: &[char]) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    if query.len() > text.len() {
        return None;
    }
    let mut previous = vec![None; text.len()];
    for (step, letter) in query.iter().enumerate() {
        let mut current = vec![None; text.len()];
        let mut best: Option<i64> = None;
        let mut matched = false;
        for (i, c) in text.iter().enumerate() {
            #[cfg(test)]
            VISITS.with(|visits| visits.set(visits.get() + 1));
            if i > 0
                && let Some(value) = previous[i - 1]
            {
                best = Some(best.map_or(value + i as i64 - 1, |b| b.max(value + i as i64 - 1)));
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
#[path = "fuzzy_tests.rs"]
mod tests;
