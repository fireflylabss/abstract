use super::*;

#[test]
fn expanded_folders_roundtrip_relative_paths_per_space() {
    let mut session = Session::default();
    let space = Path::new("/space");
    let other = Path::new("/other");
    assert!(session.expanded(space).is_empty());
    session.set_expanded(other, &["kept".into()]);
    session.set_expanded(
        space,
        &["folder/nested".into(), "folder".into(), "folder".into()],
    );
    let mut restored = Session {
        kv: KeyVals::parse(&session.snapshot().1),
    };
    assert_eq!(
        restored.expanded(space),
        [PathBuf::from("folder"), "folder/nested".into()]
    );
    assert_eq!(restored.expanded(other), [PathBuf::from("kept")]);
    restored.set_expanded(space, &[]);
    assert!(restored.expanded(space).is_empty());
    assert_eq!(restored.expanded(other), [PathBuf::from("kept")]);
}

#[test]
fn expanded_folders_reject_unsafe_and_unrepresentable_paths() {
    let space = Path::new("/space");
    let mut session = Session {
        kv: KeyVals::parse(
            "sidebar = open\nexpanded = /space\t../outside\nexpanded = /space\t/absolute\nexpanded = /space\t\nexpanded = malformed\nexpanded = /space\tgood\textra\n",
        ),
    };
    assert!(session.expanded(space).is_empty());
    session.set_expanded(
        space,
        &[
            "".into(),
            "..".into(),
            "../outside".into(),
            "/absolute".into(),
            "folder\nname".into(),
            "folder\tname".into(),
        ],
    );
    assert!(session.expanded(space).is_empty());
    let old = Session {
        kv: KeyVals::parse("sidebar = closed\ntab = /space\tnote.md\t0\t0\n"),
    };
    assert!(old.expanded(space).is_empty());
    assert_eq!(old.tabs().len(), 1);
}

#[test]
fn settings_roundtrip_preserves_unknown_keys() {
    let mut s = Settings {
        kv: KeyVals::parse("theme = dark\nfuture-key = 1 2\ntour = done\n"),
    };
    assert_eq!(s.theme(), ThemePref::Dark);
    assert!(s.tour_done());
    s.set_theme(ThemePref::Light);
    let text = s.kv.serialize();
    let again = Settings {
        kv: KeyVals::parse(&text),
    };
    assert_eq!(again.theme(), ThemePref::Light);
    assert!(again.tour_done());
    assert_eq!(again.kv.get("future-key"), Some("1 2"));
    // One `theme` line only.
    assert_eq!(text.matches("theme").count(), 1);
}

#[test]
fn settings_defaults_on_garbage() {
    let s = Settings {
        kv: KeyVals::parse("not a key value line\n===\ntheme\n"),
    };
    assert_eq!(s.theme(), ThemePref::System);
    assert!(!s.tour_done());
}

#[test]
fn appearance_settings_default_and_roundtrip() {
    let mut s = Settings::default();
    assert!(!s.raw_tables());
    assert_eq!((s.light_theme(), s.dark_theme()), ("abstract", "abstract"));
    s.set_raw_tables(true);
    s.set_light_theme("sepia");
    s.set_dark_theme("nord");
    let again = Settings {
        kv: KeyVals::parse(&s.kv.serialize()),
    };
    assert!(again.raw_tables());
    assert_eq!((again.light_theme(), again.dark_theme()), ("sepia", "nord"));
    s.set_raw_tables(false);
    assert!(!s.raw_tables());
}

#[test]
fn session_roundtrip() {
    let mut s = Session {
        kv: KeyVals::parse(
            "window = maximized 10 20 1100 720\nsidebar = closed\nweird\nnote = /a\tb/c.md\t12\t3.5\nnote = /a\td.md\t0\t0\n",
        ),
    };
    let w = s.window().unwrap();
    assert!(w.maximized && w.w == 1100.);
    assert_eq!(s.sidebar_open(), Some(false));
    let notes = s.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].rel, PathBuf::from("b/c.md"));
    assert_eq!(notes[0].cursor, 12);
    assert_eq!(notes[0].scroll, 3.5);

    s.set_window(&SessionWindow {
        maximized: false,
        x: 1.,
        y: 2.,
        w: 800.,
        h: 600.,
    });
    s.set_sidebar(true);
    let mut notes = notes;
    notes.push(SessionNote {
        space: PathBuf::from("/tab\tbed"),
        rel: PathBuf::from("x.md"),
        cursor: 0,
        scroll: 0.,
    });
    s.set_notes(&notes);
    let again = Session {
        kv: KeyVals::parse(&s.kv.serialize()),
    };
    assert_eq!(again.window().unwrap().w, 800.);
    assert!(!again.window().unwrap().maximized);
    assert_eq!(again.sidebar_open(), Some(true));
    // Tab in path → skipped on write; the two good ones survive.
    assert_eq!(again.notes().len(), 2);
    assert_eq!(again.kv.get("weird"), None);
    assert!(again.kv.lines.iter().any(|l| l == "weird"));
}

