use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use notify::RecommendedWatcher;
use plumbq_core::history;
use plumbq_core::requests::IdChange;
use plumbq_core::send::SendContext;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::error::AppError;
use crate::watch;

/// Event sent to the UI when files in the open workspace change on disk.
pub const WORKSPACE_CHANGED: &str = "workspace-changed";

/// App-wide state: the open workspace (one per window for now) and where per-machine data
/// lives.
pub struct AppState {
    recent_path: PathBuf,
    history_dir: PathBuf,
    cookie_dir: PathBuf,
    open: Mutex<Option<OpenWorkspace>>,
    /// Sends in flight, by request id: a number unique to each send and the sender that cancels
    /// it.
    in_flight: Mutex<HashMap<String, (u64, oneshot::Sender<()>)>>,
    next_send: AtomicU64,
    /// Paths the user picked in a native dialog or dropped on the window. Commands only read
    /// files outside the workspace (imports, data files, new workspace parents) from this set.
    granted: Mutex<HashSet<PathBuf>>,
    /// The open workspace's root, shared with the in-app MCP server.
    shared_root: Arc<RwLock<Option<PathBuf>>>,
}

struct OpenWorkspace {
    root: PathBuf,
    // Kept alive for as long as the workspace is open; dropping it stops watching.
    _watcher: RecommendedWatcher,
}

impl AppState {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            recent_path: data_dir.join("recent.json"),
            history_dir: data_dir.join("history"),
            cookie_dir: data_dir.join("cookies"),
            open: Mutex::new(None),
            in_flight: Mutex::new(HashMap::new()),
            next_send: AtomicU64::new(0),
            granted: Mutex::new(HashSet::new()),
            shared_root: Arc::default(),
        }
    }

    /// Records a path the user chose, so later commands may use it.
    pub fn grant(&self, path: PathBuf) {
        let mut granted = self.granted.lock().unwrap_or_else(|e| e.into_inner());
        granted.insert(path);
    }

    /// `path` if the user chose it in a dialog or by dropping it; an error otherwise.
    pub fn granted(&self, path: &str) -> Result<PathBuf, AppError> {
        let path = PathBuf::from(path);
        let granted = self.granted.lock().unwrap_or_else(|e| e.into_inner());
        if granted.contains(&path) {
            return Ok(path);
        }
        Err(AppError {
            kind: "invalid".into(),
            message: format!("{} wasn't chosen in a dialog", path.display()),
        })
    }

    /// Registers a send of request `id`. Returns its number (for [`Self::finish_send`]) and
    /// what resolves when it is cancelled. A new send of the same request cancels the previous.
    pub fn start_send(&self, id: &str) -> (u64, oneshot::Receiver<()>) {
        let number = self.next_send.fetch_add(1, Ordering::Relaxed);
        let (cancel, cancelled) = oneshot::channel();
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, previous)) = in_flight.insert(id.to_string(), (number, cancel)) {
            // The previous send may already be done; then nobody listens, which is fine.
            let _ = previous.send(());
        }
        (number, cancelled)
    }

    /// Forgets send `number` of request `id`, unless a newer send has replaced it.
    pub fn finish_send(&self, id: &str, number: u64) {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        if in_flight
            .get(id)
            .is_some_and(|(current, _)| *current == number)
        {
            in_flight.remove(id);
        }
    }

    /// Cancels the send of request `id`. Returns whether one was in flight.
    pub fn cancel_send(&self, id: &str) -> bool {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        in_flight
            .remove(id)
            .is_some_and(|(_, cancel)| cancel.send(()).is_ok())
    }

    pub fn recent_path(&self) -> &Path {
        &self.recent_path
    }

    /// Moves history along with request ids an edit changed, so chaining and comparisons keep
    /// working. The files already changed, so a failure here is only logged.
    pub fn follow_history(&self, root: &Path, changes: &[IdChange]) {
        if let Err(err) = history::follow(self.history_dir(), root, changes) {
            tracing::warn!("failed to move history along with requests: {err}");
        }
    }

    pub fn history_dir(&self) -> &Path {
        &self.history_dir
    }

    pub fn cookie_dir(&self) -> &Path {
        &self.cookie_dir
    }

    /// Where `send` finds history (for chaining) and cookie jars.
    pub fn send_context(&self) -> SendContext {
        SendContext {
            history_dir: Some(self.history_dir.clone()),
            cookie_dir: Some(self.cookie_dir.clone()),
            ..Default::default()
        }
    }

    /// The open workspace's root as the MCP server sees it, updated on every open.
    pub fn shared_root(&self) -> Arc<RwLock<Option<PathBuf>>> {
        self.shared_root.clone()
    }

    /// The open workspace's root, or an error the UI answers by showing the welcome screen.
    pub fn root(&self) -> Result<PathBuf, AppError> {
        let open = self.open.lock().unwrap_or_else(|e| e.into_inner());
        open.as_ref()
            .map(|w| w.root.clone())
            .ok_or_else(AppError::no_workspace)
    }

    /// Opens a workspace and starts watching it, replacing the previous one.
    pub fn open(&self, app: &AppHandle, root: PathBuf) -> Result<(), AppError> {
        let app = app.clone();
        self.open_watching(root, move || {
            if let Err(err) = app.emit(WORKSPACE_CHANGED, ()) {
                tracing::warn!("failed to emit workspace change: {err}");
            }
        })
    }

    /// Holds the lock while it changes anything, so two opens at once can't leave the app on
    /// one workspace and the MCP server on the other.
    fn open_watching(
        &self,
        root: PathBuf,
        changed: impl Fn() + Send + 'static,
    ) -> Result<(), AppError> {
        let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
        let watcher = watch::start(&root, changed)?;
        *self.shared_root.write().unwrap_or_else(|e| e.into_inner()) = Some(root.clone());
        *open = Some(OpenWorkspace {
            root,
            _watcher: watcher,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ac-test-desktop-open-{name}-{}",
            &plumbq_mcp::settings::new_token()[..12]
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_open_changes_nothing_while_another_holds_the_workspace() {
        let state = Arc::new(AppState::new(&temp_dir("data")));
        let root = temp_dir("a");
        let held = state.open.lock().unwrap();
        let opening = {
            let (state, root) = (state.clone(), root.clone());
            std::thread::spawn(move || state.open_watching(root, || {}).unwrap())
        };
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            let shared = state.shared_root.read().unwrap().clone();
            assert_eq!(shared, None, "the MCP server moved before the app did");
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(held);
        opening.join().unwrap();
        let shared = state.shared_root().read().unwrap().clone();
        assert_eq!(shared, Some(root));
        assert_eq!(shared, Some(state.root().unwrap()));
    }
}
