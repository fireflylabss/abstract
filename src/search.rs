//! Global note search: case-insensitive substring matching over every `.md`
//! under the space root. Pure — no gpui — so it is testable headless. The
//! query side runs on the background executor; an empty query doubles as a
//! "recent files" list.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) struct Hit {
    pub path: PathBuf,
    pub title: String,
    /// 1-based line of the first body match (0 for title-only / recent hits).
    pub line: usize,
    pub snippet: String,
    pub score: u32,
}

/// Case-insensitive substring match, byte-offset safe: compares chars one by
/// one so multi-char lowercases (`İ`) can never produce a non-boundary slice.
pub(crate) fn match_range(hay: &str, needle_lower: &str) -> Option<Range<usize>> {
    if needle_lower.is_empty() {
        return Some(0..0);
    }
    let needle: Vec<char> = needle_lower.chars().collect();
    let chars: Vec<(usize, char)> = hay.char_indices().collect();
    'outer: for w in 0..chars.len() {
        if w + needle.len() > chars.len() {
            break;
        }
        for (k, &nc) in needle.iter().enumerate() {
            let mut lower = chars[w + k].1.to_lowercase();
            if lower.next() != Some(nc) || lower.next().is_some() {
                continue 'outer;
            }
        }
        let start = chars[w].0;
        let end = chars
            .get(w + needle.len())
            .map(|&(i, _)| i)
            .unwrap_or(hay.len());
        return Some(start..end);
    }
    None
}

/// First `# heading`, else the file stem — mirrors `title_of`'s semantics.
pub(crate) fn file_title(path: &Path, text: &str) -> String {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(80).collect())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// Trim the line and window it to ≤120 chars centered on the match.
fn snippet_of(line: &str, needle_lower: &str) -> String {
    let t = line.trim();
    if t.chars().count() <= 120 {
        return t.to_string();
    }
    let Some(r) = match_range(t, needle_lower) else {
        return t.chars().take(120).collect();
    };
    let match_char = t[..r.start].chars().count();
    let start_char = match_char.saturating_sub(60);
    let start_byte = t.char_indices().nth(start_char).map_or(0, |(i, _)| i);
    let s = &t[start_byte..];
    s.chars().take(120).collect()
}

/// Every `.md` under `root`, skipping dotfile/dotted-dir names, sorted by
/// mtime descending.
pub(crate) fn collect(root: &Path, out: &mut Vec<(PathBuf, SystemTime)>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    let mut dirs = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            dirs.push(path);
        } else if path.extension().is_some_and(|x| x == "md") {
            let modified = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((path, modified));
        }
    }
    for d in dirs {
        collect(&d, out);
    }
}

/// Case-insensitive substring search over every `.md` under `root`: title
/// matches score 3, body matches 1 (first matching line kept as the snippet).
/// Empty query returns the most recently modified notes. Sorted by score
/// then mtime, capped at 50.
pub(crate) fn search(root: &Path, query: &str) -> Vec<Hit> {
    let mut files = Vec::new();
    collect(root, &mut files);
    files.sort_by_key(|f| std::cmp::Reverse(f.1));

    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return files
            .into_iter()
            .take(50)
            .map(|(path, _)| {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                Hit {
                    title: file_title(&path, &text),
                    path,
                    line: 0,
                    snippet: String::new(),
                    score: 0,
                }
            })
            .collect();
    }

    let mut scored: Vec<(Hit, SystemTime)> = Vec::new();
    for (path, modified) in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let title = file_title(&path, &text);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let mut score = 0;
        if stem.contains(&q) || title.to_lowercase().contains(&q) {
            score = 3;
        }
        let mut line = 0;
        let mut snippet = String::new();
        for (i, l) in text.lines().enumerate() {
            if match_range(l, &q).is_some() {
                line = i + 1;
                snippet = snippet_of(l, &q);
                if score == 0 {
                    score = 1;
                }
                break;
            }
        }
        if score > 0 {
            scored.push((
                Hit {
                    path,
                    title,
                    line,
                    snippet,
                    score,
                },
                modified,
            ));
        }
    }
    scored.sort_by(|(a, am), (b, bm)| b.score.cmp(&a.score).then(bm.cmp(am)));
    scored.truncate(50);
    scored.into_iter().map(|(h, _)| h).collect()
}

