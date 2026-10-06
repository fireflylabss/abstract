//! Timing probes over generated spaces and notes. Ignored by default; run with
//! `cargo test --release perf -- --ignored --nocapture`.

use std::path::{Path, PathBuf};
use std::time::Instant;

fn note_body(i: usize, notes: usize) -> String {
    let mut s = format!("# Note {i}\n\nSome **bold** and _italic_ text with ==highlight==.\n\n");
    for k in 0..8 {
        s.push_str(&format!(
            "- item {k} links to [[Note {}]] and `code`\n",
            (i * 7 + k) % notes
        ));
    }
    s.push_str("\n> [!tip] Callout\n> body\n\n```rust\nfn main() { println!(\"hi\"); }\n```\n");
    s
}

pub(crate) fn space(tag: &str, notes: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("abstract-perf-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for i in 0..notes {
        let sub = dir.join(format!("folder {}", i % 20));
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(format!("Note {i}.md")), note_body(i, notes)).unwrap();
    }
    dir
}

pub(crate) fn big_note(bytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < bytes {
        s.push_str(&note_body(i, 1000));
        s.push('\n');
        i += 1;
    }
    s
}

pub(crate) fn time<T>(label: &str, runs: u32, mut f: impl FnMut() -> T) -> T {
    let mut out = f();
    let start = Instant::now();
    for _ in 0..runs {
        out = f();
    }
    let per = start.elapsed() / runs;
    println!("perf {label}: {per:?}");
    out
}

fn first_note(dir: &Path) -> PathBuf {
    dir.join("folder 0").join("Note 0.md")
}

#[test]
#[ignore]
fn perf_report() {
    for kb in [10, 100, 1000] {
        let text = big_note(kb * 1024);
        let mut a = crate::md::Analyzer::new();
        time(&format!("analyze {kb} KB"), 5, || a.analyze(&text));
    }
    let dir = space("report", 2000);
    let note = first_note(&dir);
    let names = vec!["note 0".to_string()];
    time("vault::scan 2000 notes", 5, || {
        crate::vault::scan(&dir).unwrap()
    });
    time("backlinks 2000 notes, cold index", 5, || {
        crate::links::LinkIndex::default().backlinks(&dir, &note, &names)
    });
    let mut index = crate::links::LinkIndex::default();
    time("backlinks 2000 notes, warm index", 5, || {
        index.backlinks(&dir, &note, &names)
    });
    time("search::search 2000 notes", 5, || {
        crate::search::search(&dir, "item 3")
    });
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Worst case for the spellcheck: scanning EVERY line of a 1 MB note with
/// both dictionaries (the UI only ever checks the visible band).
#[test]
#[ignore]
fn perf_spell() {
    let engine = crate::spell::Engine::load();
    let text = big_note(1024 * 1024);
    let mut a = crate::md::Analyzer::new();
    let an = a.analyze(&text);
    let langs = crate::spell::Langs::EN | crate::spell::Langs::PT;
    let flagged = time("spell scan 1 MB note, all lines", 3, || {
        let mut n = 0usize;
        for ix in 0..an.lines.len() {
            n += crate::spell::scan_line(&an, &text, ix, &engine, langs).len();
        }
        n
    });
    println!("perf spell flagged ranges: {flagged}");
}
