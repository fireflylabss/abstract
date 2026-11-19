use super::*;
use crate::core::md::edit::Command as Edit;
use Binding::{AltPrimary as A, Function as F, Primary as P};
use rune::{Key as RuneKey, KeyEvent};

#[derive(Clone, Copy)]
pub enum Binding {
    Primary(char, bool),
    AltPrimary(char),
    Function(u8, bool),
    Focus,
    Follow,
    Tab(bool),
    Page(bool),
}
impl Binding {
    pub fn matches(self, key: &KeyEvent) -> bool {
        match self {
            Self::Primary(c, shift) => key.mods.shift == shift && keymap::shortcut(key, c),
            Self::AltPrimary(c) => {
                !key.mods.shift
                    && key.mods.alt
                    && (key.mods.cmd || key.mods.ctrl)
                    && (key.is_command(c)
                        || matches!(&key.key, RuneKey::Text(s) if s.eq_ignore_ascii_case(&c.to_string())))
            }
            Self::Function(n, shift) => {
                key.mods.shift == shift
                    && !key.mods.cmd
                    && !key.mods.ctrl
                    && !key.mods.alt
                    && key.key == RuneKey::Function(n)
            }
            Self::Focus => {
                key.mods.shift
                    && !key.mods.alt
                    && (key.mods.cmd || key.mods.ctrl)
                    && key.key == RuneKey::Enter
            }
            Self::Follow => {
                key.mods.alt
                    && !key.mods.shift
                    && !key.mods.cmd
                    && !key.mods.ctrl
                    && key.key == RuneKey::Enter
            }
            Self::Tab(shift) => {
                key.mods.ctrl && !key.mods.alt && key.mods.shift == shift && key.key == RuneKey::Tab
            }
            Self::Page(next) => {
                key.mods.ctrl
                    && !key.mods.alt
                    && key.key
                        == if next {
                            RuneKey::PageDown
                        } else {
                            RuneKey::PageUp
                        }
            }
        }
    }
}

