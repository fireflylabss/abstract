//! The note folder as a tree: real directories and `.md` files on disk.
//! `scan` is blocking and runs on the background executor; `flatten` turns the
//! tree into sidebar rows.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_DEPTH: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Folder,
    Note,
}

#[derive(Debug)]
pub struct Node {
    /// Absolute path.
    pub path: PathBuf,
    /// Folder name, or the note's file stem.
    pub name: String,
    pub kind: NodeKind,
    pub modified: SystemTime,
    pub children: Vec<Node>,
}

impl Node {
    pub fn is_folder(&self) -> bool {
        self.kind == NodeKind::Folder
    }
}

/// Blocking. Folders sort by case-insensitive name, then notes newest first;
/// dotfiles and symlinked directories are skipped and folders deeper than
/// `MAX_DEPTH` are shown but not descended into.
pub fn scan(root: &Path) -> io::Result<Vec<Node>> {
    std::fs::create_dir_all(root)?;
    scan_dir(root, 0)
}

fn scan_dir(dir: &Path, depth: usize) -> io::Result<Vec<Node>> {
    let mut nodes: Vec<Node> = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        // Follow links: a note's recency is its content's mtime, not the
        // moment the symlink was created.
        let modified = path
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        // `file_type` does not follow links: a symlink reports itself, so
        // check the target only to refuse symlinked directories.
        if ft.is_dir() || (ft.is_symlink() && path.is_dir()) {
            if ft.is_symlink() {
                continue;
            }
            let children = if depth + 1 < MAX_DEPTH {
                scan_dir(&path, depth + 1).unwrap_or_default()
            } else {
                Vec::new()
            };
            nodes.push(Node {
                path,
                name,
                kind: NodeKind::Folder,
                modified,
                children,
            });
        } else if Path::new(&name).extension().is_some_and(|x| x == "md") {
            let name = Path::new(&name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(name);
            nodes.push(Node {
                path,
                name,
                kind: NodeKind::Note,
                modified,
                children: Vec::new(),
            });
        }
    }
    nodes.sort_by(|a, b| match (a.is_folder(), b.is_folder()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (true, true) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        (false, false) => b.modified.cmp(&a.modified),
    });
    Ok(nodes)
}

/// Depth-first visible rows: a folder's children follow it when expanded.
#[derive(Clone, Debug)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
    pub depth: usize,
    pub expanded: bool,
}

pub fn flatten(nodes: &[Node], expanded: &HashSet<PathBuf>) -> Vec<Row> {
    let mut out = Vec::new();
    flatten_into(nodes, 0, expanded, &mut out);
    out
}

fn flatten_into(nodes: &[Node], depth: usize, expanded: &HashSet<PathBuf>, out: &mut Vec<Row>) {
    for n in nodes {
        let open = n.is_folder() && expanded.contains(&n.path);
        out.push(Row {
            path: n.path.clone(),
            name: n.name.clone(),
            kind: n.kind,
            depth,
            expanded: open,
        });
        if open {
            flatten_into(&n.children, depth + 1, expanded, out);
        }
    }
}

/// Most recently modified note anywhere in the tree.
pub fn newest_note(nodes: &[Node], except: Option<&Path>) -> Option<PathBuf> {
    let mut best: Option<&Node> = None;
    let mut stack: Vec<&Node> = nodes.iter().collect();
    while let Some(n) = stack.pop() {
        if n.is_folder() {
            stack.extend(n.children.iter());
        } else if except.is_none_or(|x| n.path != x) && best.is_none_or(|b| n.modified > b.modified)
        {
            best = Some(n);
        }
    }
    best.map(|n| n.path.clone())
}

/// A filesystem-safe note/file name: no separators, reserved characters or
/// control codes, no leading/trailing dots, capped at 80 chars.
pub fn stem_for_title(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    let stem: String = trimmed.chars().take(80).collect();
    if stem.is_empty() {
        crate::i18n::t(crate::i18n::Key::Untitled).to_string()
    } else {
        stem
    }
}

/// `X.md`, `X 2.md`, … — `exclude` is the file being renamed so renaming to
/// itself is fine.
pub fn unique_path(dir: &Path, stem: &str, exclude: Option<&Path>) -> PathBuf {
    for n in 1.. {
        let name = if n == 1 {
            format!("{stem}.md")
        } else {
            format!("{stem} {n}.md")
        };
        let path = dir.join(name);
        if exclude == Some(path.as_path()) || !path.exists() {
            return path;
        }
    }
    unreachable!()
}

