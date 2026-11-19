    use super::*;

    #[test]
    fn file_create_signals_watch() {
        let dir = std::env::temp_dir().join(format!("abstract-watch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (_w, mut rx) = watch(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "# hi").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut got = false;
        while std::time::Instant::now() < deadline {
            if rx.try_recv().is_ok() {
                got = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(got, "watcher did not signal within 3s");
        std::fs::remove_dir_all(&dir).unwrap();
    }