pub struct Command {
    pub id: &'static str,
    pub label: Key,
    pub shortcut: &'static str,
    pub bindings: &'static [Binding],
    pub enabled: fn(&State) -> bool,
    pub action: fn(&mut State, &mut Tx),
    pub editor_only: bool,
}
impl Command {
    pub fn title(&self) -> &'static str {
        t(self.label)
            .split(" ({MOD}")
            .next()
            .unwrap_or(t(self.label))
    }
}
fn idle(s: &State) -> bool {
    !s.files_busy() && !s.update_restarting()
}
fn note(s: &State) -> bool {
    idle(s) && s.document().is_some()
}
fn editing(s: &State) -> bool {
    note(s) && !s.settings_open()
}
fn find_matches(s: &State) -> bool {
    note(s) && s.find.as_ref().is_some_and(|bar| !bar.matches.is_empty())
}
fn spelling(s: &State) -> bool {
    editing(s) && s.spelling_available()
}
fn always(_: &State) -> bool {
    true
}
fn zoom(s: &mut State, tx: &mut Tx, delta: f32) {
    s.settings.set_zoom(if delta == 0. {
        1.
    } else {
        (s.settings.zoom() + delta).clamp(0.5, 2.)
    });
    s.save_settings(tx);
}
macro_rules! editor_only {
    (spelling) => {
        true
    };
    (editing) => {
        true
    };
    ($other:ident) => {
        false
    };
}
macro_rules! entry {
    ($id:literal, $label:ident, $shortcut:literal, [$($binding:expr),*], $enabled:ident, $action:expr) => {
        Command { id: $id, label: Key::$label, shortcut: $shortcut, bindings: &[$($binding),*], enabled: $enabled, action: $action, editor_only: editor_only!($enabled) }
    };
}
pub const ALL: &[Command] = &[
    entry!(
        "palette",
        PaletteCommands,
        "⇧⌘/Ctrl+P",
        [P('p', true)],
        idle,
        |s, tx| s.open_palette(true, tx)
    ),
    entry!(
        "notes",
        PaletteNotes,
        "⌘/Ctrl+P",
        [P('p', false)],
        idle,
        |s, tx| s.open_palette(false, tx)
    ),
    entry!(
        "new-note",
        NewNote,
        "⌘/Ctrl+N",
        [P('n', false), P('t', false)],
        idle,
        State::new_note
    ),
    entry!(
        "new-folder",
        NewFolder,
        "⇧⌘/Ctrl+N",
        [P('n', true)],
        idle,
        |s, tx| s.rename_file(s.root.clone(), true, true, tx)
    ),
    entry!(
        "settings",
        Settings,
        "⌘/Ctrl+,",
        [P(',', false)],
        idle,
        State::toggle_settings
    ),
    entry!(
        "search",
        Search,
        "⇧⌘/Ctrl+F",
        [P('f', true)],
        idle,
        State::open_search
    ),
    entry!(
        "find",
        FindInNote,
        "⌘/Ctrl+F",
        [P('f', false)],
        note,
        |s, tx| s.open_find(false, tx)
    ),
    entry!(
        "replace",
        ToggleReplace,
        "⌘/Ctrl+H",
        [P('h', false), A('f')],
        note,
        |s, tx| s.open_find(true, tx)
    ),
    entry!(
        "find-next",
        FindNext,
        "F3",
        [F(3, false), P('g', false)],
        find_matches,
        |s, tx| s.find_step(true, tx)
    ),
    entry!(
        "find-prev",
        FindPrev,
        "⇧F3",
        [F(3, true), P('g', true)],
        find_matches,
        |s, tx| s.find_step(false, tx)
    ),
    entry!(
        "follow",
        FollowLinkCommand,
        "Alt+Enter",
        [Binding::Follow],
        editing,
        State::follow_link
    ),
    entry!(
        "spelling",
        SpellSuggestions,
        "F7",
        [F(7, false)],
        spelling,
        State::spell_suggestions
    ),
    entry!("rename", Rename, "F2", [F(2, false)], note, |s, tx| {
        if let Some(p) = s.active.clone() {
            s.rename_file(p, false, false, tx);
        }
    }),
    entry!("trash", MoveToTrash, "", [], note, |s, tx| {
        if let Some(p) = s.active.clone() {
            s.confirm_trash(p, tx);
        }
    }),
    entry!(
        "close",
        CloseTabCommand,
        "⌘/Ctrl+W",
        [P('w', false)],
        note,
        |s, tx| {
            if let Some(p) = s.active.clone() {
                s.close_tab(&p, tx);
            }
        }
    ),
    entry!(
        "next-tab",
        NextTabCommand,
        "Ctrl+Tab",
        [
            Binding::Tab(false),
            Binding::Page(true),
            P(']', true),
            P('}', true)
        ],
        note,
        |s, tx| s.cycle_tab(true, tx)
    ),
    entry!(
        "prev-tab",
        PreviousTabCommand,
        "Ctrl+⇧Tab",
        [
            Binding::Tab(true),
            Binding::Page(false),
            P('[', true),
            P('{', true)
        ],
        note,
        |s, tx| s.cycle_tab(false, tx)
    ),
    entry!(
        "save",
        SaveCommand,
        "⌘/Ctrl+S",
        [P('s', false)],
        idle,
        State::save_now
    ),
    entry!(
        "quit",
        QuitCommand,
        "⌘/Ctrl+Q",
        [P('q', false)],
        always,
        |_, tx| tx.quit()
    ),
    entry!(
        "zoom-in",
        ZoomInCommand,
        "⌘/Ctrl++",
        [P('=', false), P('+', false), P('=', true), P('+', true)],
        idle,
        |s, tx| zoom(s, tx, 0.1)
    ),
    entry!(
        "zoom-out",
        ZoomOutCommand,
        "⌘/Ctrl+−",
        [P('-', false)],
        idle,
        |s, tx| zoom(s, tx, -0.1)
    ),
    entry!(
        "zoom-reset",
        ZoomResetCommand,
        "⌘/Ctrl+0",
        [P('0', false)],
        idle,
        |s, tx| zoom(s, tx, 0.)
    ),
    entry!(
        "sidebar",
        SidebarCommand,
        "⌘/Ctrl+\\",
        [P('\\', false)],
        idle,
        |s, _| s.sidebar_open = !s.sidebar_open
    ),
    entry!(
        "theme",
        ThemeCommand,
        "⇧⌘/Ctrl+L",
        [P('l', true)],
        idle,
        |s, tx| {
            s.settings.set_theme(s.settings.theme().next());
            s.save_settings(tx);
        }
    ),
    entry!(
        "focus",
        FocusMode,
        "⇧⌘/Ctrl+Enter",
        [Binding::Focus],
        idle,
        |s, tx| {
            s.focus_mode = !s.focus_mode;
            tx.focus("editor", false);
        }
    ),
    entry!(
        "export-html",
        ExportAsHtml,
        "⇧⌘/Ctrl+E",
        [P('e', true)],
        note,
        |s, tx| {
            if let Some(p) = s.active.clone() {
                s.export_note(p, crate::io::export::Format::Html, tx);
            }
        }
    ),
    entry!(
        "export-pdf",
        ExportAsPdf,
        "Alt+⌘/Ctrl+E",
        [A('e')],
        note,
        |s, tx| {
            if let Some(p) = s.active.clone() {
                s.export_note(p, crate::io::export::Format::Pdf, tx);
            }
        }
    ),
    entry!(
        "undo",
        UndoCommand,
        "⌘/Ctrl+Z",
        [P('z', false)],
        editing,
        |s, tx| editor::restore(s, false, tx)
    ),
    entry!(
        "redo",
        RedoCommand,
        "⇧⌘/Ctrl+Z",
        [P('z', true), P('y', false)],
        editing,
        |s, tx| editor::restore(s, true, tx)
    ),
    entry!(
        "bold",
        BoldCommand,
        "⌘/Ctrl+B",
        [P('b', false)],
        editing,
        |s, tx| editor::command(s, Edit::Wrap("**"), tx)
    ),
    entry!(
        "italic",
        ItalicCommand,
        "⌘/Ctrl+I",
        [P('i', false)],
        editing,
        |s, tx| editor::command(s, Edit::Wrap("*"), tx)
    ),
    entry!(
        "strike",
        StrikeCommand,
        "⇧⌘/Ctrl+X",
        [P('x', true)],
        editing,
        |s, tx| editor::command(s, Edit::Wrap("~~"), tx)
    ),
    entry!(
        "code",
        CodeCommand,
        "⌘/Ctrl+E",
        [P('e', false)],
        editing,
        |s, tx| editor::command(s, Edit::Wrap("`"), tx)
    ),
    entry!(
        "highlight",
        HighlightCommand,
        "⇧⌘/Ctrl+H",
        [P('h', true)],
        editing,
        |s, tx| editor::command(s, Edit::Wrap("=="), tx)
    ),
    entry!(
        "comment",
        CommentCommand,
        "⌘/Ctrl+/",
        [P('/', false)],
        editing,
        |s, tx| editor::command(s, Edit::Wrap("%%"), tx)
    ),
    entry!(
        "link",
        LinkCommand,
        "⌘/Ctrl+K",
        [P('k', false)],
        editing,
        |s, tx| editor::command(s, Edit::Link, tx)
    ),
    entry!(
        "duplicate",
        DuplicateCommand,
        "⇧⌘/Ctrl+D",
        [P('d', true)],
        editing,
        |s, tx| editor::command(s, Edit::DuplicateLines, tx)
    ),
    entry!(
        "select-next",
        SelectNextCommand,
        "⌘/Ctrl+D",
        [P('d', false)],
        editing,
        |s, tx| editor::command(s, Edit::SelectNext, tx)
    ),
];

pub fn run(state: &mut State, id: &str, tx: &mut Tx) -> bool {
    let Some(command) = ALL.iter().find(|c| c.id == id) else {
        return false;
    };
    if (command.enabled)(state) {
        if !matches!(command.id, "palette" | "notes") {
            state.palette.recent.retain(|old| *old != command.id);
            state.palette.recent.insert(0, command.id);
            state.palette.recent.truncate(20);
        }
        (command.action)(state, tx);
    }
    true
}
pub fn handle(state: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    if let Some(command) = ALL.iter().find(|c| {
        (!c.editor_only || tx.focused().is_none_or(|id| id == "editor"))
            && c.bindings.iter().any(|b| b.matches(key))
    }) {
        run(state, command.id, tx)
    } else {
        false
    }
}
