use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::ops::Range;

use super::{Engine, Langs};
use crate::core::md::{Analyzer, Kind};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const ENTRY_BYTES: usize = 96;
const MAX_LINES: usize = MAX_BYTES / ENTRY_BYTES;

struct Entry {
    ranges: Vec<Range<usize>>,
    bytes: usize,
    stamp: u64,
}

#[derive(Default)]
pub struct Cache {
    lines: HashMap<u64, Entry>,
    order: BTreeMap<u64, u64>,
    bytes: usize,
    clock: u64,
    #[cfg(test)]
    misses: usize,
    #[cfg(test)]
    span_visits: usize,
}

impl Cache {
    fn evict(&mut self) {
        if let Some((_, key)) = self.order.pop_first()
            && let Some(entry) = self.lines.remove(&key)
        {
            self.bytes -= entry.bytes;
        }
    }
    pub fn check(&mut self, text: &str, engine: &Engine, langs: Langs) -> Vec<Range<usize>> {
        let analysis = Analyzer::new().analyze(text);
        let limit = analysis.lines.len().clamp(1, MAX_LINES);
        while self.lines.len() > limit {
            self.evict();
        }
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
                .chain(analysis.maths_overlapping(range).iter().map(|m| {
                    #[cfg(test)]
                    {
                        self.span_visits += 1;
                    }
                    &m.range
                }))
                .filter(|r| r.start < range.end && range.start < r.end)
            {
                (
                    target.start.saturating_sub(range.start),
                    target.end.min(range.end) - range.start,
                )
                    .hash(&mut hash);
            }
            let key = hash.finish();
            self.clock += 1;
            let bad = if let Some(entry) = self.lines.get_mut(&key) {
                self.order.remove(&entry.stamp);
                entry.stamp = self.clock;
                self.order.insert(entry.stamp, key);
                entry.ranges.clone()
            } else {
                #[cfg(test)]
                {
                    self.misses += 1;
                }
                let bad = super::scan_line(&analysis, text, ix, engine, langs);
                let bytes =
                    ENTRY_BYTES + range.len() + bad.len() * std::mem::size_of::<Range<usize>>();
                if bytes <= MAX_BYTES {
                    while self.lines.len() >= limit || self.bytes + bytes > MAX_BYTES {
                        self.evict();
                    }
                    self.bytes += bytes;
                    self.order.insert(self.clock, key);
                    self.lines.insert(
                        key,
                        Entry {
                            ranges: bad.clone(),
                            bytes,
                            stamp: self.clock,
                        },
                    );
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
