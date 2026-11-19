use super::*;

#[test]
fn resolves_buffers_and_creates_without_clobbering_collisions_or_read_errors() {
    let root = std::env::temp_dir().join(format!("abstract-navigation-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let existing = root.join("different.md");
    std::fs::write(&existing, "# Disk title\n").unwrap();
    let mut request = Request {
        ticket: 1,
        root: root.clone(),
        current: existing.clone(),
        target: Target::Wiki {
            name: "sTRASSE".into(),
            heading: "Part".into(),
        },
        buffers: HashMap::from([(existing.clone(), "# Straße\n## Part\nunsaved".into())]),
    };
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.path, existing);
    assert!(loaded.source.ends_with("unsaved"));
    assert_eq!(loaded.heading, "Part");
    std::fs::write(root.join("AB.md"), "# Unrelated").unwrap();
    request.target = Target::Wiki {
        name: "A/B".into(),
        heading: String::new(),
    };
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.path, root.join("AB 2.md"));
    assert_eq!(
        std::fs::read_to_string(root.join("AB.md")).unwrap(),
        "# Unrelated"
    );
    assert_eq!(loaded.source, "# A/B\n\n");
    let invalid = root.join("Invalid.md");
    std::fs::write(&invalid, [0xff]).unwrap();
    request.target = Target::Local {
        path: invalid.clone(),
        heading: String::new(),
    };
    assert_eq!(
        load(&request).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(std::fs::read(invalid).unwrap(), [0xff]);
    request.target = Target::Local {
        path: root.join("missing/Note.md"),
        heading: String::new(),
    };
    assert_eq!(
        load(&request).err().unwrap().kind(),
        io::ErrorKind::NotFound
    );
    std::fs::remove_dir_all(root).unwrap();
}
