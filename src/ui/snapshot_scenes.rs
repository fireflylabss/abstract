use super::*;

type Scene = (&'static str, fn(&mut State));

#[cfg(debug_assertions)]
pub(super) const ALL: &[Scene] = &[("spellcheck", spellcheck), ("update-notice", update_notice)];
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
