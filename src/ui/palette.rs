use super::*;
use crate::core::fuzzy;
use rune::{Key as RuneKey, KeyEvent, Spring, SpringPair, Transition, text_input};

#[derive(Default)]
pub(super) struct Palette {
    pub open: Option<Open>,
    pub recent: Vec<&'static str>,
}
pub(super) struct Open {
    pub query: TextEdit,
    pub selected: usize,
    return_focus: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Command(&'static str),
    Note(PathBuf),
}
pub(super) struct Entry {
    pub title: String,
    pub detail: String,
    pub target: Target,
    pub enabled: bool,
    score: i64,
    recent: usize,
}
pub(super) fn entries(s: &State) -> Vec<Entry> {
    let Some(open) = &s.palette.open else {
        return vec![];
    };
    let mut out = vec![];
    let query = open.query.text.trim_start();
    if let Some(query) = query.strip_prefix('>') {
        let query = fuzzy::Query::new(query);
        for c in commands::ALL {
            if let Some(score) = query.score(c.title()) {
                out.push(Entry {
                    title: c.title().into(),
                    detail: c.shortcut.into(),
                    target: Target::Command(c.id),
                    enabled: (c.enabled)(s),
                    score,
                    recent: s
                        .palette
                        .recent
                        .iter()
                        .position(|id| *id == c.id)
                        .unwrap_or(usize::MAX),
                });
            }
        }
        out.sort_by(|a, b| {
            a.recent
                .cmp(&b.recent)
                .then(b.score.cmp(&a.score))
                .then(a.title.cmp(&b.title))
        });
    } else {
        fn visit(
            nodes: &[vault::Node],
            root: &std::path::Path,
            query: &fuzzy::Query,
            out: &mut Vec<Entry>,
        ) {
            for node in nodes {
                if node.is_folder() {
                    visit(&node.children, root, query, out);
                } else {
                    let path = node
                        .path
                        .strip_prefix(root)
                        .unwrap_or(&node.path)
                        .to_string_lossy();
                    if let Some(score) = query.score(&path) {
                        out.push(Entry {
                            title: node.name.clone(),
                            detail: path.into_owned(),
                            target: Target::Note(node.path.clone()),
                            enabled: true,
                            score,
                            recent: 0,
                        });
                    }
                }
            }
        }
        visit(&s.tree, &s.root, &fuzzy::Query::new(query), &mut out);
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.title.cmp(&b.title))
                .then(a.detail.cmp(&b.detail))
        });
    }
    out
}

impl State {
    pub fn open_palette(&mut self, commands: bool, tx: &mut Tx) {
        let return_focus = self.palette.open.as_ref().map_or_else(
            || tx.focused().map(str::to_owned),
            |open| open.return_focus.clone(),
        );
        self.file_ops.mode = file_ops::Mode::Idle;
        self.close_spell(tx);
        let mut query = TextEdit::new(if commands { ">" } else { "" });
        query.set_caret(query.text.len());
        query.focused = true;
        self.palette.open = Some(Open {
            query,
            selected: 0,
            return_focus,
        });
        tx.focus("palette-query", false);
    }
    pub fn close_palette(&mut self, tx: &mut Tx) {
        if let Some(open) = self.palette.open.take() {
            tx.focus(open.return_focus.as_deref().unwrap_or("editor"), false);
        }
        tx.instant();
    }
    fn activate_palette(&mut self, target: Target, tx: &mut Tx) {
        if let Target::Command(id) = target {
            if !commands::ALL
                .iter()
                .any(|c| c.id == id && (c.enabled)(self))
            {
                return;
            }
            if matches!(id, "palette" | "notes") {
                commands::run(self, id, tx);
                return;
            }
        }
        self.close_palette(tx);
        match target {
            Target::Command(id) => {
                commands::run(self, id, tx);
            }
            Target::Note(path) => self.open(path, tx),
        }
    }
    pub fn palette_key(&mut self, key: &KeyEvent, tx: &mut Tx) -> bool {
        if self.palette.open.is_none() {
            return false;
        }
        match key.key {
            RuneKey::Escape => self.close_palette(tx),
            RuneKey::Up | RuneKey::Down => {
                let len = entries(self).len();
                let open = self.palette.open.as_mut().unwrap();
                if len > 0 {
                    open.selected = if key.key == RuneKey::Down {
                        (open.selected + 1) % len
                    } else {
                        (open.selected + len - 1) % len
                    };
                }
                tx.scroll_into_view(format!("palette-row-{}", open.selected));
                tx.instant();
            }
            RuneKey::Enter => {
                let selected = self.palette.open.as_ref().unwrap().selected;
                if let Some(entry) = entries(self).into_iter().nth(selected)
                    && entry.enabled
                {
                    self.activate_palette(entry.target, tx);
                }
            }
            _ => {}
        }
        true
    }
}

