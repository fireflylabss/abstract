use super::*;

#[test]
fn roundtrip_keeps_active_and_dedups() {
    let s = Spaces::parse("/a\n* /b\n/a\n\n/c\n");
    assert_eq!(
        s.paths,
        vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c")
        ]
    );
    assert_eq!(s.current(), Path::new("/b"));
    assert_eq!(Spaces::parse(&s.serialize()), s);
}

#[test]
fn remove_keeps_active_space_pointed_correctly() {
    let mut s = Spaces::parse("/a\n/b\n* /c\n");
    assert!(s.remove(0));
    assert_eq!(s.current(), Path::new("/c"));
    assert!(s.remove(1));
    assert_eq!(s.current(), Path::new("/b"));
    assert!(!s.remove(0), "last space must stay");
}

#[test]
fn activate_existing_or_new() {
    let mut s = Spaces::parse("* /a\n/b\n");
    s.activate_path("/b".into());
    assert_eq!(s.active, 1);
    s.activate_path("/z".into());
    assert_eq!((s.active, s.paths.len()), (2, 3));
}
