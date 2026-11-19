use super::*;

#[test]
fn link_alias_reuses_open_buffer_identity_and_unsaved_text() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    let alias = fixture.0.join("note.md");
    std::fs::write(&path, "# Note\n[other case](note.md)").unwrap();
    #[cfg(unix)]
    if !alias.exists() {
        std::os::unix::fs::symlink(&path, &alias).unwrap();
    }
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.active.as_ref() == Some(&path));
    rt.update(|s, tx| {
        let edit = &mut s.document_mut().unwrap().edit;
        edit.set_caret(edit.text.len());
        edit.insert("\nunsaved");
        s.edited(tx);
        s.document_mut().unwrap().edit.set_caret(10);
        s.follow_link(tx);
    });
    until(&mut rt, |s| !s.navigation.busy);
    assert_eq!(rt.state.tabs.paths(), [path.clone()]);
    assert_eq!(rt.state.active.as_ref(), Some(&path));
    assert_eq!(rt.state.documents.len(), 1);
    assert!(rt.state.document().unwrap().edit.text.ends_with("unsaved"));
}

fn command(rt: &mut Runtime<State>, c: &str, shift: bool) {
    press(
        rt,
        RuneKey::Text(c.into()),
        Mods {
            ctrl: true,
            shift,
            ..Mods::default()
        },
    );
}

#[test]
fn find_query_enter_shift_enter_and_replacement_field_keys() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "cat cat").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    command(&mut rt, "f", false);
    rt.paste("cat");
    rt.frame(1. / 60.);
    assert_eq!(rt.state.find.as_ref().unwrap().matches.len(), 2);
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert_eq!(rt.state.current_find(), Some(4..7));
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.current_find(), Some(0..3));
    press(
        &mut rt,
        RuneKey::Command('f'),
        Mods {
            cmd: true,
            alt: true,
            ..Mods::default()
        },
    );
    assert!(rt.state.find.as_ref().unwrap().replacing);
    rt.update(|_, tx| tx.focus("find-replacement", false));
    rt.paste("dog");
    rt.frame(1. / 60.);
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert_eq!(rt.state.document().unwrap().edit.text, "dog cat");
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.document().unwrap().edit.text, "dog dog");
}

#[test]
fn find_highlights_navigation_case_replace_and_single_step_undo() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "İ Straße STRAẞE Straße").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.document().is_some());
    command(&mut rt, "h", false);
    rt.update(|s, tx| {
        s.find.as_mut().unwrap().query = TextEdit::new("strasse");
        s.find.as_mut().unwrap().replacement = TextEdit::new("road");
        s.refresh_find(true, tx);
    });
    assert_eq!(rt.state.find.as_ref().unwrap().matches.len(), 3);
    assert_eq!(rt.state.find_spans(theme::LIGHT).len(), 3);
    assert_eq!(rt.state.current_find(), Some(3..10));
    command(&mut rt, "g", false);
    assert_eq!(rt.state.current_find(), Some(11..19));
    command(&mut rt, "g", true);
    assert_eq!(rt.state.current_find(), Some(3..10));
    rt.update(|s, tx| s.replace_find(false, tx));
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "İ road STRAẞE Straße"
    );
    rt.update(|s, tx| {
        s.close_find(tx);
    });
    command(&mut rt, "z", false);
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "İ Straße STRAẞE Straße"
    );
    command(&mut rt, "h", false);
    rt.update(|s, tx| {
        let bar = s.find.as_mut().unwrap();
        bar.query = TextEdit::new("strasse");
        bar.replacement = TextEdit::new("");
        bar.case = true;
        s.refresh_find(true, tx);
    });
    assert!(rt.state.find.as_ref().unwrap().matches.is_empty());
    rt.update(|s, tx| {
        s.find.as_mut().unwrap().case = false;
        s.refresh_find(true, tx);
        s.replace_find(true, tx);
        s.close_find(tx);
    });
    assert_eq!(rt.state.document().unwrap().edit.text, "İ   ");
    command(&mut rt, "z", false);
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "İ Straße STRAẞE Straße"
    );
    rt.update(|s, tx| s.open_find(false, tx));
    rt.update(|s, tx| {
        s.find.as_mut().unwrap().query = TextEdit::new("");
        s.refresh_find(true, tx);
        s.replace_find(true, tx);
    });
    assert!(rt.state.current_find().is_none());
    press(&mut rt, RuneKey::Escape, Mods::default());
    assert!(rt.state.find.is_none());
    assert!(rt.state.document().unwrap().edit.focused);
}

