use super::*;
use crate::io::{operations::Operation, watch::Change};

#[test]
fn reopened_document_uses_the_hash_just_loaded_from_disk() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" saved");
        s.edited(tx);
        s.save_now(tx);
    });
    rt.state.writer.flush().unwrap();
    rt.update(|s, tx| {
        s.completions();
        s.close_tab(&path, tx);
        s.completions();
    });
    assert!(!rt.state.documents.contains_key(&path));
    std::fs::write(&path, "external").unwrap();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| s.files_pending == 0 && s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
        s.save_now(tx);
    });
    assert!(rt.state.writer.flush().is_ok());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external local");
}

#[test]
fn a_watcher_recheck_of_the_disk_baseline_does_not_conflict_with_edits() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
        s.external_change(Change {
            path: path.clone(),
            text: Some("original".into()),
        });
    });
    assert!(!rt.state.file_ops.conflicts.contains_key(&path));
    rt.update(|s, tx| s.save_now(tx));
    rt.state.writer.flush().unwrap();
    rt.update(|s, tx| {
        s.completions();
        assert_eq!(
            s.document().unwrap().disk_hash,
            crate::io::writer::hash("original local")
        );
        s.document_mut().unwrap().edit.insert(" newer");
        s.edited(tx);
        s.writer.forget_ack(&path);
        s.external_change(Change {
            path: path.clone(),
            text: Some("original local".into()),
        });
    });
    assert!(!rt.state.file_ops.conflicts.contains_key(&path));
}

#[test]
fn writing_the_pending_local_text_externally_is_already_saved() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
    });
    std::fs::write(&path, "original local").unwrap();
    rt.update(|s, tx| s.save_now(tx));
    assert!(rt.state.writer.flush().is_ok());
}

#[cfg(unix)]
#[test]
fn relocation_does_not_follow_final_symlinks_or_hard_links() {
    use std::fs;
    let dir = Fixture::new();
    let target = dir.0.join("Target.md");
    fs::write(&target, "note").unwrap();
    let alias = dir.0.join("Alias.md");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    let hard = dir.0.join("Hard.md");
    fs::hard_link(&target, &hard).unwrap();
    for other in [&alias, &hard] {
        assert!(crate::io::identity::same_file(&target, other));
        assert!(crate::io::identity::relative(&target, other).is_none());
        assert!(crate::io::identity::relative(other, &target).is_none());
    }
}

#[cfg(unix)]
#[test]
fn symlink_note_saves_through_to_its_regular_target() {
    let fixture = Fixture::new();
    let target = fixture.0.join("Target.md");
    let alias = fixture.0.join("Alias.md");
    std::fs::write(&target, "original").unwrap();
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(alias.clone(), tx));
    until(&mut rt, |s| {
        s.active.as_ref() == Some(&alias) && s.files_pending == 0
    });
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
        s.save_now(tx);
    });
    assert!(rt.state.writer.flush().is_ok());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "original local");
    assert!(
        std::fs::symlink_metadata(alias)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn shutdown_recovers_a_failed_save_without_a_watcher_conflict() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
    });
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    rt.state.shutdown();
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("Note (conflict).md")).unwrap(),
        "original local"
    );
}

#[test]
fn clean_external_reload_rebases_the_next_save() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" saved");
        s.edited(tx);
        s.save_now(tx);
    });
    rt.state.writer.flush().unwrap();
    rt.update(|s, _| {
        s.completions();
    });
    std::fs::write(&path, "external").unwrap();
    rt.update(|s, tx| {
        s.external_change(Change {
            path: path.clone(),
            text: Some("external".into()),
        });
        s.document_mut().unwrap().edit.set_caret(8);
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
        s.save_now(tx);
    });
    assert!(rt.state.writer.flush().is_ok());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external local");
}

#[test]
fn editor_rename_reveals_a_hidden_row() {
    let fixture = Fixture::new();
    let folder = fixture.0.join("Folder");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("Note.md"), "note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.sidebar_open = false;
        s.focus_mode = true;
        s.expanded.clear();
        tx.focus("editor", false);
    });
    press(&mut rt, RuneKey::Function(2), Mods::default());
    assert!(rt.state.sidebar_open && !rt.state.focus_mode);
    assert!(rt.state.expanded.contains(&folder));
}

