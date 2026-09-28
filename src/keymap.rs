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
        KeyBinding::new("f2", RenameNote, c),
        KeyBinding::new("f1", StartTour, c),
        KeyBinding::new("cmd-q", Quit, c),
    ]);
}
