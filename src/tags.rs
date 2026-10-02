//! `#tag` parsing, the per-space tag index (incremental, like `links.rs`),
//! and `#` autocompletion. Pure — no gpui — so it is testable headless.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A tag character after `#`: letters, digits, `_` or `-` (unicode-aware so
/// `#reunião` works).
pub(crate) fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// `#name` at a word boundary: the char before `#` is not a tag char or
/// `#` (heading syntax), and at least one tag char follows. Positions where
/// `skip(offset)` is true (code spans/blocks) are not tag starts. Returns the
/// names, `#` stripped.
pub(crate) fn parse(text: &str, skip: impl Fn(usize) -> bool) -> Vec<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    for (ix, &(b, c)) in chars.iter().enumerate() {
        if c != '#' || skip(b) {
            continue;
        }
        if ix > 0 {
            let p = chars[ix - 1].1;
            if is_tag_char(p) || p == '#' {
                continue;
            }
        }
        let mut end = ix + 1;
        while end < chars.len() && is_tag_char(chars[end].1) {
            end += 1;
        }
        if end == ix + 1 {
            continue;
        }
        let end_byte = chars.get(end).map_or(text.len(), |&(b, _)| b);
        out.push(text[b + 1..end_byte].to_string());
    }
    out
}

/// Tag names written in `text`, skipping code spans/blocks via `analyzer`.
fn tags_of(text: &str, analyzer: &mut crate::md::Analyzer) -> Vec<String> {
    let a = analyzer.analyze(text);
    parse(text, |o| {
        a.flags[o] & crate::md::CODE != 0
            || matches!(a.lines[a.line_of(o)].1, crate::md::Kind::Code)
    })
}

/// Per-note tag sets, re-read only when a file's mtime moves.
#[derive(Default)]
pub(crate) struct TagIndex {
    /// path → (mtime, tags as written).
    notes: HashMap<PathBuf, (SystemTime, Vec<String>)>,
}

/// A refreshed index: `(display name, note count)` sorted alphabetically —
/// display keeps the first casing seen — plus lowercased tag → note paths.
pub(crate) type TagMap = HashMap<String, HashSet<PathBuf>>;
type TagSummary = (Vec<(String, usize)>, TagMap);

impl TagIndex {
    /// Rescan stale `.md` files under `root` and return the summary.
    /// Blocking; runs on the background executor.
    pub(crate) fn refresh(&mut self, root: &Path) -> TagSummary {
        let mut files = Vec::new();
        crate::search::collect(root, &mut files);
        let live: HashSet<&Path> = files.iter().map(|(p, _)| p.as_path()).collect();
        self.notes.retain(|p, _| live.contains(p.as_path()));
        let mut analyzer = crate::md::Analyzer::new();
        for (path, modified) in &files {
            let fresh = self.notes.get(path).is_some_and(|(m, _)| m == modified);
            if fresh {
                continue;
            }
            match std::fs::read_to_string(path) {
                Ok(text) => {
                    self.notes
                        .insert(path.clone(), (*modified, tags_of(&text, &mut analyzer)));
                }
                Err(_) => {
                    self.notes.remove(path);
                }
            }
        }
        let mut display: HashMap<String, String> = HashMap::new();
        let mut paths: HashMap<String, HashSet<PathBuf>> = HashMap::new();
        for (p, (_, tags)) in &self.notes {
            let mut seen = HashSet::new();
            for t in tags {
                let key = t.to_lowercase();
                if seen.insert(key.clone()) {
                    display.entry(key.clone()).or_insert_with(|| t.clone());
                    paths.entry(key).or_default().insert(p.clone());
                }
            }
        }
        let mut tags: Vec<(String, usize)> = display
            .iter()
            .map(|(k, d)| (d.clone(), paths[k].len()))
            .collect();
        tags.sort_by_key(|a| a.0.to_lowercase());
        (tags, paths)
    }
}

/// Tag names starting with `prefix` (case-insensitive), sorted, max 8.
/// `tags` is the `(display name, count)` list [`TagIndex::refresh`] returns.
pub(crate) fn complete(tags: &[(String, usize)], prefix: &str) -> Vec<String> {
    let p = prefix.to_lowercase();
    let mut names: Vec<String> = tags
        .iter()
        .map(|(n, _)| n.clone())
        .filter(|n| n.to_lowercase().starts_with(&p))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.truncate(8);
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<String> {
        parse(text, |_| false)
    }

    #[test]
    fn parse_boundaries() {
        assert_eq!(names("#tag"), vec!["tag"]);
        assert_eq!(names("a #tag b"), vec!["tag"]);
        assert_eq!(names("(#tag)"), vec!["tag"]);
        assert_eq!(names("#a #b_c-d"), vec!["a", "b_c-d"]);
        // Not at a word boundary.
        assert!(names("foo#bar").is_empty());
        assert!(names("C#").is_empty());
        // Heading markers and lone `#` produce nothing.
        assert!(names("# Heading").is_empty());
        assert!(names("##tag").is_empty());
        assert!(names("a # tag").is_empty());
        // Unicode names.
        assert_eq!(names("#reunião"), vec!["reunião"]);
        // Trailing punctuation is not part of the name.
        assert_eq!(names("#tag, #tag."), vec!["tag", "tag"]);
        // Code positions are skipped by the callback.
        assert!(parse("a `#tag`", |o| o == 3).is_empty());
    }

    fn space(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("abstract-tags-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn index_counts_and_follows_edits() {
        let dir = space("index");
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        std::fs::write(&a, "# A\ntext #foo and `#code`\n").unwrap();
        std::fs::write(&b, "# B\n#foo #Bar\n```\n#nope\n```\n").unwrap();
        let mut index = TagIndex::default();
        let (tags, paths) = index.refresh(&dir);
        assert_eq!(tags, vec![("Bar".to_string(), 1), ("foo".to_string(), 2)]);
        assert_eq!(paths["foo"].len(), 2);
        assert_eq!(paths["bar"].len(), 1);
        assert!(!paths.contains_key("code"));
        assert!(!paths.contains_key("nope"));

        // Edit drops a tag and adds one.
        std::fs::write(&a, "# A\ntext #new\n").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&a)
            .unwrap()
            .set_modified(later)
            .unwrap();
        let (tags, paths) = index.refresh(&dir);
        assert_eq!(
            tags,
            vec![
                ("Bar".to_string(), 1),
                ("foo".to_string(), 1),
                ("new".to_string(), 1)
            ]
        );
        assert_eq!(paths["foo"].iter().next(), Some(&b));

        // Delete leaves a clean index.
        std::fs::remove_file(&a).unwrap();
        let (tags, _) = index.refresh(&dir);
        assert_eq!(tags, vec![("Bar".to_string(), 1), ("foo".to_string(), 1)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn complete_filters_by_prefix() {
        let tags = vec![
            ("alfa".to_string(), 2usize),
            ("beta".to_string(), 1),
            ("Alfa2".to_string(), 1),
        ];
        assert_eq!(complete(&tags, "al"), vec!["alfa", "Alfa2"]);
        assert_eq!(complete(&tags, "B"), vec!["beta"]);
        assert_eq!(complete(&tags, "z"), Vec::<String>::new());
        assert_eq!(complete(&tags, "").len(), 3);
    }
}
