use super::*;

#[test]
fn small_and_empty_sources_are_analyzed_once_per_revision() {
    let mut cache = Cache::new();
    cache.update("# Heading", 1);
    assert_eq!(cache.get(1).unwrap().flags.len(), 9);
    cache.update("different", 1);
    assert!(matches!(
        cache.get(1).unwrap().lines[0].1,
        crate::core::md::Kind::Heading(1)
    ));
    cache.update("", 2);
    assert!(cache.get(2).unwrap().flags.is_empty());
    assert!(cache.get(1).is_none());
}

#[test]
fn stale_async_reply_is_dropped_and_latest_revision_is_analyzed() {
    let mut cache = Cache::new();
    let (tx, reply) = mpsc::channel();
    cache.state = State::Running { revision: 1, reply };
    let current = format!("# Current\n{}", "a".repeat(ASYNC_THRESHOLD));
    cache.update(&current, 2);
    assert!(cache.get(2).is_none());
    tx.send(Analyzer::new().analyze("stale")).unwrap();
    assert!(cache.poll(&current, 2));
    assert!(cache.get(1).is_none());
    assert!(matches!(cache.state, State::Running { revision: 2, .. }));
}

#[test]
fn small_edit_does_not_wait_for_a_stale_large_analysis() {
    let mut cache = Cache::new();
    let (tx, reply) = mpsc::channel();
    cache.state = State::Running { revision: 1, reply };
    cache.update("# Current", 2);
    assert!(tx.send(Analyzer::new().analyze("stale")).is_err());
    assert!(matches!(
        cache.get(2).unwrap().lines[0].1,
        crate::core::md::Kind::Heading(1)
    ));
}

#[test]
fn threshold_uses_background_analysis_and_worker_disconnect_is_safe() {
    let mut cache = Cache::new();
    cache.update(&"a".repeat(ASYNC_THRESHOLD), 1);
    assert!(cache.running());
    assert!(cache.get(1).is_none());
    let (tx, reply) = mpsc::channel();
    cache.state = State::Running { revision: 2, reply };
    drop(tx);
    assert!(cache.poll("new", 2));
    assert!(!cache.running());
    assert!(cache.get(2).is_some());
}
