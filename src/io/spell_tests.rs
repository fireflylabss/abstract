use super::*;
use std::time::Duration;

#[test]
fn dropping_worker_persists_queued_dictionary_actions() {
    let path = std::env::temp_dir()
        .join(format!("abstract-spell-shutdown-{}", std::process::id()))
        .join("userdict");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    let worker = Spelling::new(path.clone());
    worker
        .submit(Job::Add("abstractshutdownwordd".into()))
        .unwrap();
    drop(worker);
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("abstractshutdownwordd")
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn dictionary_languages_suggestions_ignore_and_persistence_run_on_worker() {
    let path = std::env::temp_dir()
        .join(format!("abstract-spell-worker-{}", std::process::id()))
        .join("userdict");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    let worker = Spelling::new(path.clone());
    let check = |id, text: &str, langs| {
        worker
            .submit(Job::Check {
                id,
                text: text.into(),
                langs,
            })
            .unwrap();
        let Reply::Checked(ticket, ranges) = worker
            .replies
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(id, ticket);
        ranges
    };
    assert!(check(1, "hello world", Langs::EN).is_empty());
    assert!(check(2, "olá mundo", Langs::PT).is_empty());
    assert!(check(3, "", Langs::EN).is_empty());
    assert_eq!(check(4, "helo", Langs::EN), vec![0..4]);
    worker
        .submit(Job::Suggest {
            id: 5,
            word: "helo".into(),
            langs: Langs::EN,
        })
        .unwrap();
    assert!(
        matches!(worker.replies.recv_timeout(Duration::from_secs(10)).unwrap(), Reply::Suggested(5, words) if words.iter().any(|w| w == "hello"))
    );
    worker.submit(Job::Ignore("helo".into())).unwrap();
    assert!(matches!(
        worker
            .replies
            .recv_timeout(Duration::from_secs(10))
            .unwrap(),
        Reply::Changed(Ok(()))
    ));
    assert!(check(6, "helo", Langs::EN).is_empty());
    worker.submit(Job::Add("abstractwordd".into())).unwrap();
    assert!(matches!(
        worker
            .replies
            .recv_timeout(Duration::from_secs(10))
            .unwrap(),
        Reply::Changed(Ok(()))
    ));
    assert!(check(7, "abstractwordd", Langs::EN).is_empty());
    let persisted = Engine::load_from(&path);
    assert!(persisted.check("abstractwordd", Langs::EN));
    assert!(!persisted.check("helo", Langs::EN));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
#[test]
fn dictionary_persistence_error_is_reported() {
    let path = std::env::temp_dir().join(format!("abstract-spell-error-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let worker = Spelling::new(path.clone());
    worker.submit(Job::Add("abstractwordd".into())).unwrap();
    assert!(matches!(
        worker
            .replies
            .recv_timeout(Duration::from_secs(10))
            .unwrap(),
        Reply::Changed(Err(_))
    ));
    let _ = std::fs::remove_dir_all(path);
}
