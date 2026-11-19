use super::*;

#[test]
fn closed_documents_are_skipped_before_parsing_queued_snapshots() {
    let (jobs, queue) = mpsc::channel();
    let (observed, visits) = mpsc::channel();
    let (resume, gate) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        work(queue, |source| {
            observed.send(source.to_owned()).unwrap();
            if source == "block" {
                gate.recv().unwrap();
            }
            Analysis::default()
        })
    });
    let timeout = std::time::Duration::from_secs(5);
    let alive = Cache::new();
    let (tx, _block) = mpsc::channel();
    jobs.send(("block".into(), tx, Arc::downgrade(&alive.alive)))
        .unwrap();
    assert_eq!(visits.recv_timeout(timeout).unwrap(), "block");
    let (tx, closed) = mpsc::channel();
    let closed_cache = Cache::new();
    jobs.send(("closed".into(), tx, Arc::downgrade(&closed_cache.alive)))
        .unwrap();
    drop(closed_cache);
    drop(closed);
    let (tx, live) = mpsc::channel();
    jobs.send(("live".into(), tx, Arc::downgrade(&alive.alive)))
        .unwrap();
    resume.send(()).unwrap();
    live.recv_timeout(timeout).unwrap();
    drop(jobs);
    worker.join().unwrap();
    assert_eq!(visits.try_iter().collect::<Vec<_>>(), ["live"]);
}

#[test]
fn documents_share_one_background_analysis_worker() {
    let alive = Cache::new();
    let first = background("a".repeat(ASYNC_THRESHOLD), Arc::downgrade(&alive.alive));
    let second = background(
        format!("# Latest\n{}", "b".repeat(ASYNC_THRESHOLD)),
        Arc::downgrade(&alive.alive),
    );
    let timeout = std::time::Duration::from_secs(5);
    assert_eq!(
        first.recv_timeout(timeout).unwrap().flags.len(),
        ASYNC_THRESHOLD
    );
    assert!(matches!(
        second.recv_timeout(timeout).unwrap().lines[0].1,
        crate::core::md::Kind::Heading(1)
    ));
    assert_eq!(WORKER_STARTS.load(std::sync::atomic::Ordering::Relaxed), 1);
}

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
fn shrinking_then_growing_keeps_one_worker_and_skips_intermediate_revisions() {
    let mut cache = Cache::new();
    let (tx, reply) = mpsc::channel();
    cache.state = State::Running { revision: 1, reply };
    cache.update("# Current", 2);
    let current = format!("# Latest\n{}", "x".repeat(ASYNC_THRESHOLD));
    cache.update(&current, 3);
    assert!(matches!(cache.state, State::Running { revision: 1, .. }));
    assert!(cache.get(2).is_none());
    tx.send(Analyzer::new().analyze("stale")).unwrap();
    assert!(cache.poll(&current, 3));
    assert!(matches!(cache.state, State::Running { revision: 3, .. }));
}

#[test]
fn shrinking_to_a_small_note_analyzes_latest_after_worker_finishes() {
    let mut cache = Cache::new();
    let (tx, reply) = mpsc::channel();
    cache.state = State::Running { revision: 1, reply };
    cache.update("# Current", 2);
    tx.send(Analyzer::new().analyze("stale")).unwrap();
    assert!(cache.poll("# Current", 2));
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
    assert_eq!(cache.get(2).unwrap().flags.len(), 3);
}
