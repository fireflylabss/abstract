use super::*;

#[test]
#[ignore = "release-only interactive edit benchmark"]
fn large_note_keystroke_medians() {
    let fixture = Fixture::new();
    let mut rt = fixture.runtime();
    until(&mut rt, |s| !s.scanning && s.files_pending == 0);
    for size in [200 * 1024, 1024 * 1024] {
        let paragraph = "A calm paragraph with **bold** and [a link](Note.md).\n\n";
        let mut source = paragraph.repeat(size / paragraph.len() + 1);
        source.truncate(size);
        let path = fixture.0.join(format!("benchmark-{size}.md"));
        std::fs::write(&path, &source).unwrap();
        rt.update(|s, tx| {
            s.load_document(path.clone(), source.clone());
            s.tabs.open(path.clone());
            s.focus_tab(tx);
            s.settings.kv.set("autosave_delay", "10");
        });
        until(&mut rt, |s| {
            s.document()
                .is_some_and(|d| d.analysis.get(d.revision).is_some())
        });
        let mut samples = vec![];
        for _ in 0..31 {
            let start = Instant::now();
            rt.key(KeyEvent {
                key: RuneKey::Text("x".into()),
                mods: Mods::default(),
            });
            rt.frame(1. / 60.);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        assert_eq!(rt.state.document().unwrap().edit.text.len(), size + 31);
        samples.sort_by(f64::total_cmp);
        println!(
            "{} KiB: headless keystroke + frame median {:.3} ms (31 samples, release, spellcheck off, no GPU presentation)",
            size / 1024,
            samples[15]
        );
    }
}
