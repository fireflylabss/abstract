mod attach_ui;
mod export_ui;
mod files_menu;
mod find_ui;
mod links_ui;
mod main_view;
mod notes;
mod rename;
mod save;
mod search_ui;
mod settings_ui;
mod sidebar;
mod spaces_ui;
mod tour_ui;
mod update_ui;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use gpui_kit::component::input::{self, Input, InputEvent, InputState};
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::assets::{SANS, ease_out_quint, icon, icon_btn, rise};
use crate::chrome::{
    chrome_left_pad, drag_fallback, session_window, titlebar_drag, window_controls,
};
use crate::editor::{Attach, Changed, CompletionKey, LiveEditor, OpenLink};
use crate::i18n::{self, Key, t, tf};
use crate::keymap::*;
use crate::spaces::{self, Spaces};
use crate::store::{self, Session, SessionNote, SessionWindow, Settings};
use crate::theme::{self, Palette, PaletteAccess, ThemePref};
use crate::tour;
use crate::vault::{self, NodeKind};
use crate::watch::SpaceWatcher;

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
        .unwrap_or_else(|| SharedString::from(t(Key::Untitled)))
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
/// write. A rename also relinks `[[old stem]]` in the other notes of `space`.
/// Returns whether the visible tree changed (created or renamed).
/// Blocking; runs on the background executor under the app's write lock.
/// Until it lands, the text is kept for the crash report.
fn write_note(
    lock: &Arc<Mutex<()>>,
    file: &Arc<Mutex<NoteFile>>,
    synced: bool,
    text: &str,
    space: Option<&Path>,
) -> std::io::Result<bool> {
    let held = crate::crash::unsaved(text);
    let result = write_note_now(lock, file, synced, text, space);
    if result.is_ok() {
        crate::crash::saved(held);
    }
    result
}

