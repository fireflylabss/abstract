use super::*;

#[test]
fn windows_reserved_names_are_safe_on_every_platform() {
    for name in [
        "CON", "con.txt", "PRN", "AUX", "NUL", "COM1", "com9.md", "LPT1", "lpt9.txt",
    ] {
        assert!(stem_for_title(name).starts_with('_'), "{name}");
    }
    assert_eq!(stem_for_title(" name. "), "name");
    assert_eq!(stem_for_title("a<>:\"/\\|?*b"), "ab");
    assert_eq!(stem_for_title("COM10"), "COM10");
    assert_eq!(stem_for_title("lpt0"), "lpt0");
    assert!(!stem_for_title("... ").is_empty());
    assert!(stem_for_title(&"🎉".repeat(90)).len() <= 200);
}
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
