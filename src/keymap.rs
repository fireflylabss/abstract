use gpui_kit::*;

use crate::editor;
use crate::i18n::{Key, t};

pub(crate) const MOD: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};

actions!(
    abstract_app,
    [
        NewNote,
        NewFolder,
        DeleteNote,
        RenameNote,
        SaveNow,
        CycleTheme,
        ToggleSidebar,
        OpenSpace,
        ToggleSpaces,
        StartTour,
        SearchNotes,
        SearchSelection,
        InsertImage,
        FindInNote,
        ReplaceInNote,
        FindNext,
        FindPrevious,
        OpenSettings,
        Quit,
        ExportHtml,
        CopyAsHtml,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        CloseTab,
        NextTab,
        PrevTab,
        ReopenClosedTab,
        ExportPdf,
        ToggleFocus,
        CommandPalette,
        QuickOpen,
    ]
);

/// `Cmd/Ctrl+1..9` jumps to a tab; `last` (the `9` key) goes to the final
/// one, however many are open.
#[derive(Clone, PartialEq, gpui_kit::Action)]
#[action(namespace = abstract_app, no_json)]
pub struct GoToTab {
    /// 0-based index into the tab strip.
    pub ix: usize,
    pub last: bool,
}

/// One user-facing command: the palette lists it and `bind_keys` binds its
/// keys, so both always agree.
pub(crate) struct Command {
    /// Interface string; a trailing ` (shortcut)` hint is dropped for the
    /// palette title.
    pub label: Key,
    pub keys: &'static [&'static str],
    pub action: fn() -> Box<dyn Action>,
    bind: fn(&str, Option<&str>) -> KeyBinding,
    /// Handled by the editor, whose own key context already binds `keys`.
    pub editor: bool,
}

impl Command {
    pub(crate) fn title(&self) -> &'static str {
        title_of(t(self.label))
    }

    /// The first binding that applies on this platform, as `Cmd+Shift+P`.
    pub(crate) fn shortcut(&self) -> String {
        let mac = cfg!(target_os = "macos");
        let uses_cmd = |k: &&&str| k.starts_with("cmd-");
        let key = if mac {
            self.keys.iter().find(uses_cmd).or(self.keys.first())
        } else {
            self.keys.iter().find(|k| !uses_cmd(k))
        };
        key.map(|k| display_keys(k)).unwrap_or_default()
    }
}