fn write_note_now(
    lock: &Arc<Mutex<()>>,
    file: &Arc<Mutex<NoteFile>>,
    synced: bool,
    text: &str,
    space: Option<&Path>,
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
            let old = stem_of(&from);
            if let Some(space) = space
                && !vault::is_placeholder_stem(&old)
            {
                crate::links::relink(space, &target, &old, Some(&target), true);
            }
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
    status_open: bool,
    settings_open: bool,
    settings_focus: FocusHandle,
    /// Last seen mtime of each image the editor has shown (`None`: missing).
    image_stamps: HashMap<PathBuf, Option<SystemTime>>,
    /// Serializes on-disk ops on the open note (rename + write + trash mark).
    write_lock: Arc<Mutex<()>>,
    _save_task: Option<Task<()>>,
    _io_task: Option<Task<()>>,
    _bounds_task: Option<Task<()>>,
    search: Option<search_ui::SearchPalette>,
    find: Option<find_ui::FindBar>,
    completion: Option<links_ui::Completion>,
    backlinks: Vec<(PathBuf, String)>,
    update: Option<update_ui::UpdateState>,
    install: crate::update::Install,
    _update_task: Option<Task<()>>,
    /// Report left by the previous run's crash, shown until dismissed.
    pub(crate) crash: Option<crate::crash::Pending>,
    /// Note path and names the current `backlinks` were computed for.
    backlinks_key: Option<(PathBuf, Vec<String>)>,
    link_index: Arc<Mutex<crate::links::LinkIndex>>,
    _backlinks_task: Option<Task<()>>,
    _watcher: Option<SpaceWatcher>,
    _watch_task: Option<Task<()>>,
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
        editor.update(cx, |ed, cx| ed.set_raw_tables(settings.raw_tables(), cx));
        let on_change = cx.subscribe(&editor, |this: &mut Self, editor, _: &Changed, cx| {
            let text = editor.read(cx).text();
            this.words = text.split_whitespace().count();
            this.schedule_save(cx);
            this.update_completion(cx);
            this.refresh_find(false, None, cx);
            cx.notify();
        });
        let on_completion = cx.subscribe(&editor, |this: &mut Self, _, ev: &CompletionKey, cx| {
            this.completion_key(ev, cx);
        });
        let on_link = cx.subscribe(&editor, |_this: &mut Self, _, ev: &OpenLink, cx| {
            let target = ev.0.clone();
            cx.spawn(async move |this, cx| {
                this.update_in(cx, |this, window, cx| this.open_link(&target, window, cx))
                    .ok();
            })
            .detach();
        });
        let on_attach = cx.subscribe(&editor, |this: &mut Self, _, ev: &Attach, cx| {
            this.attach(ev.0.clone(), cx);
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
            status_open: false,
            settings_open: false,
            settings_focus: cx.focus_handle(),
            image_stamps: HashMap::new(),
            write_lock: Arc::new(Mutex::new(())),
            _save_task: None,
            _io_task: None,
            _bounds_task: None,
            search: None,
            find: None,
            completion: None,
            backlinks: Vec::new(),
            update: None,
            install: crate::update::Install::detect(),
            _update_task: None,
            crash: None,
            backlinks_key: None,
            link_index: Arc::default(),
            _backlinks_task: None,
            _watcher: None,
            _watch_task: None,
            _subs: vec![
                on_change,
                on_completion,
                on_link,
                on_attach,
                on_quit,
                on_bounds,
                on_activation,
                on_appearance,
            ],
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
        app.check_updates(cx);
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
            .on_action(
                cx.listener(|this, _: &SearchNotes, window, cx| this.open_search(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SearchSelection, window, cx| {
                this.search_selection(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &InsertImage, window, cx| this.insert_image(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FindInNote, window, cx| this.open_find(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ReplaceInNote, window, cx| this.open_find(true, window, cx)),
            )
            .on_action(cx.listener(|this, _: &FindNext, _, cx| this.find_step(true, cx)))
            .on_action(cx.listener(|this, _: &FindPrevious, _, cx| this.find_step(false, cx)))
            .on_action(
                cx.listener(|this, _: &OpenSettings, window, cx| this.toggle_settings(window, cx)),
            )
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .on_action(
                cx.listener(|this, _: &ExportHtml, window, cx| this.export_current(window, cx)),
            )
            .on_action(cx.listener(|this, _: &CopyAsHtml, _, cx| this.copy_as_html(cx)))
            .child(self.render_sidebar(cx))
            .child(self.render_main(window, cx))
            .when(self.settings_open, |el| el.child(self.render_settings(cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::{NoteFile, title_of, write_note};
    use crate::i18n::{self, Key};
    use crate::vault;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[test]
    fn title_of_picks_first_heading_text() {
        assert_eq!(title_of("# Foo").as_ref(), "Foo");
        assert_eq!(title_of("\n\n  ## Bar baz  \nmore").as_ref(), "Bar baz");
        let long = "x".repeat(100);
        assert_eq!(title_of(&long).chars().count(), 80);
        assert_eq!(title_of("").as_ref(), "Sem título");
        assert_eq!(title_of("\n\n").as_ref(), "Sem título");
    }

    fn note(path: PathBuf) -> Arc<Mutex<NoteFile>> {
        Arc::new(Mutex::new(NoteFile {
            path,
            mtime: None,
            deleted: false,
        }))
    }

    #[test]
    fn write_note_syncs_filename_to_title() {
        let dir =
            std::env::temp_dir().join(format!("abstract-app-test-sync-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = note(dir.join("Sem título.md"));
        let lock = Arc::new(Mutex::new(()));
        assert!(write_note(&lock, &file, true, "# Hello", None).unwrap());
        let expected = dir.join(format!("{}.md", vault::stem_for_title("Hello")));
        assert!(expected.exists());
        assert_eq!(file.lock().unwrap().path, expected);
        assert!(file.lock().unwrap().mtime.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_note_synced_rename_relinks_space() {
        let dir =
            std::env::temp_dir().join(format!("abstract-app-test-relink-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Projeto.md");
        std::fs::write(&path, "# Projeto").unwrap();
        std::fs::write(dir.join("other.md"), "see [[Projeto]]").unwrap();
        let file = note(path);
        let lock = Arc::new(Mutex::new(()));
        assert!(write_note(&lock, &file, true, "# Plano", Some(&dir)).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.join("other.md")).unwrap(),
            "see [[Plano]]"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_note_unsynced_overwrites_in_place() {
        let dir =
            std::env::temp_dir().join(format!("abstract-app-test-unsynced-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.md");
        std::fs::write(&path, "old").unwrap();
        let file = note(path.clone());
        let lock = Arc::new(Mutex::new(()));
        assert!(!write_note(&lock, &file, false, "# Hello", None).unwrap());
        assert_eq!(file.lock().unwrap().path, path);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Hello");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_note_deleted_writes_nothing() {
        let dir =
            std::env::temp_dir().join(format!("abstract-app-test-deleted-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = Arc::new(Mutex::new(NoteFile {
            path: dir.join("gone.md"),
            mtime: None,
            deleted: true,
        }));
        let lock = Arc::new(Mutex::new(()));
        assert!(!write_note(&lock, &file, true, "# Hello", None).unwrap());
        assert!(!dir.join("gone.md").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn untitled_filename_follows_language() {
        // No `i18n::set` here — the global language races with parallel tests.
        assert_eq!(i18n::lookup(i18n::Lang::En, Key::Untitled), "Untitled");
        // Stems from any language keep counting as placeholders.
        assert!(vault::is_placeholder_stem("Untitled"));
        assert!(vault::is_placeholder_stem("Untitled 2"));
        assert!(vault::is_placeholder_stem("Sem título"));
        assert!(!vault::is_placeholder_stem("Untitled x"));
        // Tests run with the default language (PtBr) → canonical pt stem.
        let dir = std::env::temp_dir().join(format!(
            "abstract-app-test-untitled-lang-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = note(dir.join("x.md"));
        let lock = Arc::new(Mutex::new(()));
        assert!(write_note(&lock, &file, true, "   \n", None).unwrap());
        assert_eq!(file.lock().unwrap().path, dir.join("Sem título.md"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_note_synced_rename_avoids_collision() {
        let dir = std::env::temp_dir().join(format!(
            "abstract-app-test-collision-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Hello.md"), "other").unwrap();
        let path = dir.join("x.md");
        std::fs::write(&path, "old").unwrap();
        let file = note(path.clone());
        let lock = Arc::new(Mutex::new(()));
        assert!(write_note(&lock, &file, true, "# Hello", None).unwrap());
        let new_path = file.lock().unwrap().path.clone();
        assert_ne!(new_path, dir.join("Hello.md"));
        assert!(new_path.exists());
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
