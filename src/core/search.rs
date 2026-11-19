//! Global note search: case-insensitive substring matching over every `.md`
//! under the space root. Pure — no gpui — so it is testable headless. The
//! query side runs on the background executor; an empty query doubles as a
//! "recent files" list.

use std::collections::HashMap;
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
    pub range: Range<usize>,
}

/// Case-insensitive matching mapped back to complete original characters.
pub(crate) fn match_range(hay: &str, needle_lower: &str) -> Option<Range<usize>> {
    if needle_lower.is_empty() {
        return Some(0..0);
    }
    crate::core::find::matches(hay, needle_lower, false)
        .into_iter()
        .next()
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
    search_with(root, query, &HashMap::new())
}

pub(crate) fn search_with(
    root: &Path,
    query: &str,
    buffers: &HashMap<PathBuf, String>,
) -> Vec<Hit> {
    let mut files = Vec::new();
    collect(root, &mut files);
    for path in buffers.keys().filter(|p| p.starts_with(root)) {
        if !files.iter().any(|(p, _)| p == path) {
            files.push((path.clone(), SystemTime::now()));
        }
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.1));

    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return files
            .into_iter()
            .take(50)
            .map(|(path, _)| {
                let text = buffers
                    .get(&path)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&path).ok())
                    .unwrap_or_default();
                Hit {
                    title: file_title(&path, &text),
                    path,
                    line: 0,
                    snippet: String::new(),
                    score: 0,
                    range: 0..0,
                }
            })
            .collect();
    }

    let mut scored: Vec<(Hit, SystemTime)> = Vec::new();
    for (path, modified) in files {
        let Some(text) = buffers
            .get(&path)
            .cloned()
            .or_else(|| std::fs::read_to_string(&path).ok())
        else {
            continue;
        };
        let title = file_title(&path, &text);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let mut score = 0;
        if match_range(&stem, &q).is_some() || match_range(&title, &q).is_some() {
            score = 3;
        }
        let mut line = 0;
        let mut snippet = String::new();
        let mut range = 0..0;
        let mut offset = 0;
        for (i, l) in text.split_inclusive('\n').enumerate() {
            if let Some(found) = match_range(l, &q) {
                range = offset + found.start..offset + found.end;
                line = i + 1;
                snippet = snippet_of(l, &q);
                if score == 0 {
                    score = 1;
                }
                break;
            }
            offset += l.len();
        }
        if score > 0 {
            scored.push((
                Hit {
                    path,
                    title,
                    line,
                    snippet,
                    score,
                    range,
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

#[cfg(test)]
mod tests;