#[test]
fn session_malformed_values() {
    let s = Session {
        kv: KeyVals::parse("window = banana\nsidebar = maybe\nnote = /a\tb.md\n"),
    };
    assert_eq!(s.window(), None);
    assert_eq!(s.sidebar_open(), None);
    assert!(s.notes().is_empty());
}

#[test]
fn session_tabs_roundtrip_and_note_fallback() {
    let space = PathBuf::from("/s");
    let notes = vec![
        SessionNote {
            space: space.clone(),
            rel: PathBuf::from("a.md"),
            cursor: 1,
            scroll: 0.5,
        },
        SessionNote {
            space: space.clone(),
            rel: PathBuf::from("b.md"),
            cursor: 0,
            scroll: 0.,
        },
    ];
    let mut s = Session::default();
    s.set_tabs(&notes);
    s.set_active_tab(Some((&space, Path::new("b.md"))));
    let again = Session {
        kv: KeyVals::parse(&s.kv.serialize()),
    };
    assert_eq!(again.tabs(), notes);
    assert_eq!(again.active_tab(), Some((space.clone(), "b.md".into())));
    // Pre-tab `note` lines restore as a single-tab space.
    let legacy = Session {
        kv: KeyVals::parse("note = /s\tc.md\t3\t1.0\n"),
    };
    let tabs = legacy.tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].rel, PathBuf::from("c.md"));
    assert_eq!(tabs[0].cursor, 3);
    // A `tab` line wins over `note` lines for the same space.
    let mixed = Session {
        kv: KeyVals::parse("note = /s\told.md\t0\t0\ntab = /s\tnew.md\t0\t0\n"),
    };
    let tabs = mixed.tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].rel, PathBuf::from("new.md"));
    // `tab_active` is removed when nothing is open.
    s.set_active_tab(None);
    let again = Session {
        kv: KeyVals::parse(&s.kv.serialize()),
    };
    assert_eq!(again.active_tab(), None);
}

#[test]
fn session_tabs_keep_strip_order_with_active_in_the_middle() {
    let space = PathBuf::from("/s");
    let n = |rel: &str| SessionNote {
        space: space.clone(),
        rel: PathBuf::from(rel),
        cursor: 0,
        scroll: 0.,
    };
    let notes = vec![n("a.md"), n("b.md"), n("c.md")];
    let mut s = Session::default();
    s.set_tabs(&notes);
    s.set_active_tab(Some((&space, Path::new("b.md"))));
    // The serialized `tab` lines keep strip order — active stays put.
    let tab_lines: Vec<&str> =
        s.kv.lines
            .iter()
            .filter(|l| l.starts_with("tab "))
            .map(String::as_str)
            .collect();
    assert_eq!(tab_lines.len(), 3);
    assert!(tab_lines[0].contains("\ta.md\t"));
    assert!(tab_lines[1].contains("\tb.md\t"));
    assert!(tab_lines[2].contains("\tc.md\t"));
    let again = Session {
        kv: KeyVals::parse(&s.kv.serialize()),
    };
    assert_eq!(again.tabs(), notes);
    assert_eq!(again.active_tab(), Some((space, "b.md".into())));
}

