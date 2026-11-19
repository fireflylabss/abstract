use super::*;

#[test]
fn all_keys_filled_in_both_languages() {
    for &k in Key::ALL {
        assert!(!en(k).is_empty());
        assert!(!pt(k).is_empty());
        // Placeholder sets must match across languages.
        let ph = |s: &str| {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|x| x.split('}').next().map(String::from))
                .collect();
            v.sort();
            v
        };
        assert_eq!(ph(en(k)), ph(pt(k)), "placeholder mismatch: {:?}", en(k));
    }
}

#[test]
fn tf_replaces_placeholders() {
    // No `set` — the global language races with parallel tests; the
    // default is PtBr, and MOD differs per platform.
    assert_eq!(tf(Key::Words, &[("n", "5")]), "5 palavras");
    assert_eq!(lookup(Lang::En, Key::Words), "{n} words");
    assert!(tf(Key::Sidebar, &[]).contains(&format!("{}+\\", crate::core::prefs::MOD)));
}

#[test]
fn shortcuts_use_the_platform_modifier() {
    for &k in Key::ALL {
        for lang in [Lang::En, Lang::PtBr] {
            assert!(
                !lookup(lang, k).contains("Ctrl"),
                "hard-coded Ctrl: {}",
                en(k)
            );
        }
    }
}
