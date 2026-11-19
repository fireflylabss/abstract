    use super::*;

    fn space(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("abstract-crash-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn report_lists_context_and_redacts_home() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/u".into());
        let r = report(
            7,
            "main",
            "editor::Undo",
            &format!("{home}/src/x.rs:3"),
            "boom",
            None,
            "0: frame",
        );
        assert!(r.starts_with(&format!("abstract {}", env!("CARGO_PKG_VERSION"))));
        assert!(r.contains("last action: editor::Undo"));
        assert!(r.contains("panicked at ~/src/x.rs:3:\nboom"));
        assert!(!r.contains(&home));
    }

    #[test]
    fn stripped_backtrace_is_summarized() {
        let stripped = "   0: __mh_execute_header\n   1: __mh_execute_header\n";
        assert!(tidy_backtrace(stripped).starts_with("unavailable"));
        let real =
            "   0: abstract::main\n             at ./src/main.rs:3:5\n   1: std::rt::lang_start\n";
        assert_eq!(tidy_backtrace(real), real);
    }

    #[test]
    fn pending_is_shown_once_and_pruned() {
        let dir = space("pending");
        assert!(take_pending(&dir).is_none());
        for stamp in 1..=12 {
            std::fs::write(dir.join(format!("crash-{stamp}.txt")), format!("r{stamp}")).unwrap();
        }
        std::fs::write(dir.join("recovered-12.md"), "text").unwrap();
        std::fs::write(dir.join("recovered-1.md"), "old").unwrap();
        let p = take_pending(&dir).unwrap();
        assert_eq!(p.text, "r12");
        assert!(p.recovered);
        assert!(take_pending(&dir).is_none());
        assert!(!dir.join("crash-1.txt").exists());
        assert!(!dir.join("recovered-1.md").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), KEEP + 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unsaved_text_is_released_only_by_its_own_save() {
        let mut u = Unsaved { id: 0, text: None };
        let a = u.hold("one");
        let b = u.hold("two");
        u.release(a);
        assert_eq!(u.text.as_deref(), Some("two"));
        u.release(b);
        assert!(u.text.is_none());
    }

    #[test]
    fn issue_url_is_encoded_and_bounded() {
        let report = format!("a\nb\nc\nd\n\npanicked at x.rs:1:\n{}", "é".repeat(5000));
        let url = issue_url(&report);
        assert!(url.starts_with(ISSUES));
        assert!(url.contains("title=Crash%3A%20panicked%20at%20x.rs%3A1%3A"));
        assert!(!url.contains(' ') && !url.contains('\n'));
        assert!(url.len() < URL_BODY_MAX * 3 + 500);
    }
