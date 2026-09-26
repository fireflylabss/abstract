//! Wiki-links `[[target]]` / `[[target|alias]]`: parsing (pure), resolving a
//! target against the note tree, autocompletion stems, and backlink lookup.
//! Pure — no gpui — so it is testable headless.

use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::vault::{self, NodeKind};

pub(crate) struct WikiLink {
    /// Whole `[[…]]` span.
    pub range: Range<usize>,
    /// Trimmed target text inside the brackets.
    pub target: Range<usize>,
    /// Trimmed alias after `|`, when present.
    pub alias: Option<Range<usize>>,
}

/// `[[target]]` and `[[target|alias]]`; no newline inside, target trimmed
/// non-empty. Positions where `skip(offset)` is true (code spans/blocks) are
/// not link starts.
pub(crate) fn parse(text: &str, skip: impl Fn(usize) -> bool) -> Vec<WikiLink> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' && !skip(i) {
            let mut j = i + 2;
            let mut bar = None;
            let mut end = None;
            while j < bytes.len() {
                match bytes[j] {
                    b'\n' => break,
                    b'|' if bar.is_none() => bar = Some(j),
                    b']' if bytes.get(j + 1) == Some(&b']') => {
                        end = Some(j);
                        break;
                    }
                    _ => {}
                }
                j += 1;
            }
            if let Some(close) = end {
                let (t_start, t_end) = trim_range(text, i + 2, bar.unwrap_or(close));
                if t_start < t_end {
                    let alias = bar.and_then(|b| {
                        let (a, z) = trim_range(text, b + 1, close);
                        (a < z).then_some(a..z)
                    });
                    out.push(WikiLink {
                        range: i..close + 2,
                        target: t_start..t_end,
                        alias,
                    });
                }
                i = close + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn trim_range(text: &str, mut a: usize, mut b: usize) -> (usize, usize) {
    let is_ws = |i: usize| text.as_bytes()[i].is_ascii_whitespace();
    while a < b && is_ws(a) {
        a += 1;
    }
    while b > a && is_ws(b - 1) {
        b -= 1;
    }
    (a, b)
}

/// Names that refer to a note: its file stem and its `# title`, lowercased
/// and trimmed.
pub(crate) fn names_of(path: &Path, text: &str) -> Vec<String> {
    let mut names = vec![
        path.file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
    ];
    let title = crate::search::file_title(path, text).to_lowercase();
    if !title.is_empty() && title != names[0] {
        names.push(title);
    }
    names
}

fn note_leaves<'a>(tree: &'a [vault::Node], out: &mut Vec<&'a vault::Node>) {
    for n in tree {
        match n.kind {
            NodeKind::Note => out.push(n),
            NodeKind::Folder => note_leaves(&n.children, out),
        }
    }
}

/// First note whose stem == `target`, else whose `# title` == `target`
/// (case-insensitive, trimmed).
pub(crate) fn resolve(tree: &[vault::Node], target: &str) -> Option<PathBuf> {
    let t = target.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    let mut notes = Vec::new();
    note_leaves(tree, &mut notes);
    if let Some(n) = notes.iter().find(|n| n.name.to_lowercase() == t) {
        return Some(n.path.clone());
    }
    notes.iter().find_map(|n| {
        let text = std::fs::read_to_string(&n.path).ok()?;
        (crate::search::file_title(&n.path, &text).to_lowercase() == t).then_some(n.path.clone())
    })
}

