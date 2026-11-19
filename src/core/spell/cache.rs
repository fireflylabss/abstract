use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use super::{Engine, Langs};
use crate::core::md::{Analyzer, Kind};

const MAX_LINES: usize = 2048;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Default)]
pub struct Cache {
    lines: HashMap<u64, Vec<Range<usize>>>,
    bytes: usize,
}

impl Cache {
    pub fn check(&mut self, text: &str, engine: &Engine, langs: Langs) -> Vec<Range<usize>> {
        let analysis = Analyzer::new().analyze(text);
        let mut out = vec![];
        for (ix, (range, kind)) in analysis.lines.iter().enumerate() {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            super::line_key(&text[range.clone()], langs, engine.epoch()).hash(&mut hash);
            analysis.flags[range.clone()].hash(&mut hash);
            (*kind == Kind::Code).hash(&mut hash);
            for target in analysis
                .wiki_links
                .iter()
                .map(|l| &l.range)
                .chain(analysis.images.iter().map(|i| &i.src))
                .filter(|r| r.start < range.end && range.start < r.end)
            {
                (
                    target.start.saturating_sub(range.start),
                    target.end.min(range.end) - range.start,
                )
                    .hash(&mut hash);
            }
            let key = hash.finish();
            let bad = if let Some(bad) = self.lines.get(&key) {
                bad.clone()
            } else {
                let bad = super::scan_line(&analysis, text, ix, engine, langs);
                let bytes = range.len() + bad.len() * std::mem::size_of::<Range<usize>>();
                if self.lines.len() >= MAX_LINES || self.bytes + bytes > MAX_BYTES {
                    self.lines.clear();
                    self.bytes = 0;
                }
                if bytes <= MAX_BYTES {
                    self.bytes += bytes;
                    self.lines.insert(key, bad.clone());
                }
                bad
            };
            out.extend(
                bad.into_iter()
                    .map(|r| range.start + r.start..range.start + r.end),
            );
        }
        out
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
