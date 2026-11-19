use super::*;

fn at(src: &str, hits: &[(Range<usize>, u16)], s: &str) -> u16 {
    let b = src.find(s).unwrap();
    hits.iter()
        .find(|(r, _)| r.contains(&b))
        .map(|(_, f)| *f)
        .unwrap_or(0)
}

#[test]
fn rust_strings_and_comments() {
    let src = "let x = \"a // not comment\"; // real";
    let h = highlight("rust", src);
    assert_eq!(at(src, &h, "let"), KEYWORD);
    assert_eq!(at(src, &h, "x ="), 0);
    assert_eq!(at(src, &h, "a // not comment"), STRING);
    assert_eq!(at(src, &h, "real"), COMMENT);
    // Rust lifetime is not a string.
    let h = highlight("rust", "fn f<'a>(x: &'a str) {}");
    assert!(h.iter().all(|(_, f)| *f != STRING));
    let h = highlight("rust", "let c = 'x';");
    assert_eq!(at("let c = 'x';", &h, "'x'"), STRING);
}

#[test]
fn python_comment_string_keyword() {
    let src = "# c\ns = 'x'\ndef f(): 42";
    let h = highlight("python", src);
    assert_eq!(at(src, &h, "# c"), COMMENT);
    assert_eq!(at(src, &h, "'x'"), STRING);
    assert_eq!(at(src, &h, "def"), KEYWORD);
    assert_eq!(at(src, &h, "42"), NUMBER);
    let h = highlight("python", "t = \"\"\"docstring\"\"\"");
    assert_eq!(at("t = \"\"\"docstring\"\"\"", &h, "docstring"), STRING);
}

#[test]
fn json_keys_are_keywords() {
    let src = "{\"name\": 1, \"aéb\": \"v\"}";
    let h = highlight("json", src);
    assert_eq!(at(src, &h, "name"), STRING); // key is inside quotes
    assert_eq!(at(src, &h, "1"), NUMBER);
    let h = highlight("yaml", "key: value\nother: 2");
    assert_eq!(at("key: value\nother: 2", &h, "key"), KEYWORD);
    assert_eq!(at("key: value\nother: 2", &h, "other"), KEYWORD);
    assert_eq!(at("key: value\nother: 2", &h, "2"), NUMBER);
}

#[test]
fn unknown_lang_keeps_generic() {
    let h = highlight("cobol", "x = \"s\" // c\n42");
    assert_eq!(at("x = \"s\" // c\n42", &h, "s\""), STRING);
    assert_eq!(at("x = \"s\" // c\n42", &h, "c"), COMMENT);
    assert_eq!(at("x = \"s\" // c\n42", &h, "42"), NUMBER);
}

#[test]
fn utf8_boundaries() {
    let src = "s = \"é x\" # é\névar = 1";
    let h = highlight("python", src);
    for (r, _) in &h {
        let _ = &src[r.clone()]; // must not panic → boundaries valid
    }
    assert_eq!(at(src, &h, "é x"), STRING);
    assert_eq!(at(src, &h, "# é"), COMMENT);
    assert_eq!(at(src, &h, "1"), NUMBER);
}