#[test]
fn global_results_open_original_ranges_in_tabs_and_handle_empty_results() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    let source = "# Note\r\nİ Straße\r\n";
    std::fs::write(&path, source).unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    command(&mut rt, "f", true);
    rt.paste("strasse");
    rt.frame(1. / 60.);
    until(&mut rt, |s| !s.searching());
    let hit = &rt.state.search.as_ref().unwrap().hits[0];
    assert_eq!(&source[hit.range.clone()], "Straße");
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert!(rt.state.search.is_none());
    let doc = rt.state.document().unwrap();
    assert_eq!(&doc.edit.text[doc.edit.anchor..doc.edit.focus], "Straße");
    command(&mut rt, "f", true);
    rt.paste("missing");
    rt.frame(1. / 60.);
    until(&mut rt, |s| !s.searching());
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert!(rt.state.search.as_ref().unwrap().hits.is_empty());
    press(&mut rt, RuneKey::Escape, Mods::default());
    assert!(rt.state.search.is_none());
}

#[test]
fn alt_enter_resolves_titles_creates_wiki_notes_follows_local_and_external_links() {
    let fixture = Fixture::new();
    let origin = fixture.0.join("Origin.md");
    let other = fixture.0.join("filename.md");
    let source = "[[mY nOtE#Part|alias]]\n[[Fresh]]\n[local](filename.md#Part)\n[web](https://example.com)\n[bad](missing/fail.md)";
    std::fs::write(&origin, source).unwrap();
    std::fs::write(&other, "# My Note\n\n## Part\nbody\n").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(origin.clone(), tx));
    until(&mut rt, |s| s.active.as_ref() == Some(&origin));
    rt.update(|s, _| s.document_mut().unwrap().edit.set_caret(3));
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            alt: true,
            ..Mods::default()
        },
    );
    until(&mut rt, |s| !s.navigation.busy);
    assert_eq!(rt.state.active.as_ref(), Some(&other));
    assert_eq!(rt.state.document().unwrap().edit.focus, 11);
    for (needle, target) in [("Fresh", fixture.0.join("Fresh.md")), ("local", other)] {
        rt.update(|s, tx| {
            s.open(origin.clone(), tx);
            s.document_mut()
                .unwrap()
                .edit
                .set_caret(source.find(needle).unwrap());
        });
        press(
            &mut rt,
            RuneKey::Enter,
            Mods {
                alt: true,
                ..Mods::default()
            },
        );
        until(&mut rt, |s| !s.navigation.busy);
        assert_eq!(rt.state.active.as_ref(), Some(&target));
        assert!(target.is_file());
    }
    rt.update(|s, tx| {
        s.open(origin.clone(), tx);
        s.document_mut()
            .unwrap()
            .edit
            .set_caret(source.find("web").unwrap());
    });
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            alt: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.take_url_requests().len(), 1);
    rt.update(|s, _| {
        s.document_mut()
            .unwrap()
            .edit
            .set_caret(source.find("bad").unwrap())
    });
    press(
        &mut rt,
        RuneKey::Enter,
        Mods {
            alt: true,
            ..Mods::default()
        },
    );
    until(&mut rt, |s| !s.navigation.busy);
    assert!(rt.state.error.is_some());
    assert_eq!(rt.state.active.as_ref(), Some(&origin));
}
