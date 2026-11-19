use super::*;
use crate::core::update::{Install, Release};
use crate::io::update::tests::mock;

fn spans(s: &State) -> Vec<rune::Span> {
    s.spell_spans(theme::resolve(&s.settings, false).0)
}
#[test]
fn spelling_menu_replaces_undoes_ignores_and_toggle_hides_stale_spans() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("Note.md"), "helo world").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    rt.update(|s, tx| {
        s.settings.set_spellcheck(true);
        s.settings.kv.set("spell_lang", "en");
        tx.focus("editor", false);
    });
    until(&mut rt, |s| !spans(s).is_empty());
    assert_eq!((spans(&rt.state)[0].start, spans(&rt.state)[0].end), (0, 4));
    press(&mut rt, RuneKey::Function(7), Mods::default());
    until(&mut rt, |s| {
        spelling::overlay(s, theme::resolve(&s.settings, false).0).is_some()
    });
    until(&mut rt, |_| true);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !rt
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some("spell-choice-0"))
    {
        assert!(Instant::now() < deadline);
        rt.frame(1. / 60.);
        std::thread::sleep(Duration::from_millis(2));
    }
    rt.update(|_, tx| tx.focus("spell-choice-0", false));
    press(&mut rt, RuneKey::Enter, Mods::default());
    assert_ne!(rt.state.document().unwrap().edit.text, "helo world");
    press(
        &mut rt,
        RuneKey::Command('z'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.document().unwrap().edit.text, "helo world");
    until(&mut rt, |s| !spans(s).is_empty());
    press(&mut rt, RuneKey::Function(7), Mods::default());
    rt.update(|_, tx| tx.focus("spell-ignore", false));
    press(&mut rt, RuneKey::Enter, Mods::default());
    until(&mut rt, |s| spans(s).is_empty());
    rt.update(|s, tx| {
        s.settings.set_spellcheck(false);
        s.spell_suggestions(tx);
    });
    assert!(spelling::overlay(&rt.state, theme::resolve(&rt.state.settings, false).0).is_none());
}
#[test]
fn background_update_after_readiness_respects_setting_and_timestamp() {
    for enabled in [false, true] {
        let fixture = Fixture::new();
        let mut rt = fixture.runtime();
        let (worker, calls) = mock(Ok("99.0.0"), false);
        rt.state.services.worker = Some(worker);
        rt.state.settings.set_updates(enabled);
        rt.frame(1. / 60.);
        assert!(calls.lock().unwrap().is_empty());
        rt.update(State::services_ready);
        if enabled {
            until(&mut rt, |s| {
                matches!(s.services.phase, services::Phase::Available(..))
            });
            until(&mut rt, |s| s.settings_pending == 0);
            assert!(
                std::fs::read_to_string(fixture.0.join("settings"))
                    .unwrap()
                    .contains("update_checked = ")
            );
            rt.update(|s, tx| {
                s.settings.set_updates(false);
                s.update_settings_changed(tx);
            });
            assert!(matches!(rt.state.services.phase, services::Phase::Quiet));
        } else {
            assert!(calls.lock().unwrap().is_empty());
        }
    }
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    let (worker, calls) = mock(Ok("99.0.0"), false);
    rt.state.services.worker = Some(worker);
    rt.state.settings.set_update_checked(u64::MAX);
    rt.update(State::services_ready);
    assert!(calls.lock().unwrap().is_empty());
}
#[test]
fn failed_update_stays_nonmodal_and_no_new_release_is_quiet() {
    for version in [Ok("0.0.1"), Err("offline")] {
        let fixture = Fixture::new();
        let mut rt = fixture.runtime();
        let (worker, _) = mock(version, false);
        rt.state.services.worker = Some(worker);
        rt.update(State::services_ready);
        until(&mut rt, |s| {
            matches!(s.services.phase, services::Phase::Quiet)
        });
    }
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    let (worker, calls) = mock(Ok("99.0.0"), true);
    rt.state.services.worker = Some(worker);
    rt.state.services.phase = services::Phase::Available(
        Release {
            version: "99.0.0".into(),
        },
        Install::Tarball("abstract".into()),
    );
    rt.update(State::install_update);
    until(&mut rt, |s| {
        matches!(s.services.phase, services::Phase::Failed(..))
    });
    assert_eq!(*calls.lock().unwrap(), vec!["install"]);
}
#[test]
fn update_waits_for_saves_and_conflicts_block_relaunch() {
    let fixture = Fixture::new();
    let path = fixture.0.join("Note.md");
    std::fs::write(&path, "original").unwrap();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| s.document().is_some());
    let (worker, calls) = mock(Ok("99.0.0"), false);
    rt.state.services.worker = Some(worker);
    rt.update(|s, tx| {
        s.document_mut().unwrap().edit.insert("edited");
        s.edited(tx);
        s.file_ops.conflicts.insert(path.clone(), None);
        s.services.phase = services::Phase::Available(
            Release {
                version: "99.0.0".into(),
            },
            Install::Tarball("abstract".into()),
        );
        s.install_update(tx);
    });
    until(&mut rt, |s| {
        matches!(s.services.phase, services::Phase::RestartFailed(..))
    });
    assert_eq!(*calls.lock().unwrap(), vec!["install"]);
    rt.update(|s, tx| {
        s.file_ops.conflicts.clear();
        s.install_update(tx);
    });
    until(&mut rt, |_| calls.lock().unwrap().contains(&"relaunch"));
    assert!(std::fs::read_to_string(path).unwrap().contains("edited"));
}
