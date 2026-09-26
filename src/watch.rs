//! Live filesystem watching for the open space: external edits, new files
//! and deletions surface in the sidebar/editor without waiting for window
//! activation.

use std::path::Path;

use futures::channel::mpsc::UnboundedReceiver;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Recursive watcher over a space; coalesced change signals arrive on the receiver.
pub(crate) struct SpaceWatcher {
    _inner: RecommendedWatcher,
}

pub(crate) fn watch(dir: &Path) -> notify::Result<(SpaceWatcher, UnboundedReceiver<()>)> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let mut inner = RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            // Dotfiles only (incl. `.{name}.abstract-tmp` from write_atomic):
            // nothing the UI shows.
            if event.paths.iter().all(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            }) {
                return;
            }
            tx.unbounded_send(()).ok();
        },
        notify::Config::default(),
    )?;
    inner.watch(dir, RecursiveMode::Recursive)?;
    Ok((SpaceWatcher { _inner: inner }, rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_create_signals_watch() {
        let dir = std::env::temp_dir().join(format!("abstract-watch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (_w, mut rx) = watch(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "# hi").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut got = false;
        while std::time::Instant::now() < deadline {
            if rx.try_recv().is_ok() {
                got = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(got, "watcher did not signal within 3s");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
