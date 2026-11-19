use super::*;

type Scene = (&'static str, fn(&mut State));

#[cfg(debug_assertions)]
pub(super) const ALL: &[Scene] = &[
    ("spellcheck", spellcheck),
    ("update-notice", update_notice),
    ("palette-commands-light", commands_light),
    ("palette-commands-dark", commands_dark),
    ("palette-notes-light", notes_light),
    ("palette-notes-dark", notes_dark),
    ("overall", overall),
    ("polish-light", polish_light),
    ("polish-dark", polish_dark),
];
#[cfg(not(debug_assertions))]
pub(super) const ALL: &[Scene] = &[];

#[cfg(debug_assertions)]
fn spellcheck(s: &mut State) {
    let path = s.root.join("Spelling.md");
    s.settings.set_spellcheck(true);
    s.settings.kv.set("spell_lang", "en");
    s.settings.kv.set("theme", "light");
    s.settings.set_updates(false);
    s.load_document(path.clone(), "# A little help with spelling\n\nWrite freely. Keep your ideas close.\n\nA helo from your local Markdown editor.\n\n`wrng code` and [a lnik](https://example.com) stay untouched.\n".into());
    s.tabs.open(path.clone());
    s.active = Some(path);
    s.document_mut().unwrap().edit.set_caret(0);
}
#[cfg(debug_assertions)]
fn update_notice(s: &mut State) {
    spellcheck(s);
    s.services.phase = services::Phase::Available(
        crate::core::update::Release {
            version: "0.1.6".into(),
        },
        crate::core::update::Install::Manual,
    );
}

#[cfg(debug_assertions)]
fn overall(s: &mut State) {
    s.settings.set_spellcheck(false);
    s.settings.set_updates(false);
    s.settings.kv.set("theme", "light");
    let path = s.root.join("A quiet workspace.md");
    s.load_document(path.clone(), "# A quiet workspace\n\nA little room for your ideas.\n\n## Today\n\n- [x] Make something useful\n- [ ] Leave space to think\n\nWrite in **Markdown**, stay in the flow.\n\n> Everything stays in your local files.\n".into());
    s.tabs.open(path.clone());
    s.active = Some(path);
    s.document_mut().unwrap().edit.set_caret(0);
}
#[cfg(debug_assertions)]
fn palette(s: &mut State, commands: bool, dark: bool) {
    overall(s);
    s.settings
        .kv
        .set("theme", if dark { "dark" } else { "light" });
    s.open_palette(commands, &mut Tx::default());
}
#[cfg(debug_assertions)]
fn commands_light(s: &mut State) {
    palette(s, true, false);
}
#[cfg(debug_assertions)]
fn commands_dark(s: &mut State) {
    palette(s, true, true);
}
#[cfg(debug_assertions)]
fn notes_light(s: &mut State) {
    palette(s, false, false);
}
#[cfg(debug_assertions)]
fn notes_dark(s: &mut State) {
    palette(s, false, true);
}

#[cfg(debug_assertions)]
fn polish_light(s: &mut State) {
    s.scans = Scans::new();
    s.scanning = false;
    s.restore_started = true;
    s.settings.set_spellcheck(false);
    s.settings.set_updates(false);
    s.settings.kv.set("theme", "light");
    let path = s.root.join("Welcome to abstract.md");
    let source = "# Welcome to abstract\n\nA minimal markdown editor that renders as you type — **bold**, *italic* and `inline code` all show up instantly. Your notes stay plain files on disk.\n\n- plain text files\n  - nested under real folders\n- wiki-links and backlinks\n\n- [ ] try the tour (F1)\n- [x] open a folder as a space\n\n```rust\nfn main() {\n    println!(\"local-first\");\n}\n```\n";
    s.load_document(path.clone(), source.into());
    s.tabs.open(path.clone());
    s.active = Some(path.clone());
    s.document_mut()
        .unwrap()
        .edit
        .set_caret(source.find("\n\n```").unwrap() + 1);
    s.tree = [
        (path, "Welcome to abstract"),
        (s.root.join("- item one.md"), "- item one"),
    ]
    .into_iter()
    .map(|(path, name)| vault::Node {
        path,
        name: name.into(),
        kind: vault::NodeKind::Note,
        modified: std::time::SystemTime::UNIX_EPOCH,
        children: vec![],
    })
    .collect();
}

#[cfg(debug_assertions)]
fn polish_dark(s: &mut State) {
    polish_light(s);
    s.settings.kv.set("theme", "dark");
}
