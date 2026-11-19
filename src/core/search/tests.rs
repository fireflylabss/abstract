use super::*;

#[test]
fn open_buffers_override_disk_and_include_unsaved_notes() {
    let dir = space("buffers");
    let path = write(&dir, "note.md", "old disk text", 0);
    let buffers = HashMap::from([
        (path.clone(), "unsaved Straße".into()),
        (dir.join("new.md"), "another Straße".into()),
    ]);
    let hits = search_with(&dir, "strasse", &buffers);
    assert_eq!(hits.len(), 2);
    for hit in hits {
        assert_eq!(&buffers[&hit.path][hit.range], "Straße");
    }
    assert!(search_with(&dir, "old disk", &buffers).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expanding_case_matches_point_into_original_text() {
    let dir = space("unicode-ranges");
    let source = "# Note\r\nİ before STRAẞE and Straße\r\n";
    write(&dir, "note.md", source, 0);
    for (query, expected) in [("i̇", "İ"), ("strasse", "STRAẞE"), ("before", "before")] {
        let hits = search(&dir, query);
        assert_eq!(hits.len(), 1);
        assert_eq!(&source[hits[0].range.clone()], expected);
        assert_eq!(hits[0].line, 2);
    }
    assert!(search(&dir, "absent").is_empty());
    assert!(search(&dir.join("missing"), "note").is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

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
