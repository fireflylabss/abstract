use super::*;
use crate::core::search::Hit;
use crate::io::search::Searches;
use crate::ui::theme::{Palette, rgba};
use rune::{Key as RuneKey, KeyEvent, text_input};

pub(super) struct SearchBar {
    pub(super) query: TextEdit,
    pub(super) hits: Vec<Hit>,
    pub(super) selected: usize,
    worker: Searches,
}

impl State {
    pub fn open_search(&mut self, tx: &mut Tx) {
        self.customization.open = false;
        self.search.get_or_insert_with(|| SearchBar {
            query: TextEdit::new(""),
            hits: vec![],
            selected: 0,
            worker: Searches::new(),
        });
        self.start_search(tx);
        tx.focus("global-query", false);
    }

    pub fn close_search(&mut self, tx: &mut Tx) {
        self.search = None;
        tx.focus("editor", false);
    }

    pub(super) fn start_search(&mut self, tx: &mut Tx) {
        if self.search.is_none() {
            return;
        }
        let buffers = self.buffers();
        if let Some(search) = &mut self.search {
            search.selected = 0;
            search.hits.clear();
            search
                .worker
                .start(self.root.clone(), search.query.text.clone(), buffers);
            tx.wake_after(Duration::from_millis(16));
        }
    }

    pub(super) fn poll_search(&mut self) -> bool {
        if let Some(search) = &mut self.search
            && let Some(hits) = search.worker.poll()
        {
            search.hits = hits;
            return true;
        }
        false
    }

    pub(super) fn searching(&self) -> bool {
        self.search.as_ref().is_some_and(|s| s.worker.busy)
    }

    pub fn open_result(&mut self, index: usize, tx: &mut Tx) {
        let Some(hit) = self.search.as_ref().and_then(|s| s.hits.get(index)) else {
            return;
        };
        let (path, range) = (hit.path.clone(), hit.range.clone());
        self.search = None;
        self.open_at(path, range, tx);
    }
}

fn input_key(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    if keymap::handle(state, key, tx) {
        return true;
    }
    let Some(search) = &mut state.search else {
        return false;
    };
    match key.key {
        RuneKey::Up | RuneKey::Down => {
            if !search.hits.is_empty() {
                let len = search.hits.len();
                search.selected = if matches!(key.key, RuneKey::Down) {
                    (search.selected + 1) % len
                } else {
                    (search.selected + len - 1) % len
                };
                tx.scroll_into_view(format!("global-hit-{}", search.selected));
            }
        }
        RuneKey::Enter => {
            let selected = search.selected;
            state.open_result(selected, tx);
        }
        _ => return false,
    }
    true
}

pub(super) fn view(state: &State, p: Palette) -> El {
    let Some(search) = &state.search else {
        return col();
    };
    let mut results = col()
        .key("global-results")
        .w_full()
        .max_h(250.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    if search.hits.is_empty() {
        results = results.child(
            text(t(if search.worker.busy {
                Key::Searching
            } else {
                Key::NoNotesFound
            }))
            .padding(12.)
            .color(rgb(p.dim)),
        );
    }
    for (index, hit) in search.hits.iter().enumerate() {
        let location = if hit.line == 0 {
            String::new()
        } else {
            format!("{} · ", hit.line)
        };
        results = results.child(
            col()
                .key(format!("global-hit-{index}"))
                .w_full()
                .gap(3.)
                .padding(9.)
                .role("button")
                .label(format!("{}: {}", hit.title, hit.snippet))
                .focusable(true)
                .fill(rgb(if index == search.selected {
                    p.active
                } else {
                    p.panel
                }))
                .hover(rgb(p.hover))
                .child(
                    text(&hit.title)
                        .font(TextStyle::new(13., 600.))
                        .color(rgb(p.fg)),
                )
                .child(
                    text(format!("{location}{}", hit.snippet))
                        .font(TextStyle::new(12., 400.))
                        .color(rgb(p.dim)),
                )
                .on_press_tx(move |s: &mut State, tx| s.open_result(index, tx)),
        );
    }
    col()
        .key("global-search")
        .w_full()
        .fill(rgb(p.panel))
        .child(
            row()
                .items_center()
                .w_full()
                .padding(7.)
                .gap(8.)
                .child(
                    text_input(&search.query, TextStyle::new(14., 400.))
                        .key("global-query")
                        .single_line()
                        .grow(1.)
                        .min_w(0.)
                        .label(t(Key::Search))
                        .placeholder(t(Key::SearchPlaceholder), rgb(p.faint))
                        .color(rgb(p.fg))
                        .caret(rgb(p.caret), rgba(p.selection))
                        .on_edit(|s: &mut State| &mut s.search.as_mut().unwrap().query)
                        .on_input_change(|s: &mut State, tx| s.start_search(tx))
                        .on_input_key(input_key),
                )
                .child(
                    text("×")
                        .key("global-close")
                        .role("button")
                        .label(t(Key::Close))
                        .focusable(true)
                        .padding(5.)
                        .color(rgb(p.dim))
                        .hover(rgb(p.hover))
                        .on_press_tx(State::close_search),
                ),
        )
        .child(results)
}
