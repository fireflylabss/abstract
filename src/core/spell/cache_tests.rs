use super::*;
use std::collections::BTreeSet;
use std::sync::{RwLock, atomic::AtomicU32};

fn engine() -> Engine {
    Engine {
        en: None,
        pt: None,
        user: RwLock::new(BTreeSet::new()),
        epoch: AtomicU32::new(0),
    }
}
#[test]
fn unchanged_lines_are_reused_and_relative_offsets_move() {
    let engine = engine();
    let mut cache = Cache::default();
    assert_eq!(
        cache.check("wrng\nbadd", &engine, Langs::EN),
        vec![0..4, 5..9]
    );
    let count = cache.lines.len();
    assert_eq!(
        cache.check("\nwrng\nbadd", &engine, Langs::EN),
        vec![1..5, 6..10]
    );
    assert!(cache.lines.len() <= count + 1);
    engine.add_user_word("wrng");
    assert_eq!(cache.check("\nwrng\nbadd", &engine, Langs::EN), vec![6..10]);
}
#[test]
fn contextual_changes_invalidate_identical_lines_and_skip_all_non_prose() {
    let mut cache = Cache::default();
    let engine = engine();
    assert_eq!(cache.check("wrng", &engine, Langs::EN), vec![0..4]);
    assert!(cache.check("```\nwrng\n```", &engine, Langs::EN).is_empty());
    assert!(
        cache
            .check(
                "---\nwrng: badd\n---\n`wrng` [badd](foo) [[wrng|badd]] https://wrng.invalid",
                &engine,
                Langs::EN
            )
            .is_empty()
    );
    assert!(cache.check("", &engine, Langs::EN).is_empty());
    assert_eq!(cache.check("wrng", &engine, Langs::PT), vec![0..4]);
}
#[test]
fn cache_is_bounded_by_entries_and_bytes() {
    let text = (0..MAX_LINES + 10)
        .map(|i| format!("word{i}\n"))
        .collect::<String>();
    let mut cache = Cache::default();
    cache.check(&text, &engine(), Langs::EN);
    assert!(cache.lines.len() <= MAX_LINES);
    assert!(cache.bytes <= MAX_BYTES);
    cache.check(&"a".repeat(MAX_BYTES + 1), &engine(), Langs::EN);
    assert!(cache.bytes <= MAX_BYTES);
}
