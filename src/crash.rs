//! Local panic reports. Release builds abort on panic, so the hook is the only
//! point where anything can be written: the report, plus the text of a save
//! that had not landed, go to `$XDG_STATE_HOME/abstract/crashes`, and the next
//! launch offers to copy the report or file an issue. Nothing is uploaded.
//! Native faults (e.g. a segfault in a GPU driver) bypass the hook.

use std::fmt::Write as _;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const ISSUES: &str = "https://github.com/fireflylabss/abstract/issues/new";
/// Reports kept on disk; older ones are pruned on launch.
const KEEP: usize = 10;
/// Report bytes put into the issue URL (browsers cap URL length).
const URL_BODY_MAX: usize = 6000;

static LAST_ACTION: Mutex<&'static str> = Mutex::new("");
static UNSAVED: Mutex<Unsaved> = Mutex::new(Unsaved { id: 0, text: None });

/// Text of the newest save that has not landed yet.
struct Unsaved {
    id: u64,
    text: Option<String>,
}

impl Unsaved {
    fn hold(&mut self, text: &str) -> u64 {
        self.id += 1;
        self.text = Some(text.to_owned());
        self.id
    }

    fn release(&mut self, id: u64) {
        if self.id == id {
            self.text = None;
        }
    }
}

pub(crate) fn dir() -> PathBuf {
    crate::store::xdg("XDG_STATE_HOME", ".local/state")
        .join("abstract")
        .join("crashes")
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Record the name of the last dispatched action for the next report.
pub(crate) fn action(name: &'static str) {
    *lock(&LAST_ACTION) = name;
}

/// Keep `text` until [`saved`] is called with the returned id, so a crash
/// before (or after a failed) write still leaves the text on disk.
pub(crate) fn unsaved(text: &str) -> u64 {
    lock(&UNSAVED).hold(text)
}

/// The write for `id` landed; newer pending text is kept.
pub(crate) fn saved(id: u64) {
    lock(&UNSAVED).release(id);
}

pub(crate) fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        write_crash(&dir(), stamp, info);
        default(info);
    }));
}

fn write_crash(dir: &Path, stamp: u64, info: &PanicHookInfo<'_>) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    // `try_lock`: the panicking thread may hold either slot.
    let recovered = UNSAVED
        .try_lock()
        .ok()
        .and_then(|u| u.text.clone())
        .and_then(|text| {
            let path = dir.join(format!("recovered-{stamp}.md"));
            std::fs::write(&path, text).ok().map(|()| path)
        });
    let last = LAST_ACTION.try_lock().map_or("", |a| *a);
    let message = info
        .payload_as_str()
        .unwrap_or("(non-string panic payload)");
    let location = info.location().map_or_else(
        || "unknown".into(),
        |l| format!("{}:{}", l.file(), l.line()),
    );
    let backtrace = tidy_backtrace(&std::backtrace::Backtrace::force_capture().to_string());
    let report = report(
        stamp,
        std::thread::current().name().unwrap_or("unnamed"),
        last,
        &location,
        message,
        recovered.as_deref(),
        &backtrace,
    );
    let _ = std::fs::write(dir.join(format!("crash-{stamp}.txt")), report);
}

fn report(
    stamp: u64,
    thread: &str,
    last: &str,
    location: &str,
    message: &str,
    recovered: Option<&Path>,
    backtrace: &str,
) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "abstract {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(s, "time: {stamp}");
    let _ = writeln!(s, "thread: {thread}");
    let _ = writeln!(
        s,
        "last action: {}",
        if last.is_empty() { "-" } else { last }
    );
    if let Some(p) = recovered {
        let _ = writeln!(s, "recovered text: {}", p.display());
    }
    let _ = writeln!(
        s,
        "\npanicked at {location}:\n{message}\n\nbacktrace:\n{backtrace}"
    );
    redact_home(&s)
}

/// Stripped release builds resolve every frame to the same image symbol;
/// such a trace carries nothing, so say so instead of listing it.
fn tidy_backtrace(bt: &str) -> String {
    let mut names = bt.lines().filter_map(|l| {
        let (n, name) = l.trim_start().split_once(": ")?;
        n.parse::<usize>().ok().map(|_| name.trim())
    });
    let first = names.next();
    if first.is_some_and(|f| names.all(|n| n == f)) {
        "unavailable (symbols are stripped from release builds)".into()
    } else {
        bt.to_owned()
    }
}

/// Replace the user's home directory with `~`, so reports carry no user name.
fn redact_home(s: &str) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    match home.map(|h| h.to_string_lossy().into_owned()) {
        Some(h) if h.len() > 1 => s.replace(h.trim_end_matches(['/', '\\']), "~"),
        _ => s.to_owned(),
    }
}

