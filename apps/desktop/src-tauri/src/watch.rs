//! Watches the open workspace and says when its files changed, once per burst: a git checkout
//! of many files is one reload in the UI, not one per file.

use std::path::{Component, Path};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::AppError;

/// Changes within this time of the first one are one reload.
const BURST: Duration = Duration::from_millis(200);

/// Starts watching `root`; `changed` runs once per burst of changes that matter. Watching stops
/// when the returned watcher is dropped.
pub fn start(
    root: &Path,
    changed: impl Fn() + Send + 'static,
) -> Result<RecommendedWatcher, AppError> {
    let (events, bursts) = mpsc::channel();
    std::thread::spawn(move || coalesce(&bursts, BURST, changed));
    let watched = root.to_path_buf();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event) if event.paths.iter().any(|p| is_relevant(&watched, p)) => {
                let _ = events.send(());
            }
            Ok(_) => {}
            Err(err) => tracing::warn!("workspace watcher error: {err}"),
        })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    Ok(watcher)
}

/// Calls `changed` once for each burst: `window` after the first event, with every event that
/// came in meanwhile. Ends when the watcher (the sender) is gone.
fn coalesce(events: &Receiver<()>, window: Duration, changed: impl Fn()) {
    while events.recv().is_ok() {
        std::thread::sleep(window);
        while events.try_recv().is_ok() {}
        changed();
    }
}

/// Changes inside `.git`, `node_modules` and the like don't affect the workspace; skipping them
/// avoids reloading on every commit or install. Only the part below the workspace root counts,
/// so a workspace inside e.g. `~/.config` still reloads.
fn is_relevant(root: &Path, path: &Path) -> bool {
    let inside = path.strip_prefix(root).unwrap_or(path);
    !inside.components().any(|c| match c {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            name.starts_with('.') || name == "node_modules" || name == "target"
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    use super::*;

    fn wait_for(count: &AtomicUsize, at_least: usize) -> usize {
        let deadline = Instant::now() + Duration::from_secs(5);
        while count.load(Ordering::SeqCst) < at_least && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        count.load(Ordering::SeqCst)
    }

    #[test]
    fn a_burst_of_changes_is_one_reload() {
        let (events, bursts) = mpsc::channel();
        let reloads = Arc::new(AtomicUsize::new(0));
        let counted = reloads.clone();
        std::thread::spawn(move || {
            coalesce(&bursts, BURST, move || {
                counted.fetch_add(1, Ordering::SeqCst);
            })
        });
        for _ in 0..100 {
            events.send(()).unwrap();
        }
        assert_eq!(wait_for(&reloads, 1), 1);
        events.send(()).unwrap();
        assert_eq!(wait_for(&reloads, 2), 2, "a later change reloads again");
    }

    #[test]
    fn relevance_ignores_dot_dirs_only_below_the_root() {
        let root = Path::new("/home/ada/.config/api");
        assert!(is_relevant(root, &root.join("users/list.http")));
        assert!(!is_relevant(root, &root.join(".git/HEAD")));
        assert!(!is_relevant(root, &root.join("node_modules/x")));
    }
}