#[test]
fn write_atomic_writes_and_overwrites() {
    let dir = std::env::temp_dir().join(format!("abstract-store-test-{}", std::process::id()));
    let file = dir.join("nested").join("out.txt");
    write_atomic(&file, b"one").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "one");
    write_atomic(&file, b"two").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
    assert!(!dir.join("nested").join(".out.txt.abstract-tmp").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn text_width_setting_defaults_and_roundtrip() {
    let mut s = Settings::default();
    // Missing reads as medium.
    assert_eq!(s.text_width(), TextWidth::Medium);
    for w in TextWidth::ALL {
        s.set_text_width(w);
        let again = Settings {
            kv: KeyVals::parse(&s.kv.serialize()),
        };
        assert_eq!(again.text_width(), w);
        assert_eq!(again.kv.get("text_width"), Some(w.as_str()));
    }
    // Garbage also reads as medium.
    let bad = Settings {
        kv: KeyVals::parse("text_width = banana\n"),
    };
    assert_eq!(bad.text_width(), TextWidth::Medium);
}

#[test]
fn xdg_explicit_var_wins() {
    // Unique var name: env mutation in tests is racy, so only the
    // explicit-var branch is exercised here.
    const VAR: &str = "ABSTRACT_XDG_TEST_VAR_9F3B";
    unsafe { std::env::set_var(VAR, "/tmp/abstract-xdg-wins") };
    assert_eq!(
        xdg(VAR, "fallback"),
        PathBuf::from("/tmp/abstract-xdg-wins")
    );
    unsafe { std::env::remove_var(VAR) };
}

#[test]
fn glass_defaults_and_roundtrip() {
    let mut s = Settings::default();
    // Missing keys read as the per-OS default material, full strength,
    // sidebar+tabs at 75%, everything else opaque, theme tint.
    assert_eq!(s.glass(), crate::core::prefs::default_enabled());
    assert_eq!(
        s.glass_material(),
        crate::core::prefs::Material::default_for_os()
    );
    assert_eq!(s.glass_intensity(), 100);
    assert_eq!(
        s.glass_surface(crate::core::prefs::Surface::Sidebar),
        Some(75)
    );
    assert_eq!(s.glass_surface(crate::core::prefs::Surface::Tabs), Some(75));
    for surf in crate::core::prefs::Surface::ALL {
        let want = surf.default_percent();
        assert_eq!(s.glass_surface(surf), want, "{surf:?}");
    }
    assert!(!s.glass_tint());
    assert_eq!(s.glass_tint_color(), crate::core::prefs::DEFAULT_TINT_COLOR);
    assert_eq!(s.glass_tint_strength(), 50);
    assert_eq!(s.glass_text_opacity(), 100);
    assert!(!s.glass_text_contrast());

    // Round-trip through serialization.
    s.set_glass(false);
    s.set_glass_material(crate::core::prefs::Material::default_for_os());
    s.set_glass_intensity(42);
    s.set_glass_surface(crate::core::prefs::Surface::Sidebar, Some(60));
    s.set_glass_surface(crate::core::prefs::Surface::Menus, None);
    s.set_glass_tint(true);
    s.set_glass_tint_color(0xff8800);
    s.set_glass_tint_strength(25);
    s.set_glass_text_opacity(80);
    s.set_glass_text_contrast(true);
    let again = Settings {
        kv: KeyVals::parse(&s.kv.serialize()),
    };
    assert!(!again.glass());
    assert_eq!(again.glass_intensity(), 42);
    assert_eq!(
        again.glass_surface(crate::core::prefs::Surface::Sidebar),
        Some(60)
    );
    assert_eq!(
        again.glass_surface(crate::core::prefs::Surface::Menus),
        None
    );
    assert_eq!(again.kv.get("glass_menus"), Some("off"));
    assert!(again.glass_tint());
    assert_eq!(again.glass_tint_color(), 0xff8800);
    assert_eq!(again.kv.get("glass_tint_color"), Some("ff8800"));
    assert_eq!(again.glass_tint_strength(), 25);
    assert_eq!(again.glass_text_opacity(), 80);
    assert!(again.glass_text_contrast());

    // Garbage and out-of-range values fall back to defaults/clamps.
    let bad = Settings {
        kv: KeyVals::parse(
            "glass_intensity = banana\nglass_tint_color = ffffff00\nglass_sidebar = 300\n",
        ),
    };
    assert_eq!(bad.glass_intensity(), 100);
    assert_eq!(
        bad.glass_tint_color(),
        crate::core::prefs::DEFAULT_TINT_COLOR
    );
    // Out-of-range is unparseable, so the surface default wins.
    assert_eq!(
        bad.glass_surface(crate::core::prefs::Surface::Sidebar),
        Some(75)
    );
    // A material saved on another OS parses to one valid here.
    let foreign = Settings {
        kv: KeyVals::parse("glass_material = mica\n"),
    };
    assert!(crate::core::prefs::Material::options().contains(&foreign.glass_material()));
}