#[test]
fn clean_external_deletion_can_be_closed() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    std::fs::remove_file(&path).unwrap();
    rt.update(|s, tx| {
        s.external_change(Change {
            path: path.clone(),
            text: None,
        });
        s.close_tab(&path, tx);
    });
    assert!(!rt.state.tabs.paths().contains(&path));
    assert!(!rt.state.file_ops.conflicts.contains_key(&path));
}

#[test]
fn closing_a_clean_deleted_note_removes_its_search_result() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "needle").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.open_search(tx);
        s.search.as_mut().unwrap().query.insert("needle");
        s.start_search(tx);
    });
    until(&mut rt, |s| !s.searching());
    assert!(
        rt.state
            .search
            .as_ref()
            .unwrap()
            .hits
            .iter()
            .any(|hit| hit.path == path)
    );
    std::fs::remove_file(&path).unwrap();
    rt.update(|s, tx| {
        s.external_change(Change {
            path: path.clone(),
            text: None,
        });
        s.close_tab(&path, tx);
    });
    until(&mut rt, |s| !s.searching());
    assert!(
        !rt.state
            .search
            .as_ref()
            .unwrap()
            .hits
            .iter()
            .any(|hit| hit.path == path)
    );
}

#[test]
fn dirty_external_deletion_can_be_explicitly_discarded() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
    });
    std::fs::remove_file(&path).unwrap();
    rt.update(|s, tx| {
        s.external_change(Change {
            path: path.clone(),
            text: None,
        });
        s.reload_conflict(tx);
    });
    until(&mut rt, |s| !s.files_busy());
    assert!(!rt.state.tabs.paths().contains(&path));
    assert!(!path.exists());
}

#[test]
fn dirty_external_deletion_can_be_explicitly_recreated() {
    let fixture = Fixture::new();
    let parent = fixture.0.join("Folder");
    std::fs::create_dir(&parent).unwrap();
    let path = parent.join("Note.md");
    std::fs::write(&path, "note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
    });
    std::fs::remove_dir_all(parent).unwrap();
    rt.update(|s, tx| {
        s.external_change(Change {
            path: path.clone(),
            text: None,
        });
        s.keep_local(tx);
    });
    rt.state.writer.flush().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "note local");
}

#[test]
fn search_results_do_not_retain_renamed_paths() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "needle").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.open_search(tx);
        s.search.as_mut().unwrap().query.insert("needle");
        s.start_search(tx);
    });
    until(&mut rt, |s| !s.searching());
    assert_eq!(rt.state.search.as_ref().unwrap().hits.len(), 1);
    rt.update(|s, tx| {
        s.operate(
            Operation::Rename {
                from: path.clone(),
                to: fixture.0.join("Moved.md"),
            },
            tx,
        )
    });
    until(&mut rt, |s| !s.files_busy());
    until(&mut rt, |s| !s.searching());
    assert!(
        rt.state
            .search
            .as_ref()
            .unwrap()
            .hits
            .iter()
            .all(|hit| hit.path != path)
    );
    assert!(
        rt.state
            .search
            .as_ref()
            .unwrap()
            .hits
            .iter()
            .any(|hit| hit.path == fixture.0.join("Moved.md"))
    );
}

#[test]
fn undo_follows_a_renamed_parent() {
    let fixture = Fixture::new();
    let folder = fixture.0.join("Folder");
    let moved = fixture.0.join("Moved");
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("Note.md");
    std::fs::write(&path, "note").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| s.operate(Operation::Trash(path.clone()), tx));
    until(&mut rt, |s| !s.files_busy());
    rt.update(|s, tx| {
        s.operate(
            Operation::Rename {
                from: folder.clone(),
                to: moved.clone(),
            },
            tx,
        )
    });
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(
        rt.state.file_ops.undo.as_ref().unwrap().path,
        moved.join("Note.md")
    );
    rt.update(|s, tx| s.operate(Operation::Restore(s.file_ops.undo.clone().unwrap()), tx));
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(
        std::fs::read_to_string(moved.join("Note.md")).unwrap(),
        "note"
    );
    assert!(!folder.exists());
}

#[test]
fn rename_flush_checks_external_changes_before_writing() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
    });
    std::fs::write(&path, "external").unwrap();
    rt.update(|s, tx| {
        s.operate(
            Operation::Rename {
                from: path.clone(),
                to: fixture.0.join("Moved.md"),
            },
            tx,
        )
    });
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_or_default(),
        "external"
    );
    assert!(rt.state.file_ops.conflicts.contains_key(&path));
}

