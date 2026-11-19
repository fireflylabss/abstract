use super::*;
use crate::core::store::{KeyVals, Settings};

#[test]
fn defaults_and_invalid_values_are_finite_and_bounded() {
    for p in preferences() {
        assert!(!p.value(None).is_empty(), "{}", p.key);
        for bad in ["", "NaN", "inf", "invalid"] {
            assert_eq!(p.value(Some(bad)), p.value(None), "{}", p.key);
        }
        if let Input::Number { min, max, .. } = p.input {
            assert_eq!(p.value(Some("-1000")).parse::<f32>().unwrap(), min);
            assert_eq!(p.value(Some("1000")).parse::<f32>().unwrap(), max);
        }
    }
}

#[test]
fn stepping_and_legacy_settings_preserve_hidden_keys() {
    let mut s = Settings {
        kv: KeyVals::parse("font = Georgia\ndiscord = off\nunknown = future\nspell_lang = both\n"),
    };
    assert_eq!(s.preference("ui_font"), "Georgia");
    assert_eq!(s.preference("editor_font"), "Georgia");
    for p in preferences() {
        let v = p.step(s.kv.get(p.key), true);
        assert_eq!(p.value(Some(&v)), v);
        s.kv.set(p.key, &v);
    }
    let text = s.kv.serialize();
    for line in ["font = Georgia", "discord = off", "unknown = future"] {
        assert!(text.contains(line));
    }
    assert!(
        !preferences()
            .iter()
            .any(|p| matches!(p.key, "discord" | "raw_tables" | "tour"))
    );
}

#[test]
fn custom_legacy_colors_and_fonts_and_invalid_zoom_are_safe() {
    let s = Settings {
        kv: KeyVals::parse("glass_tint_color = #FF0033\nfont = Custom Family\nzoom = NaN\n"),
    };
    assert_eq!(s.preference("glass_tint_color"), "ff0033");
    assert_eq!(s.glass_tint_color(), 0xff0033);
    assert_eq!(s.preference("ui_font"), "Custom Family");
    assert_eq!(s.preference("editor_font"), "Custom Family");
    assert_eq!(s.zoom(), 1.);
}