/// Byte offset of the first char of 1-based `line` in `path` — cheap cursor
/// target for jumping to a hit's line. None when unreadable.
pub(crate) fn offset_of_line(path: &Path, line: usize) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }
    let text = std::fs::read_to_string(path).ok()?;
    let mut off = 0;
    for l in text.lines().take(line - 1) {
        off += l.len() + 1;
    }
    Some(off.min(text.len()))
}

/// Subsequence fuzzy match for the palette's `>` command mode: needle chars
/// must appear in order; consecutive runs and word-start hits score higher.
/// `needle_lower` must already be lowercase.
pub(crate) fn fuzzy_score(hay: &str, needle_lower: &str) -> Option<u32> {
    let hay: Vec<char> = hay.chars().collect();
    let mut needle = needle_lower.chars().peekable();
    let mut score = 0u32;
    let mut last: Option<usize> = None;
    for (i, &hc) in hay.iter().enumerate() {
        let Some(&nc) = needle.peek() else { break };
        let mut lower = hc.to_lowercase();
        if lower.next() != Some(nc) || lower.next().is_some() {
            continue;
        }
        needle.next();
        score += 1;
        if last == i.checked_sub(1) {
            score += 6; // consecutive run
        } else if i == 0 || hay[i - 1] == ' ' {
            score += 3; // word start
        }
        last = Some(i);
    }
    if needle.peek().is_some() {
        return None;
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "abstract-search-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, text: &str, secs_ago: u64) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        let mtime = SystemTime::now() - std::time::Duration::from_secs(secs_ago);
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(mtime).unwrap();
        p
    }

    #[test]
    fn title_hit_outranks_body_hit() {
        let dir = space("title");
        write(&dir, "alpha.md", "# Alpha\nbanana body\n", 10);
        write(&dir, "beta.md", "# Beta\nalpha in the body\n", 5);
        write(&dir, "gamma.md", "# Gamma\nnothing here\n", 1);
        let hits = search(&dir, "alpha");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Alpha");
        assert_eq!(hits[0].score, 3);
        assert_eq!(hits[1].title, "Beta");
        assert_eq!(hits[1].score, 1);
        assert_eq!(hits[1].line, 2);
        assert!(hits[1].snippet.contains("alpha"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_is_case_insensitive() {
        let dir = space("case");
        write(&dir, "note.md", "HeLLo Wörld\n", 1);
        let hits = search(&dir, "wörld");
        assert_eq!(hits.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_query_lists_recent() {
        let dir = space("recent");
        write(&dir, "old.md", "# Old\n", 100);
        write(&dir, "mid.md", "# Mid\n", 50);
        write(&dir, "new.md", "# New\n", 1);
        let hits = search(&dir, "");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].title, "New");
        assert_eq!(hits[1].title, "Mid");
        assert_eq!(hits[2].title, "Old");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dotfiles_are_skipped() {
        let dir = space("dot");
        write(&dir, "real.md", "find me\n", 1);
        write(&dir, ".hidden.md", "find me\n", 1);
        std::fs::create_dir_all(dir.join(".secret")).unwrap();
        std::fs::write(dir.join(".secret").join("inner.md"), "find me").unwrap();
        let hits = search(&dir, "find me");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, dir.join("real.md"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn match_range_returns_byte_offsets() {
        let r = match_range("Héllo World", "world").unwrap();
        assert_eq!(r, 7..12);
        assert_eq!(&"Héllo World"[r], "World");
        // 'İ' lowercases to 2 chars and must never match/slice wrongly.
        assert_eq!(match_range("İx", "i"), None);
    }

    #[test]
    fn fuzzy_score_ranks_runs_and_word_starts() {
        // Subsequence required.
        assert_eq!(fuzzy_score("Save now", "to"), None);
        // Consecutive run beats scattered hits.
        assert!(fuzzy_score("Toggle outline", "to") > fuzzy_score("Start tour", "to"));
        // Word start beats mid-word.
        assert!(fuzzy_score("Save now", "s") > fuzzy_score("Open settings…", "s"));
        // Empty query matches everything at score 0.
        assert_eq!(fuzzy_score("anything", ""), Some(0));
        // Case-insensitive; multi-char lowercases never match single chars.
        assert!(fuzzy_score("CYCLE theme", "ct").is_some());
        assert_eq!(fuzzy_score("İx", "i"), None);
    }
}
