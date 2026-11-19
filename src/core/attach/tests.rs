    use super::*;

    #[test]
    fn rebase_keeps_image_targets() {
        let text = "![a](attachments/a.png) ![b](../b%20c.png) ![w](https://x/y.png) ![[e.png]]";
        let images = crate::core::md::Analyzer::new().analyze(text).images;
        let mut t = text.to_string();
        for (r, s) in rebase(text, &images, Path::new("/v/n"), Path::new("/v/n/deep")) {
            t.replace_range(r, &s);
        }
        assert_eq!(
            t,
            "![a](../attachments/a.png) ![b](../../b%20c.png) ![w](https://x/y.png) ![[e.png]]"
        );
        assert!(rebase(text, &images, Path::new("/v/n"), Path::new("/v/n")).is_empty());
    }

    #[test]
    fn relative_paths() {
        let r = Path::new("/v");
        assert_eq!(
            relative(r, Path::new("/v/attachments/a.png")),
            "attachments/a.png"
        );
        assert_eq!(
            relative(Path::new("/v/x/y"), Path::new("/v/z/a.png")),
            "../../z/a.png"
        );
    }

    #[test]
    fn encode_decode_round_trip() {
        let s = "my pics/a (1)%.png";
        assert_eq!(encode(s), "my%20pics/a%20%281%29%25.png");
        assert_eq!(decode(&encode(s)), s);
        assert_eq!(decode("<a b.png>"), "a b.png");
        assert_eq!(decode("100%"), "100%");
    }

    #[test]
    fn candidates_for_sources() {
        let (n, r) = (Path::new("/v/notes"), Path::new("/v"));
        assert_eq!(candidates("a%20b.png", false, n, r), [n.join("a b.png")]);
        assert_eq!(
            candidates("https://x/a.png", false, n, r),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            candidates("file:///tmp/a.png", false, n, r),
            [PathBuf::from("/tmp/a.png")]
        );
        assert_eq!(candidates("s.png", true, n, r).len(), 4);
    }

    #[test]
    fn import_stores_and_references() {
        let root = std::env::temp_dir().join(format!("abstract-attach-{}", stamp()));
        let notes = root.join("notes");
        let outside = root.with_extension("out");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("my cat.jpg"), b"jpg").unwrap();
        std::fs::write(outside.join("doc.pdf"), b"pdf").unwrap();
        std::fs::write(notes.join("Other.md"), b"# Other").unwrap();
        std::fs::write(root.join("in.png"), b"png").unwrap();

        let md = import(
            vec![
                Incoming::Image {
                    bytes: b"img".to_vec(),
                    ext: "png",
                },
                Incoming::Image {
                    bytes: b"img2".to_vec(),
                    ext: "png",
                },
                Incoming::Path(outside.join("my cat.jpg")),
                Incoming::Path(notes.join("Other.md")),
                Incoming::Path(root.join("in.png")),
                Incoming::Path(outside.join("doc.pdf")),
            ],
            &notes,
            &root,
            "1",
        )
        .unwrap();
        assert_eq!(md[0], "![](attachments/image-1.png)");
        assert_eq!(md[1], "![](attachments/image-1-2.png)");
        assert_eq!(md[2], "![my cat](attachments/my%20cat.jpg)");
        assert_eq!(md[3], "[[Other]]");
        assert_eq!(md[4], "![in](../in.png)");
        assert!(md[5].starts_with("[doc.pdf](file://") && md[5].ends_with("/doc.pdf)"));
        assert_eq!(
            std::fs::read(notes.join("attachments/image-1-2.png")).unwrap(),
            b"img2"
        );
        assert_eq!(
            std::fs::read(notes.join("attachments/my cat.jpg")).unwrap(),
            b"jpg"
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