#[test]
fn autosave_does_not_recreate_an_externally_moved_parent() {
    let fixture = Fixture::new();
    let folder = fixture.0.join("Folder");
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
    });
    std::fs::rename(&folder, fixture.0.join("Moved")).unwrap();
    rt.update(|s, tx| s.save_now(tx));
    let _ = rt.state.writer.flush();
    assert!(!folder.exists());
}

#[cfg(unix)]
#[test]
fn rename_relocates_buffers_opened_through_directory_aliases() {
    let fixture = Fixture::new();
    let folder = fixture.0.join("Folder");
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("Note.md");
    let moved = folder.join("Moved.md");
    std::fs::write(&path, "original").unwrap();
    let alias = fixture.0.join("Alias");
    std::os::unix::fs::symlink(&folder, &alias).unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(alias.join("Note.md"), tx));
    until(&mut rt, |s| s.files_pending == 0 && s.document().is_some());
    rt.update(|s, tx| {
        s.operate(
            Operation::Rename {
                from: path.clone(),
                to: moved.clone(),
            },
            tx,
        )
    });
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(rt.state.active.as_ref(), Some(&moved));
}

#[test]
fn quitting_with_a_conflict_preserves_both_versions_without_overwriting_recovery() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    std::fs::write(fixture.0.join("Note (conflict).md"), "older recovery").unwrap();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.open(path.clone(), tx));
    until(&mut rt, |s| {
        s.active.as_ref() == Some(&path) && s.files_pending == 0
    });
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
        s.external_change(Change {
            path: path.clone(),
            text: Some("external".into()),
        });
    });
    std::fs::write(&path, "external").unwrap();
    rt.state.shutdown();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external");
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("Note (conflict).md")).unwrap(),
        "older recovery"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("Note (conflict) 2.md")).unwrap(),
        "original local"
    );
    rt.state.shutdown();
    assert!(!fixture.0.join("Note (conflict) 3.md").exists());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" later");
        s.edited(tx);
    });
    rt.state.shutdown();
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("Note (conflict) 3.md")).unwrap(),
        "original local later"
    );
}

#[test]
fn explicit_keep_local_writes_even_when_a_previous_save_was_acknowledged() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "local").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    std::fs::write(&path, "external").unwrap();
    rt.update(|s, tx| {
        s.file_ops
            .conflicts
            .insert(path.clone(), Some("external".into()));
        s.keep_local(tx);
    });
    rt.state.writer.flush().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "local");
    assert!(rt.state.file_ops.conflicts.is_empty());
}

#[test]
fn reload_failure_keeps_the_buffer_and_conflict() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "local").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    std::fs::remove_file(&path).unwrap();
    rt.update(|s, tx| {
        s.file_ops
            .conflicts
            .insert(path.clone(), Some("previous disk content".into()));
        s.reload_conflict(tx);
    });
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(rt.state.document().unwrap().edit.text, "local");
    assert!(rt.state.file_ops.conflicts.contains_key(&path));
    assert!(rt.state.error.is_some());
}

#[test]
fn folder_move_and_immediate_quit_flush_all_open_descendants() {
    let fixture = Fixture::new();
    let old = fixture.0.join("Old");
    let new = fixture.0.join("New");
    std::fs::create_dir(&old).unwrap();
    for name in ["A.md", "B.md"] {
        std::fs::write(old.join(name), name).unwrap();
    }
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    for name in ["A.md", "B.md"] {
        rt.update(|s, tx| s.open(old.join(name), tx));
        until(&mut rt, |s| {
            s.active.as_ref() == Some(&old.join(name)) && s.files_pending == 0
        });
        rt.update(|s, tx| {
            s.document_mut().unwrap().edit.insert(" edited");
            s.edited(tx);
        });
    }
    rt.update(|s, tx| {
        s.operate(
            Operation::Rename {
                from: old.clone(),
                to: new.clone(),
            },
            tx,
        )
    });
    rt.state.shutdown();
    assert!(!old.exists());
    for name in ["A.md", "B.md"] {
        assert_eq!(
            std::fs::read_to_string(new.join(name)).unwrap(),
            format!("{name} edited")
        );
    }
    assert!(
        rt.state
            .tabs
            .paths()
            .iter()
            .all(|path| path.starts_with(new.clone()))
    );
}

