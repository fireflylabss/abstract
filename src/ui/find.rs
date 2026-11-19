use super::*;
use crate::core::find as logic;
use crate::ui::theme::{Palette, rgba};
use rune::{Key as RuneKey, KeyEvent, Span, text_input};
use std::ops::Range;

pub(super) struct FindBar {
    pub(super) query: TextEdit,
    pub(super) replacement: TextEdit,
    pub(super) replacing: bool,
    pub(super) case: bool,
    pub(super) matches: Vec<Range<usize>>,
    pub(super) current: Option<usize>,
}

impl State {
    pub fn close_tools(&mut self, tx: &mut Tx) -> bool {
        if self.search.is_some() {
            self.close_search(tx);
            true
        } else if self.find.is_some() {
            self.close_find(tx);
            true
        } else {
            false
        }
    }
    pub fn open_find(&mut self, replacing: bool, tx: &mut Tx) {
        self.customization.open = false;
        let Some(doc) = self.document() else { return };
        let selection = &doc.edit.text
            [doc.edit.anchor.min(doc.edit.focus)..doc.edit.anchor.max(doc.edit.focus)];
        let seed =
            (!selection.is_empty() && !selection.contains('\n')).then(|| selection.to_owned());
        let bar = self.find.get_or_insert_with(|| FindBar {
            query: TextEdit::new(""),
            replacement: TextEdit::new(""),
            replacing: false,
            case: false,
            matches: vec![],
            current: None,
        });
        bar.replacing |= replacing;
        if let Some(seed) = seed {
            bar.query = TextEdit::new(seed);
        }
        bar.query.set_selection(0, bar.query.text.len());
        self.refresh_find(true, tx);
        tx.focus("find-query", false);
    }

    pub fn close_find(&mut self, tx: &mut Tx) {
        self.find = None;
        tx.focus("editor", false);
    }

    pub(super) fn refresh_find(&mut self, select: bool, tx: &mut Tx) {
        let Some(bar) = &self.find else { return };
        let (matches, at) = self
            .document()
            .map(|doc| {
                (
                    logic::matches(&doc.edit.text, &bar.query.text, bar.case),
                    doc.edit.anchor.min(doc.edit.focus),
                )
            })
            .unwrap_or_default();
        let current = logic::nearest(&matches, at);
        let bar = self.find.as_mut().unwrap();
        bar.matches = matches;
        bar.current = current;
        if select {
            self.select_find(tx);
        }
    }

    fn select_find(&mut self, tx: &mut Tx) {
        let range = self
            .find
            .as_ref()
            .and_then(|bar| bar.current.and_then(|i| bar.matches.get(i)))
            .cloned();
        if let Some(range) = range
            && let Some(doc) = self.document_mut()
        {
            doc.edit.set_selection(range.start, range.end);
            tx.scroll_into_view("find-current");
        }
    }

    pub fn find_step(&mut self, forward: bool, tx: &mut Tx) {
        if let Some(bar) = &mut self.find
            && !bar.matches.is_empty()
        {
            let len = bar.matches.len();
            let i = bar.current.unwrap_or(0);
            bar.current = Some(if forward {
                (i + 1) % len
            } else {
                (i + len - 1) % len
            });
            self.select_find(tx);
        }
    }

    pub fn replace_find(&mut self, all: bool, tx: &mut Tx) {
        let Some(bar) = &self.find else { return };
        let Some(range) = bar.current.and_then(|i| bar.matches.get(i)).cloned() else {
            return;
        };
        let replacement = bar.replacement.text.clone();
        let ranges = bar.matches.clone();
        let Some(doc) = self.document_mut() else {
            return;
        };
        let (range, text) = if all {
            (
                0..doc.edit.text.len(),
                logic::replace_all(&doc.edit.text, &ranges, &replacement),
            )
        } else {
            (range, replacement)
        };
        if doc.edit.text[range.clone()] == text {
            if !all {
                self.find_step(true, tx);
            }
            return;
        }
        doc.edit.undo_group(|edit| {
            edit.set_selection(range.start, range.end);
            edit.insert(&text);
        });
        self.edited(tx);
        self.refresh_find(true, tx);
    }

