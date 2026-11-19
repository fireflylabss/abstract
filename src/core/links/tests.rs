use super::*;
use crate::core::vault::Node;
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
fn link_index_follows_edits_and_deletes() {
    let dir = space("index");
    let alpha = dir.join("alpha.md");
    let beta = dir.join("beta.md");
    std::fs::write(&alpha, "# Alpha\n").unwrap();
    std::fs::write(&beta, "# Beta\nsee [[alpha]]\n").unwrap();
    let names = names_of(&alpha, "# Alpha\n");
    let mut index = LinkIndex::default();
    assert_eq!(index.backlinks(&dir, &alpha, &names).len(), 1);

    std::fs::write(&beta, "# Beta\nno link\n").unwrap();
    let later = SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&beta)
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert!(index.backlinks(&dir, &alpha, &names).is_empty());

    std::fs::write(&beta, "# Beta\n[[Alpha]]\n").unwrap();
    assert_eq!(index.backlinks(&dir, &alpha, &names).len(), 1);
    std::fs::remove_file(&beta).unwrap();
    assert!(index.backlinks(&dir, &alpha, &names).is_empty());
    assert!(index.notes.is_empty());
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
    let mut titles: Vec<String> = LinkIndex::default()
        .backlinks(&dir, &alpha, &names)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["Beta".to_string(), "Gamma".to_string()]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn retarget_keeps_alias_and_code() {
    let text = "[[Old]] [[old | a]] [[Older]] `[[Old]]`\n";
    let links = crate::core::md::Analyzer::new().analyze(text).wiki_links;
    let mut out = text.to_string();
    for (r, s) in retarget(text, &links, "Old", "New") {
        out.replace_range(r, &s);
    }
    assert_eq!(out, "[[New]] [[New | a]] [[Older]] `[[Old]]`\n");
}

#[test]
fn relink_rewrites_other_notes() {
    let dir = space("relink");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let new = dir.join("Plano.md");
    std::fs::write(&new, "# Plano\nme: [[Projeto]]\n").unwrap();
    std::fs::write(dir.join("a.md"), "see [[projeto]]\n").unwrap();
    std::fs::write(dir.join("sub/b.md"), "[[Projeto|p]] and [[Other]]\n").unwrap();
    std::fs::write(dir.join("open.md"), "[[Projeto]]\n").unwrap();
    assert_eq!(
        relink(&dir, &new, "Projeto", &[dir.join("open.md")], false),
        Some(0)
    );
    let read = |p: &str| std::fs::read_to_string(dir.join(p)).unwrap();
    assert_eq!(read("a.md"), "see [[Plano]]\n");
    assert_eq!(read("sub/b.md"), "[[Plano|p]] and [[Other]]\n");
    assert_eq!(read("open.md"), "[[Projeto]]\n");
    assert_eq!(read("Plano.md"), "# Plano\nme: [[Plano]]\n");
    assert!(retarget_file(&dir.join("open.md"), "Projeto", "Plano").unwrap());
    assert_eq!(read("open.md"), "[[Plano]]\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn relink_refuses_ambiguous_stems() {
    let dir = space("relink-ambiguous");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let new = dir.join("Plano.md");
    std::fs::write(&new, "").unwrap();
    std::fs::write(dir.join("sub/Projeto.md"), "").unwrap();
    std::fs::write(dir.join("a.md"), "[[Projeto]]\n").unwrap();
    assert_eq!(relink(&dir, &new, "Projeto", &[], false), None);
    assert_eq!(
        std::fs::read_to_string(dir.join("a.md")).unwrap(),
        "[[Projeto]]\n"
    );

    std::fs::remove_file(dir.join("sub/Projeto.md")).unwrap();
    std::fs::write(dir.join("b.md"), "[[plano]] dangling\n").unwrap();
    assert_eq!(relink(&dir, &new, "Projeto", &[], true), None);
    assert_eq!(
        std::fs::read_to_string(dir.join("a.md")).unwrap(),
        "[[Projeto]]\n"
    );
    assert_eq!(relink(&dir, &new, "Projeto", &[], false), Some(0));
    assert_eq!(
        std::fs::read_to_string(dir.join("a.md")).unwrap(),
        "[[Plano]]\n"
    );
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