#[test]
fn f2_inline_rename_commits_on_enter_and_escape_cancels() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "text").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|_, tx| tx.focus("editor", false));
    press(&mut rt, RuneKey::Function(2), Mods::default());
    assert!(matches!(
        rt.state.file_ops.mode,
        file_ops::Mode::Rename { .. }
    ));
    rt.paste("CON");
    rt.frame(1. / 60.);
    press(&mut rt, RuneKey::Enter, Mods::default());
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(rt.state.active.as_ref(), Some(&fixture.0.join("_CON.md")));
    press(&mut rt, RuneKey::Function(2), Mods::default());
    press(&mut rt, RuneKey::Escape, Mods::default());
    assert!(matches!(rt.state.file_ops.mode, file_ops::Mode::Idle));
    assert!(fixture.0.join("_CON.md").exists());
}

#[test]
fn rename_preserves_dirty_buffers_undo_and_flushes_only_new_paths() {
    let fixture = Fixture::new();
    let from = fixture.0.join("Note.md");
    let to = fixture.0.join("Renamed.md");
    std::fs::write(&from, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" edited");
        s.edited(tx);
        s.operate(
            Operation::Rename {
                from: from.clone(),
                to: to.clone(),
            },
            tx,
        );
    });
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(rt.state.active.as_ref(), Some(&to));
    assert_eq!(rt.state.document().unwrap().edit.text, "original edited");
    assert!(!from.exists());
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "original edited");
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" later");
        s.edited(tx);
    });
    rt.state.shutdown();
    assert_eq!(
        std::fs::read_to_string(to).unwrap(),
        "original edited later"
    );
    assert!(!from.exists());
}

#[test]
fn external_changes_reload_clean_notes_but_keep_dirty_edits_and_pause_autosave() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, _| {
        s.external_change(Change {
            path: path.clone(),
            text: Some("external clean".into()),
        })
    });
    assert_eq!(rt.state.document().unwrap().edit.text, "external clean");
    rt.update(|s, tx| {
        let end = s.document().unwrap().edit.text.len();
        s.document_mut().unwrap().edit.set_caret(end);
        s.document_mut().unwrap().edit.insert(" local");
        s.edited(tx);
        s.external_change(Change {
            path: path.clone(),
            text: Some("external dirty".into()),
        });
    });
    rt.frame(2.);
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "external clean local"
    );
    assert!(rt.state.file_ops.conflicts.contains_key(&path));
    assert!(matches!(
        rt.state.document().unwrap().save,
        SaveState::Pending { .. }
    ));
    rt.update(|s, tx| s.close_tab(&path, tx));
    assert!(rt.state.tabs.paths().contains(&path));
    std::fs::write(&path, "latest disk content").unwrap();
    rt.update(|s, tx| s.reload_conflict(tx));
    until(&mut rt, |s| !s.files_busy());
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "latest disk content"
    );
    assert!(!rt.state.file_ops.conflicts.contains_key(&path));
}

#[test]
fn own_write_acknowledgements_do_not_create_conflicts_after_more_typing() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" saved");
        s.edited(tx);
        s.save_now(tx);
    });
    rt.state.writer.flush().unwrap();
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert(" pending");
        s.edited(tx);
        s.external_change(Change {
            path: path.clone(),
            text: Some("original saved".into()),
        });
    });
    assert!(rt.state.file_ops.conflicts.is_empty());
    assert_eq!(
        rt.state.document().unwrap().edit.text,
        "original saved pending"
    );
}

#[test]
fn empty_space_folder_creation_and_failed_rename_recover() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    rt.update(|s, tx| s.operate(Operation::Folder(fixture.0.join("Folder")), tx));
    until(&mut rt, |s| !s.files_busy());
    assert!(fixture.0.join("Folder").is_dir());
    rt.update(|s, tx| {
        s.operate(
            Operation::Rename {
                from: fixture.0.join("missing.md"),
                to: fixture.0.join("new.md"),
            },
            tx,
        )
    });
    until(&mut rt, |s| !s.files_busy());
    assert!(rt.state.error.is_some());
    assert!(!fixture.0.join("new.md").exists());
}
