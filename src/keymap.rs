use gpui_kit::*;

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
        DailyNote,
        ToggleOutline,
        ExportHtml,
        CopyAsHtml,
        NextTab,
        PrevTab,
        GoBack,
        GoForward,
        CloseTab,
    ]
);

pub(crate) fn bind_keys(cx: &mut App) {
    let c = Some("AbstractApp");
    cx.bind_keys([
        KeyBinding::new("ctrl-n", NewNote, c),
        KeyBinding::new("cmd-n", NewNote, c),
        KeyBinding::new("ctrl-shift-n", NewFolder, c),
        KeyBinding::new("cmd-shift-n", NewFolder, c),
        KeyBinding::new("ctrl-o", OpenSpace, c),
        KeyBinding::new("cmd-o", OpenSpace, c),
        KeyBinding::new("ctrl-s", SaveNow, c),
        KeyBinding::new("cmd-s", SaveNow, c),
        KeyBinding::new("ctrl-shift-l", CycleTheme, c),
        KeyBinding::new("cmd-shift-l", CycleTheme, c),
        KeyBinding::new("ctrl-\\", ToggleSidebar, c),
        KeyBinding::new("cmd-\\", ToggleSidebar, c),
        KeyBinding::new("ctrl-shift-backspace", DeleteNote, c),
        KeyBinding::new("cmd-shift-backspace", DeleteNote, c),
        KeyBinding::new("ctrl-p", SearchNotes, c),
        KeyBinding::new("cmd-p", SearchNotes, c),
        KeyBinding::new("ctrl-shift-f", SearchNotes, c),
        KeyBinding::new("cmd-shift-f", SearchNotes, c),
        KeyBinding::new("ctrl-f", FindInNote, c),
        KeyBinding::new("cmd-f", FindInNote, c),
        KeyBinding::new("ctrl-h", ReplaceInNote, c),
        KeyBinding::new("cmd-alt-f", ReplaceInNote, c),
        KeyBinding::new("f3", FindNext, c),
        KeyBinding::new("cmd-g", FindNext, c),
        KeyBinding::new("shift-f3", FindPrevious, c),
        KeyBinding::new("cmd-shift-g", FindPrevious, c),
        KeyBinding::new("ctrl-,", OpenSettings, c),
        KeyBinding::new("cmd-,", OpenSettings, c),
        KeyBinding::new("f2", RenameNote, c),
        KeyBinding::new("f1", StartTour, c),
        KeyBinding::new("cmd-q", Quit, c),
        KeyBinding::new("ctrl-shift-d", DailyNote, c),
        KeyBinding::new("cmd-shift-d", DailyNote, c),
        KeyBinding::new("ctrl-shift-o", ToggleOutline, c),
        KeyBinding::new("cmd-shift-o", ToggleOutline, c),
        KeyBinding::new("ctrl-tab", NextTab, c),
        KeyBinding::new("ctrl-shift-tab", PrevTab, c),
        KeyBinding::new("cmd-shift-]", NextTab, c),
        KeyBinding::new("cmd-shift-[", PrevTab, c),
        // On macOS Alt+Left/Right stays word-move in the editor; Cmd+[/]
        // is the browser convention. Elsewhere Alt+Left/Right navigates.
        KeyBinding::new("alt-left", GoBack, c),
        KeyBinding::new("alt-right", GoForward, c),
        KeyBinding::new("cmd-[", GoBack, c),
        KeyBinding::new("cmd-]", GoForward, c),
        KeyBinding::new("ctrl-w", CloseTab, c),
        KeyBinding::new("cmd-w", CloseTab, c),
    ]);
}

