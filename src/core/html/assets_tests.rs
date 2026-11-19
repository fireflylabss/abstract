use super::*;

#[test]
fn base64_encodes_empty_and_partial_blocks() {
    for (bytes, expected) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64(bytes.as_bytes()), expected);
    }
    assert_eq!(base64(&[255, 254, 253]), "//79");
}

#[test]
fn absent_and_remote_images_do_not_read_local_files() {
    let ctx = Context {
        note_dir: Some(std::path::Path::new(".")),
        root: None,
        tree: None,
    };
    assert!(image_source("missing-image.png", false, &ctx).is_none());
    assert!(image_source("https://example.com/image.png", false, &ctx).is_none());
}
