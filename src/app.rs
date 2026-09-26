mod main_view;
mod notes;
mod rename;
mod save;
mod sidebar;
mod spaces_ui;
mod tour_ui;

use std::collections::HashSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use gpui_kit::component::input::{self, Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::assets::{SANS, ease_out_quint, icon, icon_btn, rise};
use crate::chrome::{session_window, titlebar_drag, window_controls};
use crate::editor::{Changed, LiveEditor};
use crate::keymap::*;
use crate::spaces::{self, Spaces};
use crate::store::{self, Session, SessionNote, SessionWindow, Settings};
use crate::theme::{self, Palette, PaletteAccess, ThemePref};
use crate::tour;
use crate::vault::{self, NodeKind};

/// Locks `m`, recovering the guard if a previous holder panicked.
pub(crate) fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

const SIDEBAR_W: f32 = 248.;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

/// Marks the synthetic sidebar row that hosts the new-folder input. Starts
/// with a dot so a real file can never collide.
const NEW_FOLDER_ROW: &str = ".abstract-new-folder";
// ── Notes on disk ─────────────────────────────────────────────────────────

fn title_of(text: &str) -> SharedString {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .map(|l| SharedString::from(l.chars().take(80).collect::<String>()))
        .unwrap_or_else(|| SharedString::new_static("Sem título"))
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// What the open note is on disk; shared with in-flight writes, which may
/// rename it.
pub(crate) struct NoteFile {
    path: PathBuf,
    /// Mtime after the last read/write we did; detects external edits.
    mtime: Option<SystemTime>,
    /// Trashed: in-flight writes must not recreate the file.
    deleted: bool,
}

struct CurrentNote {
    file: Arc<Mutex<NoteFile>>,
    /// Renames to the title's stem on each save (placeholder or title-derived
    /// name); a manual rename to anything else turns this off.
    synced: bool,
}

impl CurrentNote {
    pub(crate) fn path(&self) -> PathBuf {
        guard(&self.file).path.clone()
    }
}

/// Inline rename / new-folder input in a sidebar row.
pub(crate) struct RenameEdit {
    state: Entity<InputState>,
    /// Path being renamed, or the parent dir when `create`.
    target: PathBuf,
    kind: NodeKind,
    create: bool,
    _sub: Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveState {
    Saved,
    Pending,
    Failed,
}
/// One serialized on-disk update: rename to the title stem when synced, then
/// write. Returns whether the visible tree changed (created or renamed).
/// Blocking; runs on the background executor under the app's write lock.
fn write_note(
    lock: &Arc<Mutex<()>>,
    file: &Arc<Mutex<NoteFile>>,
    synced: bool,
    text: &str,
) -> std::io::Result<bool> {
    let _write = guard(lock);
    let mut f = guard(file);
    if f.deleted {
        return Ok(false);
    }
    let from = f.path.clone();
    let mut target = from.clone();
    if synced {
        let dir = from
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        target = vault::unique_path(&dir, &vault::stem_for_title(&title_of(text)), Some(&from));
    }
    let existed = from.exists();
    if target != from {
        if !existed || std::fs::rename(&from, &target).is_ok() {
            f.path = target.clone();
        } else {
            // Rename failed: still save under the old name.
            target = from.clone();
        }
    }
    store::write_atomic(&target, text.as_bytes())?;
    f.mtime = std::fs::metadata(&target).and_then(|m| m.modified()).ok();
    Ok(target != from || !existed)
}
pub(crate) struct AbstractApp {
    spaces: Spaces,
    spaces_open: bool,
    dir: PathBuf,
    tree: Vec<vault::Node>,
    expanded: HashSet<PathBuf>,
    current: Option<CurrentNote>,
    /// Planned path of a note not yet written (first keystroke creates it).
    pending_new: Option<PathBuf>,
    /// Last clicked folder; target dir for new notes and folders.
    target_folder: Option<PathBuf>,
    editing: Option<RenameEdit>,
    /// Transient status message, cleared on the next successful save/action.
    notice: Option<SharedString>,
    theme_pref: ThemePref,
    settings: Settings,
    session: Session,
    session_notes: Vec<SessionNote>,
    window_state: Option<SessionWindow>,
    tour_step: Option<usize>,
    tour_focus: FocusHandle,
    tour_shown: bool,
    loading: bool,
    editor: Entity<LiveEditor>,
    sidebar_open: bool,
    /// Bumped per toggle; keys the sidebar slide so it replays.
    sidebar_gen: usize,
    /// Bumped per opened note; keys the editor fade-in.
    open_gen: usize,
    save: SaveState,
    words: usize,
    /// Serializes on-disk ops on the open note (rename + write + trash mark).
    write_lock: Arc<Mutex<()>>,
    _save_task: Option<Task<()>>,
    _io_task: Option<Task<()>>,
    _bounds_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}
impl AbstractApp {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        settings: Settings,
        session: Session,
    ) -> Self {
        let editor = cx.new(LiveEditor::new);
        let on_change = cx.subscribe(&editor, |this: &mut Self, editor, _: &Changed, cx| {
            let text = editor.read(cx).text();
            this.words = text.split_whitespace().count();
            this.schedule_save(cx);
            cx.notify();
        });
        let on_quit = cx.on_app_quit(|this, cx| {
            this.flush_blocking(cx);
            async {}
        });
        let on_bounds = cx.observe_window_bounds(window, |this, window, cx| {
            this.bounds_changed(window, cx);
        });
        let on_activation = cx.observe_window_activation(window, |this, window, cx| {
            this.activation_changed(window, cx);
        });
        let on_appearance = cx.observe_window_appearance(window, |this, window, cx| {
            this.appearance_changed(window, cx);
        });

        let mut app = Self {
            spaces: Spaces {
                paths: vec![PathBuf::new()],
                active: 0,
            },
            spaces_open: false,
            dir: PathBuf::new(),
            tree: Vec::new(),
            expanded: HashSet::new(),
            current: None,
            pending_new: None,
            target_folder: None,
            editing: None,
            notice: None,
            theme_pref: settings.theme(),
            settings,
            session_notes: session.notes(),
            session,
            window_state: None,
            tour_step: None,
            tour_focus: cx.focus_handle(),
            tour_shown: false,
            loading: true,
            editor,
            sidebar_open: true,
            sidebar_gen: 0,
            open_gen: 0,
            save: SaveState::Saved,
            words: 0,
            write_lock: Arc::new(Mutex::new(())),
            _save_task: None,
            _io_task: None,
            _bounds_task: None,
            _subs: vec![on_change, on_quit, on_bounds, on_activation, on_appearance],
        };
        app.sidebar_open = app.session.sidebar_open().unwrap_or(true);
        app._io_task = Some(cx.spawn_in(window, async move |this, cx| {
            let spaces = cx
                .background_executor()
                .spawn(async { spaces::load() })
                .await;
            this.update_in(cx, |this, window, cx| this.enter_space(spaces, window, cx))
                .ok();
        }));
        app
    }
}

impl Render for AbstractApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = cx.palette();
        div()
            .id("abstract-root")
            .key_context("AbstractApp")
            .size_full()
            .flex()
            .font_family(SANS)
            .bg(rgb(pal.bg))
            .text_color(rgb(pal.body))
            .border_1()
            .border_color(rgb(pal.frame_border))
            .on_action(cx.listener(|this, _: &NewNote, window, cx| this.new_note(window, cx)))
            .on_action(cx.listener(|this, _: &NewFolder, window, cx| this.new_folder(window, cx)))
            .on_action(cx.listener(|this, _: &DeleteNote, window, cx| this.delete_note(window, cx)))
            .on_action(
                cx.listener(|this, _: &RenameNote, window, cx| this.rename_current(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SaveNow, _, cx| this.save_now(cx)))
            .on_action(cx.listener(|this, _: &CycleTheme, window, cx| this.cycle_theme(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
            .on_action(cx.listener(|this, _: &OpenSpace, window, cx| this.open_space(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleSpaces, _, cx| this.toggle_spaces(cx)))
            .on_action(cx.listener(|this, _: &StartTour, window, cx| this.start_tour(window, cx)))
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .child(self.render_sidebar(cx))
            .child(self.render_main(window, cx))
    }
}
