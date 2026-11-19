use super::*;
use crate::io::export::Format;

#[test]
fn export_keeps_the_dialog_confirmed_path_and_never_overwrites_its_sibling() {
    for format in [Format::Html, Format::Pdf] {
        for name in ["chosen.txt", "chosen"] {
            for exists in [false, true] {
                let fixture = Fixture::new();
                let note = fixture.0.join("Note.md");
                std::fs::write(&note, "# Note\n").unwrap();
                let confirmed = fixture.0.join(name);
                if exists {
                    std::fs::write(&confirmed, "confirmed old content").unwrap();
                }
                let sibling = confirmed.with_extension(format.extension());
                std::fs::write(&sibling, "unconfirmed content").unwrap();
                let mut rt = fixture.runtime();
                until(&mut rt, |s| s.document().is_some());
                rt.update(|s, tx| s.export_note(note.clone(), format, tx));
                rt.frame(0.01);
                rt.resolve_file_dialog(Some(confirmed.clone())).unwrap();
                until(&mut rt, |s| s.export.notice.is_some());
                assert_eq!(
                    std::fs::read_to_string(&sibling).unwrap(),
                    "unconfirmed content"
                );
                let bytes = std::fs::read(&confirmed).unwrap();
                match format {
                    Format::Html => {
                        assert!(String::from_utf8(bytes).unwrap().contains("<h1>Note</h1>"))
                    }
                    Format::Pdf => assert!(bytes.starts_with(b"%PDF")),
                }
            }
        }
    }
}

#[test]
fn export_uses_inactive_dirty_buffer_and_snapshots_before_the_dialog() {
    let fixture = Fixture::new();
    let first = fixture.0.join("First.md");
    let second = fixture.0.join("Second.md");
    std::fs::write(&first, "# Disk\n").unwrap();
    std::fs::write(&second, "# Second\n").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| {
        !s.scanning && s.files_pending == 0 && s.document().is_some()
    });
    rt.update(|s, tx| s.open(first.clone(), tx));
    until(&mut rt, |s| s.active.as_ref() == Some(&first));
    rt.update(|s, tx| {
        s.settings.kv.set("autosave_delay", "60");
        s.document_mut().unwrap().edit.insert("UNSAVED BUFFER");
        s.edited(tx);
        s.open(second.clone(), tx);
    });
    until(&mut rt, |s| s.active.as_ref() == Some(&second));
    rt.update(|s, tx| s.export_note(first.clone(), Format::Html, tx));
    rt.frame(0.01);
    assert_eq!(rt.file_dialog().unwrap().0, rune::FileDialogKind::Save);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "# Disk\n");
    rt.update(|s, _| s.documents.get_mut(&first).unwrap().edit.insert(" LATER"));
    let destination = fixture.0.join("out.html");
    rt.resolve_file_dialog(Some(destination.clone())).unwrap();
    until(&mut rt, |s| s.export.notice.is_some());
    let html = std::fs::read_to_string(destination).unwrap();
    assert!(html.contains("UNSAVED BUFFER"));
    assert!(!html.contains("LATER"));
    assert!(!html.contains("<h1>Second</h1>"));
    assert_eq!(rt.state.active.as_ref(), Some(&second));
}

#[test]
fn export_shortcuts_support_both_key_forms_and_cancel_without_writing() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.v1.md"), "# Note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    for command in [false, true] {
        for text in [false, true] {
            for pdf in [false, true] {
                let mods = Mods {
                    cmd: command,
                    ctrl: !command,
                    shift: !pdf,
                    alt: pdf,
                    ..Mods::default()
                };
                let key = if text {
                    RuneKey::Text("e".into())
                } else {
                    RuneKey::Command('e')
                };
                press(&mut rt, key, mods);
                let (_, options) = rt
                    .file_dialog()
                    .expect("export shortcut opens a save dialog");
                assert_eq!(
                    options.file_name.as_deref(),
                    Some(if pdf { "Note.v1.pdf" } else { "Note.v1.html" })
                );
                rt.resolve_file_dialog(None).unwrap();
                rt.frame(0.01);
                assert!(rt.state.export.notice.is_none());
                assert_eq!(rt.state.export.phase, export::Phase::Idle);
            }
        }
    }
    assert!(!fixture.0.join("Note.v1.html").exists());
    assert!(!fixture.0.join("Note.v1.pdf").exists());
}

#[test]
fn tab_context_exports_an_inactive_note_and_errors_are_visible() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "# Note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| s.file_menu(path.clone(), false, [200., 100.], tx));
    rt.frame(0.01);
    let (x, y) = rt.find("export-html").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.01);
    let dest = fixture.0.join("occupied.html");
    std::fs::create_dir(&dest).unwrap();
    rt.resolve_file_dialog(Some(dest)).unwrap();
    until(&mut rt, |s| s.export.notice.is_some());
    assert!(rt.state.export.notice.as_deref().unwrap().contains(':'));
}

#[test]
fn pasted_images_are_imported_and_inserted_as_one_undo_step() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "# Note\n").unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(4, 3, image::Rgb([40, 120, 220]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let bytes = png.into_inner();
    let content = rune::ClipboardContent::default()
        .with_image(rune::ClipboardImage::from_png(bytes.clone()).unwrap());
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    let original = rt.state.document().unwrap().edit.text.clone();
    rt.paste_content(content);
    rt.frame(0.01);
    until(&mut rt, |s| !s.attachments.busy());
    let text = &rt.state.document().unwrap().edit.text;
    assert!(text.contains("![](attachments/image-"), "{text}");
    let image = std::fs::read_dir(fixture.0.join("attachments"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::read(image).unwrap(), bytes);
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.undo();
        s.edited(tx);
    });
    assert_eq!(rt.state.document().unwrap().edit.text, original);
}

#[test]
fn dropped_images_keep_their_target_tab_and_report_import_errors() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let path = fixture.0.join("First.md");
    let second = fixture.0.join("Second.md");
    std::fs::write(&path, "first").unwrap();
    std::fs::write(&second, "second").unwrap();
    let image = outside.0.join("image.png");
    std::fs::write(&image, b"image bytes").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.active.as_ref() == Some(&path));
    let (x, y) = rt.find("editor").unwrap();
    rt.update(|_, _| {});
    rt.drop_files(vec![image], Some((x, y)));
    rt.frame(0.01);
    until(&mut rt, |s| {
        s.attachments.busy()
            || s.documents[&path]
                .edit
                .text
                .contains("attachments/image.png")
    });
    rt.update(|s, tx| s.open(second.clone(), tx));
    until(&mut rt, |s| {
        s.active.as_ref() == Some(&second) && !s.attachments.busy()
    });
    assert!(
        rt.state.documents[&path]
            .edit
            .text
            .contains("attachments/image.png")
    );
    assert_eq!(rt.state.documents[&second].edit.text, "second");
    assert_eq!(
        std::fs::read(fixture.0.join("attachments/image.png")).unwrap(),
        b"image bytes"
    );
    rt.update(|s, tx| s.drop_images(&[outside.0.join("missing.png")], tx));
    until(&mut rt, |s| !s.attachments.busy());
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("attachment-status"))
    );
    assert_eq!(rt.state.documents[&second].edit.text, "second");
}