    pub fn find_spans(&self, p: Palette) -> Vec<Span> {
        self.find
            .as_ref()
            .map(|bar| {
                bar.matches
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        Span::new(r.start, r.end).bg(
                            rgba(if bar.current == Some(i) {
                                p.find_current
                            } else {
                                p.highlight
                            }),
                            1.,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn current_find(&self) -> Option<Range<usize>> {
        self.find
            .as_ref()
            .and_then(|bar| bar.current.and_then(|i| bar.matches.get(i)))
            .cloned()
    }
}

fn input_key(state: &mut State, key: &KeyEvent, tx: &mut Tx, replacement: bool) -> bool {
    if keymap::handle(state, key, tx) {
        return true;
    }
    if matches!(key.key, RuneKey::Enter) {
        if key.mods.alt {
            return true;
        }
        if replacement {
            state.replace_find(key.mods.cmd || key.mods.ctrl, tx);
        } else {
            state.find_step(!key.mods.shift, tx);
        }
        return true;
    }
    false
}

fn button(
    label: &'static str,
    title: &'static str,
    key: &'static str,
    p: Palette,
    action: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    text(label)
        .key(key)
        .role("button")
        .label(title)
        .focusable(true)
        .padding(5.)
        .color(rgb(p.dim))
        .hover(rgb(p.hover))
        .on_press_tx(action)
}

pub(super) fn view(state: &State, p: Palette) -> El {
    let Some(bar) = &state.find else {
        return row();
    };
    let count = format!("{}/{}", bar.current.map_or(0, |i| i + 1), bar.matches.len());
    let mut view = row()
        .key("find-bar")
        .w_full()
        .items_center()
        .gap(4.)
        .padding(5.)
        .fill(rgb(p.panel))
        .child(
            text_input(&bar.query, TextStyle::new(13., 400.))
                .key("find-query")
                .single_line()
                .label(t(Key::FindPlaceholder))
                .placeholder(t(Key::FindPlaceholder), rgb(p.faint))
                .grow(1.)
                .min_w(40.)
                .color(rgb(p.fg))
                .caret(rgb(p.caret), rgba(p.selection))
                .on_edit(|s: &mut State| &mut s.find.as_mut().unwrap().query)
                .on_input_change(|s: &mut State, tx| s.refresh_find(true, tx))
                .on_input_key(|s, key, tx| input_key(s, key, tx, false)),
        )
        .child(
            text(count)
                .key("find-count")
                .color(rgb(p.dim))
                .font(TextStyle::new(12., 400.)),
        )
        .child(button("↑", t(Key::FindPrev), "find-prev", p, |s, tx| {
            s.find_step(false, tx)
        }))
        .child(button("↓", t(Key::FindNext), "find-next", p, |s, tx| {
            s.find_step(true, tx)
        }))
        .child(
            button("Aa", t(Key::MatchCase), "find-case", p, |s, tx| {
                if let Some(bar) = &mut s.find {
                    bar.case = !bar.case;
                }
                s.refresh_find(true, tx);
            })
            .fill(rgb(if bar.case { p.active } else { p.panel })),
        )
        .child(button(
            "⇄",
            t(Key::ToggleReplace),
            "find-toggle-replace",
            p,
            |s, tx| {
                if let Some(bar) = &mut s.find {
                    bar.replacing = !bar.replacing;
                }
                tx.focus("find-query", false);
            },
        ));
    if bar.replacing {
        view = view
            .child(
                text_input(&bar.replacement, TextStyle::new(13., 400.))
                    .key("find-replacement")
                    .single_line()
                    .label(t(Key::ReplacePlaceholder))
                    .placeholder(t(Key::ReplacePlaceholder), rgb(p.faint))
                    .grow(1.)
                    .min_w(40.)
                    .color(rgb(p.fg))
                    .caret(rgb(p.caret), rgba(p.selection))
                    .on_edit(|s: &mut State| &mut s.find.as_mut().unwrap().replacement)
                    .on_input_key(|s, key, tx| input_key(s, key, tx, true)),
            )
            .child(button(
                "↵",
                t(Key::ReplaceOne),
                "find-replace-one",
                p,
                |s, tx| s.replace_find(false, tx),
            ))
            .child(button(
                "∀",
                t(Key::ReplaceAll),
                "find-replace-all",
                p,
                |s, tx| s.replace_find(true, tx),
            ));
    }
    view.child(button(
        "×",
        t(Key::CloseFind),
        "find-close",
        p,
        State::close_find,
    ))
}