fn title_of(label: &'static str) -> &'static str {
    let label = label.split(" (").next().unwrap_or(label);
    label.trim_end_matches('…')
}

fn display_keys(keys: &str) -> String {
    let mut rest = keys;
    let mut out = String::new();
    for (prefix, name) in [
        ("ctrl-", "Ctrl"),
        ("cmd-", "Cmd"),
        ("alt-", "Alt"),
        ("shift-", "Shift"),
    ] {
        if let Some(r) = rest.strip_prefix(prefix) {
            rest = r;
            out.push_str(name);
            out.push('+');
        }
    }
    let mut chars = rest.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
    out
}

macro_rules! command {
    ($label:ident, $action:expr, [$($key:literal),*]) => {
        command!($label, $action, [$($key),*], false)
    };
    ($label:ident, $action:expr, [$($key:literal),*], $editor:literal) => {
        Command {
            label: Key::$label,
            keys: &[$($key),*],
            action: || Box::new($action),
            bind: |keys, context| KeyBinding::new(keys, $action, context),
            editor: $editor,
        }
    };
}

pub(crate) const COMMANDS: &[Command] = &[
    command!(CmdPalette, CommandPalette, ["ctrl-shift-p", "cmd-shift-p"]),
    command!(CmdGoToNote, QuickOpen, ["ctrl-p", "cmd-p"]),
    command!(Search, SearchNotes, ["ctrl-shift-f", "cmd-shift-f"]),
    command!(NewNote, NewNote, ["ctrl-n", "cmd-n"]),
    command!(NewFolder, NewFolder, ["ctrl-shift-n", "cmd-shift-n"]),
    command!(SwitchSpace, OpenSpace, ["ctrl-o", "cmd-o"]),
    command!(CmdSave, SaveNow, ["ctrl-s", "cmd-s"]),
    command!(Rename, RenameNote, ["f2"]),
    command!(
        DeleteNote,
        DeleteNote,
        ["ctrl-shift-backspace", "cmd-shift-backspace"]
    ),
    command!(FindInNote, FindInNote, ["ctrl-f", "cmd-f"]),
    command!(ToggleReplace, ReplaceInNote, ["ctrl-h", "cmd-alt-f"]),
    command!(FindNext, FindNext, ["f3", "cmd-g"]),
    command!(FindPrev, FindPrevious, ["shift-f3", "cmd-shift-g"]),
    command!(Settings, OpenSettings, ["ctrl-,", "cmd-,"]),
    command!(CmdTheme, CycleTheme, ["ctrl-shift-l", "cmd-shift-l"]),
    command!(CmdSidebar, ToggleSidebar, ["ctrl-\\", "cmd-\\"]),
    command!(
        FocusMode,
        ToggleFocus,
        ["ctrl-shift-enter", "cmd-shift-enter"]
    ),
    command!(CloseTab, CloseTab, ["ctrl-w", "cmd-w"]),
    command!(CmdNextTab, NextTab, ["ctrl-tab"]),
    command!(CmdPrevTab, PrevTab, ["ctrl-shift-tab"]),
    command!(
        CmdReopenTab,
        ReopenClosedTab,
        ["ctrl-shift-t", "cmd-shift-t"]
    ),
    command!(
        CmdZoomIn,
        ZoomIn,
        [
            "ctrl-=",
            "cmd-=",
            "ctrl-+",
            "cmd-+",
            "ctrl-shift-=",
            "ctrl-shift-+"
        ]
    ),
    command!(CmdZoomOut, ZoomOut, ["ctrl--", "cmd--"]),
    command!(CmdZoomReset, ZoomReset, ["ctrl-0", "cmd-0"]),
    command!(ExportAsHtml, ExportHtml, []),
    command!(ExportAsPdf, ExportPdf, []),
    command!(CopyAsHtml, CopyAsHtml, []),
    command!(CmdTour, StartTour, ["f1"]),
    command!(Bold, editor::Bold, ["ctrl-b", "cmd-b"], true),
    command!(Italic, editor::Italic, ["ctrl-i", "cmd-i"], true),
    command!(
        Strikethrough,
        editor::Strike,
        ["ctrl-shift-x", "cmd-shift-x"],
        true
    ),
    command!(InlineCode, editor::InlineCode, ["ctrl-e", "cmd-e"], true),
    command!(
        Highlight,
        editor::Highlight,
        ["ctrl-shift-h", "cmd-shift-h"],
        true
    ),
];

pub(crate) fn bind_keys(cx: &mut App) {
    let c = Some("AbstractApp");
    cx.bind_keys(
        COMMANDS
            .iter()
            .filter(|command| !command.editor)
            .flat_map(|command| command.keys.iter().map(|keys| (command.bind)(keys, c))),
    );
    // The palette's text input binds Cmd+Shift+F to its own Replace, which
    // would swallow the switch to full-text search from another mode.
    let palette_input = Some("SearchPalette > Input");
    cx.bind_keys([
        KeyBinding::new("cmd-shift-f", SearchNotes, palette_input),
        KeyBinding::new("ctrl-shift-f", SearchNotes, palette_input),
    ]);
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, c),
        KeyBinding::new("ctrl-1", GoToTab { ix: 0, last: false }, c),
        KeyBinding::new("cmd-1", GoToTab { ix: 0, last: false }, c),
        KeyBinding::new("ctrl-2", GoToTab { ix: 1, last: false }, c),
        KeyBinding::new("cmd-2", GoToTab { ix: 1, last: false }, c),
        KeyBinding::new("ctrl-3", GoToTab { ix: 2, last: false }, c),
        KeyBinding::new("cmd-3", GoToTab { ix: 2, last: false }, c),
        KeyBinding::new("ctrl-4", GoToTab { ix: 3, last: false }, c),
        KeyBinding::new("cmd-4", GoToTab { ix: 3, last: false }, c),
        KeyBinding::new("ctrl-5", GoToTab { ix: 4, last: false }, c),
        KeyBinding::new("cmd-5", GoToTab { ix: 4, last: false }, c),
        KeyBinding::new("ctrl-6", GoToTab { ix: 5, last: false }, c),
        KeyBinding::new("cmd-6", GoToTab { ix: 5, last: false }, c),
        KeyBinding::new("ctrl-7", GoToTab { ix: 6, last: false }, c),
        KeyBinding::new("cmd-7", GoToTab { ix: 6, last: false }, c),
        KeyBinding::new("ctrl-8", GoToTab { ix: 7, last: false }, c),
        KeyBinding::new("cmd-8", GoToTab { ix: 7, last: false }, c),
        KeyBinding::new("ctrl-9", GoToTab { ix: 0, last: true }, c),
        KeyBinding::new("cmd-9", GoToTab { ix: 0, last: true }, c),
    ]);
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, display_keys, title_of};
    use crate::i18n::{self, Lang};
    use std::collections::HashSet;

    #[test]
    fn no_two_commands_share_a_key() {
        let mut seen = HashSet::new();
        for command in COMMANDS {
            for keys in command.keys {
                assert!(seen.insert(*keys), "{keys} is bound twice");
            }
        }
    }

    #[test]
    fn titles_drop_the_shortcut_hint() {
        for command in COMMANDS {
            for lang in Lang::ALL {
                let title = title_of(i18n::lookup(lang, command.label));
                assert!(!title.is_empty());
                assert!(!title.contains(['(', '{', '…']), "{title}");
            }
        }
    }

    #[test]
    fn keys_are_displayed_readably() {
        assert_eq!(display_keys("cmd-shift-p"), "Cmd+Shift+P");
        assert_eq!(display_keys("ctrl--"), "Ctrl+-");
        assert_eq!(display_keys("shift-f3"), "Shift+F3");
        assert_eq!(display_keys("ctrl-shift-backspace"), "Ctrl+Shift+Backspace");
    }
}
