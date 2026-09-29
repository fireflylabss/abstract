use super::*;
use crate::find;

/// Find/replace bar over the open note.
pub(crate) struct FindBar {
    query: Entity<InputState>,
    with: Entity<InputState>,
    replacing: bool,
    case: bool,
    matches: Vec<Range<usize>>,
    current: Option<usize>,
    _subs: [Subscription; 2],
}

impl AbstractApp {
    /// Opens (or refocuses) the bar, seeded with a one-line selection.
    pub(crate) fn open_find(
        &mut self,
        replacing: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current.is_none() {
            return;
        }
        let seed = {
            let s = self.editor.read(cx).selected_text();
            (!s.is_empty() && !s.contains('\n')).then(|| s.to_string())
        };
        if self.find.is_none() {
            let query =
                cx.new(|cx| InputState::new(window, cx).placeholder(t(Key::FindPlaceholder)));
            let with =
                cx.new(|cx| InputState::new(window, cx).placeholder(t(Key::ReplacePlaceholder)));
            let on_query =
                cx.subscribe(&query, |this: &mut Self, _, ev: &InputEvent, cx| match ev {
                    InputEvent::Change => this.refresh_find(true, None, cx),
                    InputEvent::PressEnter { shift, .. } => this.find_step(!*shift, cx),
                    _ => {}
                });
            let on_with = cx.subscribe(&with, |this: &mut Self, _, ev: &InputEvent, cx| {
                if let InputEvent::PressEnter { secondary, .. } = ev {
                    if *secondary {
                        this.replace_all(cx);
                    } else {
                        this.replace_one(cx);
                    }
                }
            });
            self.find = Some(FindBar {
                query,
                with,
                replacing: false,
                case: false,
                matches: Vec::new(),
                current: None,
                _subs: [on_query, on_with],
            });
        }
        let Some(bar) = &mut self.find else { return };
        bar.replacing |= replacing;
        let query = bar.query.clone();
        query.update(cx, |s, cx| {
            if let Some(seed) = &seed {
                s.set_value(seed.clone(), window, cx);
            }
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.refresh_find(seed.is_some(), None, cx);
        cx.notify();
    }

    pub(crate) fn close_find(&mut self, cx: &mut Context<Self>) {
        if self.find.take().is_some() {
            self.editor
                .update(cx, |ed, cx| ed.set_finds(Vec::new(), None, cx));
            self.focus_editor(cx);
            cx.notify();
        }
    }

    /// Recomputes matches against the editor text; the active match is the
    /// first at or after `anchor` (default: the selection start). `select`
    /// moves the editor selection onto it.
    pub(crate) fn refresh_find(
        &mut self,
        select: bool,
        anchor: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(bar) = &mut self.find else { return };
        let query = bar.query.read(cx).value().to_string();
        let ed = self.editor.read(cx);
        bar.matches = find::matches(ed.text(), &query, bar.case);
        bar.current = find::nearest(&bar.matches, anchor.unwrap_or(ed.selection().start));
        let (matches, current) = (bar.matches.clone(), bar.current);
        self.editor.update(cx, |ed, cx| {
            if select && let Some(i) = current {
                ed.select_range(matches[i].clone(), cx);
            }
            ed.set_finds(matches, current, cx);
        });
        cx.notify();
    }

    pub(crate) fn find_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(bar) = &mut self.find else { return };
        let n = bar.matches.len();
        if n == 0 {
            return;
        }
        let i = match bar.current {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None => 0,
        };
        bar.current = Some(i);
        let (matches, r) = (bar.matches.clone(), bar.matches[i].clone());
        self.editor.update(cx, |ed, cx| {
            ed.select_range(r, cx);
            ed.set_finds(matches, Some(i), cx);
        });
        cx.notify();
    }

    fn replace_one(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let Some(r) = bar.current.and_then(|i| bar.matches.get(i)).cloned() else {
            return;
        };
        let with = bar.with.read(cx).value().to_string();
        let next = r.start + with.len();
        self.editor
            .update(cx, |ed, cx| ed.replace_range(r, &with, cx));
        self.refresh_find(true, Some(next), cx);
    }

    fn replace_all(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let Some(first) = bar.matches.first().map(|m| m.start) else {
            return;
        };
        let n = bar.matches.len();
        let with = bar.with.read(cx).value().to_string();
        let text = find::replace_all(self.editor.read(cx).text(), &bar.matches, &with);
        self.editor
            .update(cx, |ed, cx| ed.replace_all(text, first, cx));
        self.refresh_find(false, None, cx);
        self.notice = Some(tf(Key::Replaced, &[("n", &n.to_string())]).into());
        cx.notify();
    }

    fn toggle_case(&mut self, cx: &mut Context<Self>) {
        if let Some(bar) = &mut self.find {
            bar.case = !bar.case;
        }
        self.refresh_find(true, None, cx);
    }

    fn toggle_replace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bar) = &mut self.find else { return };
        bar.replacing = !bar.replacing;
        let target = if bar.replacing { &bar.with } else { &bar.query };
        target.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn render_find(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let bar = self.find.as_ref()?;
        let pal = cx.palette();
        let count: SharedString = if bar.query.read(cx).value().is_empty() {
            "".into()
        } else if let Some(i) = bar.current {
            tf(
                Key::FindCount,
                &[
                    ("i", &(i + 1).to_string()),
                    ("n", &bar.matches.len().to_string()),
                ],
            )
            .into()
        } else {
            t(Key::FindNone).into()
        };
        let small = |id: &'static str, path: &'static str, key: Key, on: bool| {
            icon_btn(id, path, tf(key, &[]).into(), on, &pal).size(px(24.))
        };
        let field = |state: &Entity<InputState>| {
            div().flex_1().min_w_0().child(
                Input::new(state)
                    .appearance(false)
                    .bordered(false)
                    .h(px(26.))
                    .w_full()
                    .text_size(px(13.))
                    .text_color(rgb(pal.fg)),
            )
        };
        let top = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(
                small(
                    "find-toggle-replace",
                    if bar.replacing {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    },
                    Key::ToggleReplace,
                    false,
                )
                .aria_expanded(bar.replacing)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_replace(window, cx))),
            )
            .child(field(&bar.query))
            .child(
                div()
                    .flex_none()
                    .px(px(6.))
                    .text_size(px(11.))
                    .text_color(rgb(pal.faint))
                    .child(count),
            )
            .child(
                small(
                    "find-case",
                    "icons/case-sensitive.svg",
                    Key::MatchCase,
                    bar.case,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_case(cx))),
            )
            .child(
                small("find-prev", "icons/arrow-up.svg", Key::FindPrev, false)
                    .on_click(cx.listener(|this, _, _, cx| this.find_step(false, cx))),
            )
            .child(
                small("find-next", "icons/arrow-down.svg", Key::FindNext, false)
                    .on_click(cx.listener(|this, _, _, cx| this.find_step(true, cx))),
            )
            .child(
                small("find-close", "icons/close.svg", Key::CloseFind, false)
                    .on_click(cx.listener(|this, _, _, cx| this.close_find(cx))),
            );
        let bottom = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .pl(px(26.))
            .child(field(&bar.with))
            .child(
                small("replace-one", "icons/replace.svg", Key::ReplaceOne, false)
                    .on_click(cx.listener(|this, _, _, cx| this.replace_one(cx))),
            )
            .child(
                small(
                    "replace-all",
                    "icons/replace-all.svg",
                    Key::ReplaceAll,
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.replace_all(cx))),
            );
        Some(
            div()
                .id("find-bar")
                .key_context("FindBar")
                .role(Role::Search)
                .aria_label(t(Key::FindInNote))
                .absolute()
                .top(px(8.))
                .right(px(16.))
                .w(px(400.))
                .p(px(4.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .bg(rgb(pal.menu_bg))
                .border_1()
                .border_color(rgb(pal.menu_border))
                .rounded(px(8.))
                .shadow_lg()
                .occlude()
                .on_action(cx.listener(|this, _: &input::Escape, _, cx| this.close_find(cx)))
                .child(top)
                .when(bar.replacing, |el| el.child(bottom))
                .into_any_element(),
        )
    }
}
