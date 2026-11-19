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
