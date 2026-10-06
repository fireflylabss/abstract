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
        KeyBinding::new("ctrl-=", ZoomIn, c),
        KeyBinding::new("cmd-=", ZoomIn, c),
        KeyBinding::new("ctrl-+", ZoomIn, c),
        KeyBinding::new("cmd-+", ZoomIn, c),
        KeyBinding::new("ctrl-shift-=", ZoomIn, c),
        KeyBinding::new("ctrl-shift-+", ZoomIn, c),
        KeyBinding::new("ctrl--", ZoomOut, c),
        KeyBinding::new("cmd--", ZoomOut, c),
        KeyBinding::new("ctrl-0", ZoomReset, c),
        KeyBinding::new("cmd-0", ZoomReset, c),
        KeyBinding::new("ctrl-w", CloseTab, c),
        KeyBinding::new("cmd-w", CloseTab, c),
        KeyBinding::new("ctrl-tab", NextTab, c),
        KeyBinding::new("ctrl-shift-tab", PrevTab, c),
        KeyBinding::new("ctrl-shift-t", ReopenClosedTab, c),
        KeyBinding::new("cmd-shift-t", ReopenClosedTab, c),
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