pub(super) fn overlay(s: &State, cx: &Cx<State>, p: theme::Palette) -> Option<El> {
    let open = s.palette.open.as_ref()?;
    let width = cx.layout_size("app").map_or(900., |size| size[0]);
    let panel_width = (width - 32.).clamp(240., 560.);
    let mut panel = col()
        .key("command-palette")
        .w(panel_width)
        .padding(8.)
        .gap(4.)
        .fill(rgb(p.panel))
        .border(1., rgb(p.faint))
        .clip_rounded(10.)
        .shadow(10., 30., theme::rgba(0x00000030))
        .child(
            text(if open.query.text.trim_start().starts_with('>') {
                t(Key::PaletteCommands)
            } else {
                t(Key::PaletteNotes)
            })
            .font(TextStyle::new(12., 500.))
            .color(rgb(p.dim))
            .padding(6.),
        )
        .child(
            text_input(&open.query, TextStyle::new(14., 400.))
                .key("palette-query")
                .label(t(Key::PaletteHint))
                .placeholder(t(Key::PaletteHint), rgb(p.dim))
                .single_line()
                .w_full()
                .padding(10.)
                .color(rgb(p.fg))
                .fill(rgb(p.bg))
                .caret(rgb(p.caret), theme::rgba(p.selection))
                .on_edit(|s: &mut State| &mut s.palette.open.as_mut().unwrap().query)
                .on_input_change(|s: &mut State, tx| {
                    s.palette.open.as_mut().unwrap().selected = 0;
                    tx.scroll_to_y("palette-results", 0.);
                    tx.instant();
                })
                .on_input_key(|s: &mut State, key, tx| {
                    matches!(
                        key.key,
                        RuneKey::Escape | RuneKey::Up | RuneKey::Down | RuneKey::Enter
                    ) && s.palette_key(key, tx)
                }),
        );
    let list = entries(s);
    if list.is_empty() {
        panel = panel.child(text(t(Key::PaletteEmpty)).padding(12.).color(rgb(p.dim)));
    }
    let selected = open.selected;
    let height = cx.layout_size("app").map_or(760., |size| size[1]);
    let count = list.len();
    let rows = col()
        .key("palette-rows")
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            count,
            36.,
            |_: &State, index| format!("palette-row-{index}").into(),
            move |_: &State, index| {
                let entry = &list[index];
                let target = entry.target.clone();
                row()
                    .key(format!("palette-row-{index}"))
                    .w_full()
                    .h(36.)
                    .padding(8.)
                    .items_center()
                    .gap(12.)
                    .justify_between()
                    .fill(rgb(if index == selected { p.active } else { p.panel }))
                    .hover(rgb(p.hover))
                    .disabled(!entry.enabled)
                    .child(
                        text(entry.title.clone())
                            .truncate()
                            .min_w(0.)
                            .grow(1.)
                            .font(TextStyle::new(13., 400.))
                            .color(rgb(if entry.enabled { p.fg } else { p.dim })),
                    )
                    .child(
                        text(entry.detail.clone())
                            .truncate_middle()
                            .max_w(panel_width * 0.45)
                            .font(TextStyle::new(11., 400.))
                            .color(rgb(p.dim)),
                    )
                    .on_press_tx(move |s: &mut State, tx| s.activate_palette(target.clone(), tx))
            },
        );
    panel = panel.child(
        col()
            .key("palette-results")
            .w_full()
            .max_h((height - 176.).clamp(36., 288.))
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(rows),
    );
    let transition = if cx.system_reduced_motion() {
        Transition::none()
    } else {
        Transition::scale(0.98).springs(SpringPair::new(
            Spring::new(0.18, 0.),
            Spring::new(0.12, 0.),
        ))
    };
    Some(
        panel
            .absolute(((width - panel_width) / 2.).max(8.), 64.)
            .transition(transition)
            .modal_tx(true, State::close_palette),
    )
}
