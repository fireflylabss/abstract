fn folded(text: &str) -> Vec<char> {
    text.chars()
        .flat_map(|c| match c {
            'ß' | 'ẞ' => "ss".chars().collect::<Vec<_>>(),
            'ς' => vec!['σ'],
            _ => c.to_lowercase().collect(),
        })
        .collect()
}

pub fn score(text: &str, query: &str) -> Option<i64> {
    let text = folded(text);
    let query = folded(query.trim());
    if query.is_empty() {
        return Some(0);
    }
    let mut previous = vec![None; text.len()];
    for (step, letter) in query.iter().enumerate() {
        let mut current = vec![None; text.len()];
        let mut best: Option<i64> = None;
        for (i, c) in text.iter().enumerate() {
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
        }
        previous = current;
    }
    previous.into_iter().flatten().max()
}

#[cfg(test)]
#[path = "fuzzy_tests.rs"]
mod tests;
