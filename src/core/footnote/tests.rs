use super::*;

fn labels(text: &str) -> Vec<(&str, bool)> {
    parse(text, |_| false)
        .into_iter()
        .map(|f| (&text[f.label], f.def))
        .collect()
}

#[test]
fn references_and_definitions() {
    let text = "One[^1] and [^note].\n\n[^1]: First.\n   [^note]: Second [^1]:";
    assert_eq!(
        labels(text),
        [
            ("1", false),
            ("note", false),
            ("1", true),
            ("note", true),
            ("1", false)
        ]
    );
    let def = &parse(text, |_| false)[2];
    assert_eq!(&text[def.range.clone()], "[^1]:");
}

#[test]
fn rejects_bad_labels_escapes_and_code() {
    assert!(labels("[^] [^a b] [^x \\[^y]").is_empty());
    let text = "`[^1]` [^2]";
    let code = |o: usize| o < 6;
    let found: Vec<&str> = parse(text, code)
        .iter()
        .map(|f| &text[f.label.clone()])
        .collect();
    assert_eq!(found, ["2"]);
}

#[test]
fn insertion_numbers_and_appends() {
    let text = "Hello world.\n";
    let notes = parse(text, |_| false);
    let (r, new, caret) = insertion(text, &notes, 5);
    let mut out = text.to_string();
    out.replace_range(r.clone(), &new);
    assert_eq!(out, "Hello[^1] world.\n\n[^1]: ");
    assert_eq!(caret, out.len());

    let text = "A[^1] b[^7].\n\n[^1]: x\n[^7]: y\n";
    let notes = parse(text, |_| false);
    let (r, new, _) = insertion(text, &notes, 12);
    let mut out = text.to_string();
    out.replace_range(r, &new);
    assert_eq!(out, "A[^1] b[^7].[^8]\n\n[^1]: x\n[^7]: y\n[^8]: ");

    let (r, new, _) = insertion("", &[], 0);
    assert_eq!((r, new.as_str()), (0..0, "[^1]\n\n[^1]: "));
}

#[test]
fn jumps_between_reference_and_definition() {
    let text = "See[^a].\n\n[^a]: Note.";
    let notes = parse(text, |_| false);
    assert_eq!(
        target(text, &notes, &notes[0]),
        Some(text.find("Note").unwrap())
    );
    assert_eq!(target(text, &notes, &notes[1]), Some(5));
    assert_eq!(
        target(
            "[^x]",
            &parse("[^x]", |_| false),
            &parse("[^x]", |_| false)[0]
        ),
        None
    );
}