/// A report from a previous run that the user has not seen yet.
#[derive(Clone, Debug)]
pub(crate) struct Pending {
    pub path: PathBuf,
    pub text: String,
    pub recovered: bool,
}

/// Newest unseen report in `dir`; every unseen report is marked seen so a
/// crash while showing it cannot loop. Prunes to the newest [`KEEP`].
pub(crate) fn take_pending(dir: &Path) -> Option<Pending> {
    let mut reports: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stamp = name
                .strip_prefix("crash-")?
                .strip_suffix(".txt")?
                .trim_end_matches(".seen")
                .parse()
                .ok()?;
            Some((stamp, e.path()))
        })
        .collect();
    reports.sort();
    let excess = reports.len().saturating_sub(KEEP);
    for (stamp, path) in reports.drain(..excess) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(dir.join(format!("recovered-{stamp}.md")));
    }
    let mut newest = None;
    for (stamp, path) in reports {
        if path.to_string_lossy().ends_with(".seen.txt") {
            continue;
        }
        let seen = dir.join(format!("crash-{stamp}.seen.txt"));
        if std::fs::rename(&path, &seen).is_ok() {
            newest = Some((stamp, seen));
        }
    }
    let (stamp, path) = newest?;
    let text = std::fs::read_to_string(&path).ok()?;
    let recovered = dir.join(format!("recovered-{stamp}.md")).exists();
    Some(Pending {
        path,
        text,
        recovered,
    })
}

/// "New issue" URL prefilled with `report` (truncated to fit a URL).
pub(crate) fn issue_url(report: &str) -> String {
    let mut end = report.len().min(URL_BODY_MAX);
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    let first = report
        .lines()
        .find(|l| l.starts_with("panicked at"))
        .unwrap_or("panic");
    let title = format!("Crash: {}", first.trim());
    let body = format!("```\n{}\n```\n", &report[..end]);
    format!(
        "{ISSUES}?title={}&body={}",
        percent_encode(&title),
        percent_encode(&body)
    )
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("abstract-crash-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn report_lists_context_and_redacts_home() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/u".into());
        let r = report(
            7,
            "main",
            "editor::Undo",
            &format!("{home}/src/x.rs:3"),
            "boom",
            None,
            "0: frame",
        );
        assert!(r.starts_with(&format!("abstract {}", env!("CARGO_PKG_VERSION"))));
        assert!(r.contains("last action: editor::Undo"));
        assert!(r.contains("panicked at ~/src/x.rs:3:\nboom"));
        assert!(!r.contains(&home));
    }

    #[test]
    fn stripped_backtrace_is_summarized() {
        let stripped = "   0: __mh_execute_header\n   1: __mh_execute_header\n";
        assert!(tidy_backtrace(stripped).starts_with("unavailable"));
        let real =
            "   0: abstract::main\n             at ./src/main.rs:3:5\n   1: std::rt::lang_start\n";
        assert_eq!(tidy_backtrace(real), real);
    }

    #[test]
    fn pending_is_shown_once_and_pruned() {
        let dir = space("pending");
        assert!(take_pending(&dir).is_none());
        for stamp in 1..=12 {
            std::fs::write(dir.join(format!("crash-{stamp}.txt")), format!("r{stamp}")).unwrap();
        }
        std::fs::write(dir.join("recovered-12.md"), "text").unwrap();
        std::fs::write(dir.join("recovered-1.md"), "old").unwrap();
        let p = take_pending(&dir).unwrap();
        assert_eq!(p.text, "r12");
        assert!(p.recovered);
        assert!(take_pending(&dir).is_none());
        assert!(!dir.join("crash-1.txt").exists());
        assert!(!dir.join("recovered-1.md").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), KEEP + 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unsaved_text_is_released_only_by_its_own_save() {
        let mut u = Unsaved { id: 0, text: None };
        let a = u.hold("one");
        let b = u.hold("two");
        u.release(a);
        assert_eq!(u.text.as_deref(), Some("two"));
        u.release(b);
        assert!(u.text.is_none());
    }

    #[test]
    fn issue_url_is_encoded_and_bounded() {
        let report = format!("a\nb\nc\nd\n\npanicked at x.rs:1:\n{}", "é".repeat(5000));
        let url = issue_url(&report);
        assert!(url.starts_with(ISSUES));
        assert!(url.contains("title=Crash%3A%20panicked%20at%20x.rs%3A1%3A"));
        assert!(!url.contains(' ') && !url.contains('\n'));
        assert!(url.len() < URL_BODY_MAX * 3 + 500);
    }
}
