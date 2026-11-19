    use super::*;

    #[test]
    fn frame_has_le_header() {
        let f = frame(1, "{}");
        assert_eq!(&f[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&f[8..], b"{}");
    }

    #[test]
    fn handshake_carries_client_id() {
        assert_eq!(
            handshake_json("abstract"),
            "{\"v\":1,\"client_id\":\"abstract\"}"
        );
    }

    #[test]
    fn activity_carries_details_state_and_start() {
        let j = activity_json(42, 7, "My Title", "Space");
        assert!(j.contains("\"cmd\":\"SET_ACTIVITY\""));
        assert!(j.contains(&format!("\"pid\":{}", std::process::id())));
        assert!(j.contains("\"details\":\"My Title\""));
        assert!(j.contains("\"state\":\"Space\""));
        assert!(j.contains("\"start\":42"));
        assert!(j.contains("\"nonce\":\"abstract-7\""));
    }

    #[test]
    fn escape_quotes_backslashes_and_controls() {
        assert_eq!(escape("a\"b\\c\nd\te"), "a\\\"b\\\\c\\nd\\te");
        assert_eq!(escape("\u{1}"), "\\u0001");
        assert_eq!(escape("puro ç ão"), "puro ç ão");
    }

    #[test]
    fn disabled_presence_sends_nothing() {
        let mut p = Presence::new(false);
        p.set(Some("x".into()), "y".into());
        p.set(None, "y".into());
        p.set_enabled(false);
    }