/// Notes (excluding `note`) containing a `[[link]]` whose lowercased, trimmed
/// target is in `names`. Returns (path, title) sorted by title.
pub(crate) fn backlinks(root: &Path, note: &Path, names: &[String]) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    crate::search::collect(root, &mut files);
    let mut out = Vec::new();
    for (path, _) in files {
        if path == note {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let hit = parse(&text, |_| false)
            .into_iter()
            .any(|l| names.contains(&text[l.target].trim().to_lowercase()));
        if hit {
            out.push((path.clone(), crate::search::file_title(&path, &text)));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Note stems starting with `prefix` (case-insensitive), sorted, max 8.
/// Empty prefix → the 8 most recently modified notes.
pub(crate) fn complete(tree: &[vault::Node], prefix: &str) -> Vec<String> {
    let mut notes = Vec::new();
    note_leaves(tree, &mut notes);
    let p = prefix.trim().to_lowercase();
    if p.is_empty() {
        notes.sort_by_key(|n| std::cmp::Reverse(n.modified));
        return notes.into_iter().take(8).map(|n| n.name.clone()).collect();
    }
    let mut stems: Vec<String> = notes
        .iter()
        .map(|n| n.name.clone())
        .filter(|s| s.to_lowercase().starts_with(&p))
        .collect();
    stems.sort();
    stems.dedup();
    stems.truncate(8);
    stems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Node;
    use std::time::SystemTime;

    fn space(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("abstract-links-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_forms() {
        let l = parse("see [[Foo]] ok", |_| false);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].range, 4..11);
        assert_eq!(&"see [[Foo]] ok"[l[0].target.clone()], "Foo");
        assert!(l[0].alias.is_none());

        let l = parse("a [[Foo | bar baz]]", |_| false);
        assert_eq!(&"a [[Foo | bar baz]]"[l[0].target.clone()], "Foo");
        assert_eq!(
            &"a [[Foo | bar baz]]"[l[0].alias.clone().unwrap()],
            "bar baz"
        );

        // Unclosed, multiline and empty-target are rejected.
        assert!(parse("a [[Foo", |_| false).is_empty());
        assert!(parse("a [[Foo\nbar]]", |_| false).is_empty());
        assert!(parse("a [[  ]]", |_| false).is_empty());
        // Skip callback (code spans) suppresses the link.
        assert!(parse("a [[Foo]]", |o| o == 2).is_empty());
        // Alias-only-empty is fine: `[[x|]]` → target link, no alias.
        let l = parse("a [[x|]]", |_| false);
        assert_eq!(l.len(), 1);
        assert!(l[0].alias.is_none());
    }

    fn node(path: PathBuf, name: &str, kind: NodeKind, modified: u64) -> Node {
        Node {
            path,
            name: name.into(),
            kind,
            modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(modified),
            children: Vec::new(),
        }
    }

    #[test]
    fn resolve_by_stem_and_title() {
        let dir = space("resolve");
        let a = dir.join("alpha.md");
        std::fs::write(&a, "# Nota Alfa\nbody").unwrap();
        let tree = vec![node(a.clone(), "alpha", NodeKind::Note, 1)];
        assert_eq!(resolve(&tree, "alpha"), Some(a.clone()));
        assert_eq!(resolve(&tree, "ALPHA"), Some(a.clone()));
        assert_eq!(resolve(&tree, "nota alfa"), Some(a));
        assert_eq!(resolve(&tree, "alphabet"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn backlinks_finds_referrers() {
        let dir = space("backlinks");
        let alpha = dir.join("alpha.md");
        std::fs::write(&alpha, "# Alpha\n").unwrap();
        std::fs::write(dir.join("beta.md"), "# Beta\nsee [[Alpha]]\n").unwrap();
        std::fs::write(dir.join("gamma.md"), "# Gamma\n[[alpha|see]] here\n").unwrap();
        std::fs::write(dir.join("delta.md"), "# Delta\n[[alphabet]] no\n").unwrap();
        let names = names_of(&alpha, "# Alpha\n");
        let mut titles: Vec<String> = backlinks(&dir, &alpha, &names)
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        titles.sort();
        assert_eq!(titles, vec!["Beta".to_string(), "Gamma".to_string()]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn complete_filters_by_prefix() {
        let dir = space("complete");
        let g = dir.join("gamma.md");
        let ga = dir.join("gala.md");
        let b = dir.join("beta.md");
        let tree = vec![
            node(g.clone(), "gamma", NodeKind::Note, 10),
            node(ga.clone(), "gala", NodeKind::Note, 5),
            node(b.clone(), "beta", NodeKind::Note, 1),
        ];
        assert_eq!(complete(&tree, "g"), vec!["gala", "gamma"]);
        assert_eq!(complete(&tree, "GA"), vec!["gala", "gamma"]);
        assert_eq!(complete(&tree, "gam"), vec!["gamma"]);
        assert!(complete(&tree, "z").is_empty());
        // Empty prefix → most recently modified first.
        let recent = complete(&tree, "");
        assert_eq!(recent[0], "gamma");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_of_includes_stem_and_title() {
        let dir = space("names");
        let p = dir.join("alpha.md");
        assert_eq!(names_of(&p, "# Nota Alfa\n"), vec!["alpha", "nota alfa"]);
        // Title equal to the stem yields a single name.
        assert_eq!(names_of(&p, "# Alpha\n"), vec!["alpha"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