/// A `cmd-shift-o`-style binding spec rendered like the i18n strings do:
/// `cmd`/`ctrl` become `MOD`, so it reads `Cmd+Shift+O` here, `Ctrl+Shift+O`
/// elsewhere.
pub(crate) fn binding_display(spec: &str) -> String {
    spec.split('-')
        .map(|part| match part {
            "cmd" | "ctrl" => MOD.to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            key => {
                let mut it = key.chars();
                match it.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + it.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Commands offered by the palette's `>` mode — the `bind_keys` set minus
/// selection/find-context actions, plus the editor's insert actions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AppCommand {
    NewNote,
    NewFolder,
    OpenSpace,
    ToggleSidebar,
    ToggleOutline,
    CycleTheme,
    OpenSettings,
    SaveNow,
    FindInNote,
    ReplaceInNote,
    InsertTable,
    InsertFootnote,
    InsertImage,
    StartTour,
    Quit,
}

impl AppCommand {
    pub(crate) const ALL: &[Self] = &[
        Self::NewNote,
        Self::NewFolder,
        Self::OpenSpace,
        Self::ToggleSidebar,
        Self::ToggleOutline,
        Self::CycleTheme,
        Self::OpenSettings,
        Self::SaveNow,
        Self::FindInNote,
        Self::ReplaceInNote,
        Self::InsertTable,
        Self::InsertFootnote,
        Self::InsertImage,
        Self::StartTour,
        Self::Quit,
    ];

    pub(crate) fn label(self) -> crate::i18n::Key {
        match self {
            Self::NewNote => crate::i18n::Key::CmdNewNote,
            Self::NewFolder => crate::i18n::Key::NewFolder,
            Self::OpenSpace => crate::i18n::Key::CmdOpenSpace,
            Self::ToggleSidebar => crate::i18n::Key::CmdToggleSidebar,
            Self::ToggleOutline => crate::i18n::Key::CmdToggleOutline,
            Self::CycleTheme => crate::i18n::Key::CmdCycleTheme,
            Self::OpenSettings => crate::i18n::Key::CmdOpenSettings,
            Self::SaveNow => crate::i18n::Key::CmdSaveNow,
            Self::FindInNote => crate::i18n::Key::FindInNote,
            Self::ReplaceInNote => crate::i18n::Key::CmdReplace,
            Self::InsertTable => crate::i18n::Key::CmdInsertTable,
            Self::InsertFootnote => crate::i18n::Key::CmdInsertFootnote,
            Self::InsertImage => crate::i18n::Key::CmdInsertImage,
            Self::StartTour => crate::i18n::Key::CmdStartTour,
            Self::Quit => crate::i18n::Key::CmdQuit,
        }
    }

    /// Binding spec from `bind_keys` (macOS spellings; `binding_display`
    /// renders `cmd` as the platform's MOD either way).
    fn binding(self) -> Option<&'static str> {
        match self {
            Self::NewNote => Some("cmd-n"),
            Self::NewFolder => Some("cmd-shift-n"),
            Self::OpenSpace => Some("cmd-o"),
            Self::ToggleSidebar => Some("cmd-\\"),
            Self::ToggleOutline => Some("cmd-shift-o"),
            Self::CycleTheme => Some("cmd-shift-l"),
            Self::OpenSettings => Some("cmd-,"),
            Self::SaveNow => Some("cmd-s"),
            Self::FindInNote => Some("cmd-f"),
            Self::ReplaceInNote => Some(if cfg!(target_os = "macos") {
                "cmd-alt-f"
            } else {
                "ctrl-h"
            }),
            Self::StartTour => Some("f1"),
            Self::Quit => Some("cmd-q"),
            Self::InsertTable | Self::InsertFootnote | Self::InsertImage => None,
        }
    }

    /// Keybinding hint shown right of the row; none for unbound commands.
    pub(crate) fn hint(self) -> Option<String> {
        self.binding().map(binding_display)
    }

    /// Dispatch the action a keystroke would fire: on the always-rendered
    /// editor node, bubbling up to the app-root `.on_action` listeners.
    pub(crate) fn dispatch(self, editor: &FocusHandle, window: &mut Window, cx: &mut App) {
        let action: &dyn Action = match self {
            Self::NewNote => &NewNote,
            Self::NewFolder => &NewFolder,
            Self::OpenSpace => &OpenSpace,
            Self::ToggleSidebar => &ToggleSidebar,
            Self::ToggleOutline => &ToggleOutline,
            Self::CycleTheme => &CycleTheme,
            Self::OpenSettings => &OpenSettings,
            Self::SaveNow => &SaveNow,
            Self::FindInNote => &FindInNote,
            Self::ReplaceInNote => &ReplaceInNote,
            Self::InsertTable => &crate::editor::InsertTable,
            Self::InsertFootnote => &crate::editor::InsertFootnote,
            Self::InsertImage => &InsertImage,
            Self::StartTour => &StartTour,
            Self::Quit => &Quit,
        };
        editor.dispatch_action(action, window, cx);
    }

    /// `>`-mode fuzzy match over both languages, best score first, ties in
    /// `ALL` order. An empty query lists every command.
    pub(crate) fn matching(query: &str) -> Vec<Self> {
        let q = query.trim().to_lowercase();
        let mut scored: Vec<(u32, usize, Self)> = Self::ALL
            .iter()
            .enumerate()
            .filter_map(|(ix, cmd)| {
                let label = cmd.label();
                let score = [crate::i18n::Lang::En, crate::i18n::Lang::PtBr]
                    .iter()
                    .filter_map(|lang| {
                        crate::search::fuzzy_score(crate::i18n::lookup(*lang, label), &q)
                    })
                    .max();
                score.map(|s| (s, ix, *cmd))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, cmd)| cmd).collect()
    }
}
