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
    ]);
}