/// Names the app itself generated (`nota-…`) or the untitled fallback.
pub fn is_placeholder_stem(stem: &str) -> bool {
    if let Some(rest) = stem.strip_prefix("nota-") {
        return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
    }
    crate::i18n::Lang::ALL.iter().any(|l| {
        let s = crate::i18n::lookup(*l, crate::i18n::Key::Untitled);
        stem == s
            || stem
                .strip_prefix(&format!("{s} "))
                .is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The note follows its title: either a placeholder name or exactly the
/// title-derived stem, optionally suffixed with ` <n>` by `unique_path`.
pub fn synced_stem(stem: &str, title: &str) -> bool {
    if is_placeholder_stem(stem) {
        return true;
    }
    let base = stem_for_title(title);
    stem == base
        || stem.strip_prefix(base.as_str()).is_some_and(|r| {
            r.strip_prefix(' ')
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Tree(PathBuf);
    impl Tree {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "abstract-vault-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn scan_orders_and_skips() {
        let t = Tree::new();
        write(&t.0, "b.md", "b");
        write(&t.0, "a.md", "a");
        std::thread::sleep(std::time::Duration::from_millis(5));
        write(&t.0, "newer.md", "n");
        write(&t.0, "not-md.txt", "x");
        write(&t.0, ".hidden.md", "x");
        write(&t.0, ".hdir/x.md", "x");
        write(&t.0, "Beta/x.md", "x");
        write(&t.0, "alpha/x.md", "x");
        fs::create_dir_all(t.0.join("Empty")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(t.0.join("alpha"), t.0.join("linked")).unwrap();
            std::os::unix::fs::symlink(t.0.join("a.md"), t.0.join("linked.md")).unwrap();
        }

        let nodes = scan(&t.0).unwrap();
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        // Folders first (case-insensitive), then notes newest first. The
        // symlinked dir never appears; symlinked .md files do.
        assert_eq!(names[..3], ["alpha", "Beta", "Empty"]);
        assert!(names.contains(&"linked"), "symlinked .md file is a note");
        assert!(!names.contains(&".hidden"));
        assert!(!names.contains(&"not-md"));
        assert_eq!(nodes[3].name, "newer");
        let alpha = &nodes[0];
        assert_eq!(alpha.children.len(), 1);
        assert_eq!(alpha.children[0].name, "x");
    }

    #[test]
    fn flatten_respects_expanded() {
        let t = Tree::new();
        write(&t.0, "root.md", "r");
        write(&t.0, "sub/inner.md", "i");
        let nodes = scan(&t.0).unwrap();
        let rows = flatten(&nodes, &HashSet::new());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "sub");
        assert_eq!(rows[0].depth, 0);
        assert!(!rows[0].expanded);

        let mut expanded = HashSet::new();
        expanded.insert(t.0.join("sub"));
        let rows = flatten(&nodes, &expanded);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].name, "inner");
        assert_eq!(rows[1].depth, 1);
    }

    #[test]
    fn stem_for_title_sanitizes() {
        assert_eq!(stem_for_title("  Olá / mundo: * ?  "), "Olá  mundo");
        assert_eq!(stem_for_title("..."), "Sem título");
        assert_eq!(stem_for_title("   "), "Sem título");
        assert_eq!(stem_for_title("a\u{0}b"), "ab");
        assert_eq!(stem_for_title(&"x".repeat(200)).chars().count(), 80);
        assert_eq!(stem_for_title(".ok."), "ok");
    }

    #[test]
    fn unique_path_skips_existing_and_exclude() {
        let t = Tree::new();
        write(&t.0, "a.md", "");
        write(&t.0, "a 2.md", "");
        assert_eq!(unique_path(&t.0, "a", None), t.0.join("a 3.md"));
        let a = t.0.join("a.md");
        assert_eq!(unique_path(&t.0, "a", Some(&a)), a);
    }

    #[test]
    fn placeholder_and_synced() {
        assert!(is_placeholder_stem("nota-1717"));
        assert!(is_placeholder_stem("Sem título"));
        assert!(is_placeholder_stem("Sem título 3"));
        assert!(!is_placeholder_stem("nota-"));
        assert!(!is_placeholder_stem("nota-x"));
        assert!(!is_placeholder_stem("README"));

        assert!(synced_stem("nota-1", "# Título"));
        assert!(synced_stem("Título", "Título"));
        assert!(synced_stem("Título 2", "Título"));
        assert!(!synced_stem("Projeto", "Título"));
        assert!(!synced_stem("Título x", "Título"));
        // README.md whose first line is "# Projeto" is never synced.
        assert!(!synced_stem("README", "Projeto"));
    }
}
