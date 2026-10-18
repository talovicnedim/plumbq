//! What tools read from the server: the workspace root, the user's settings, and where
//! history and cookies go.

use std::path::{Path, PathBuf};

use plumbq_core::http_file::Request;
use plumbq_core::permissions::AgentPermissions;
use plumbq_core::requests::{self, IdChange};
use plumbq_core::send::SendContext;
use plumbq_core::{cookies, history};
use tokio::sync::MutexGuard;

use super::call_log::saved_settings;
use super::{PlumbqMcp, Workspace, error_text};
use crate::settings::{self, McpSettings};

impl PlumbqMcp {
    /// The workspace root, or why there is none.
    pub fn root(&self) -> Result<PathBuf, String> {
        match &self.workspace {
            Workspace::Fixed(root) => Ok(root.clone()),
            Workspace::Shared(open) => open
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .ok_or_else(|| {
                    "no workspace is open in the Plumbq app; ask the user to open one".into()
                }),
        }
    }

    /// Held by every tool that changes files.
    pub async fn write_lock(&self) -> MutexGuard<'_, ()> {
        self.write_lock.lock().await
    }

    /// The user's current settings: for the stdio server the saved ones as the call read them
    /// when it started ([`Self::call_settings`]), the app's otherwise.
    fn current_settings(&self) -> McpSettings {
        if let Some(saved) = saved_settings() {
            return saved;
        }
        if let Some(file) = &self.settings_file
            && let Ok(saved) = settings::load(file)
        {
            return saved;
        }
        self.settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Reads the saved settings once for a call. `Ok(Some)` for the stdio server: the call
    /// runs on them. `Err` says why no call may run: the user turned AI off, or the saved
    /// settings (permissions, hidden environments) can't be read, and running on defaults
    /// could show what they hid.
    pub(crate) fn call_settings(&self) -> Result<Option<McpSettings>, String> {
        let file = self.settings_file.as_ref().or(self.settings_check.as_ref());
        let saved = match file.map(|file| (file, settings::load(file))) {
            Some((file, Err(err))) => {
                return Err(format!(
                    "the Plumbq app's MCP settings can't be read ({err}); ask the user to fix \
                     {} or save the AI settings again in the app",
                    file.display()
                ));
            }
            Some((_, Ok(saved))) => Some(saved),
            None => None,
        };
        let turned_off = match &saved {
            Some(saved) => saved.turned_off,
            None => self.current_settings().turned_off,
        };
        if turned_off {
            return Err(
                "AI is turned off in the Plumbq app; nothing can be done until the user turns \
                 it back on there"
                    .to_string(),
            );
        }
        Ok(saved.filter(|_| self.settings_file.is_some()))
    }

    /// The user's current choices; read-only (the flag or the setting) overrides them.
    pub fn permissions(&self) -> AgentPermissions {
        let current = self.current_settings();
        if self.read_only || current.read_only {
            return AgentPermissions::read_only();
        }
        let mut permissions = current.permissions;
        permissions.allow_process_env |= self.allow_process_env;
        permissions
    }

    /// Started with `--read-only`, or the user turned Read-only on in the app.
    pub fn is_read_only(&self) -> bool {
        self.read_only || self.current_settings().read_only
    }

    /// Environments the user hid from the AI.
    pub fn hidden_environments(&self) -> Vec<String> {
        self.current_settings().hidden_environments
    }

    /// The request's [`requests`] `expected` source for a write, when the agent passed the
    /// `version` it read. A version that no longer matches is a conflict: the agent must read
    /// the request again rather than overwrite or delete the wrong one.
    pub fn expected_source(
        &self,
        root: &Path,
        id: &str,
        version: Option<&str>,
    ) -> Result<Option<String>, String> {
        let Some(version) = version else {
            return Ok(None);
        };
        let current = requests::get(root, id).map_err(error_text)?;
        check_version(id, Some(version), &current)?;
        Ok(Some(current.source))
    }

    /// Moves history along with changed ids, so chaining and history_compare keep working.
    /// The files already changed, so a history failure is only logged.
    pub fn follow_history(&self, root: &Path, changes: &[IdChange]) {
        if let Some(dir) = &self.history_dir
            && let Err(err) = history::follow(dir, root, changes)
        {
            tracing::warn!("failed to move history along with requests: {err}");
        }
    }

    /// History and cookies for a send. In read-only mode the jar is a copy, so cookies are
    /// sent but nothing the server receives is written to the app's jar.
    pub fn send_context(&self, root: &Path, environment: Option<&str>) -> SendContext {
        SendContext {
            history_dir: self.history_dir.clone(),
            cookie_dir: match (&self.cookie_dir, self.is_read_only()) {
                (Some(dir), true) => copy_jar(dir, root, environment),
                (dir, false) => dir.clone(),
                (None, true) => None,
            },
            agent: true,
            ..Default::default()
        }
    }
}

/// Refuses a write when the agent's `version` isn't the one of `read`.
pub fn check_version(id: &str, version: Option<&str>, read: &Request) -> Result<(), String> {
    match version {
        Some(version) if requests::version(read) != version => {
            Err(error_text(plumbq_core::Error::Conflict(format!(
                "request {id} changed since you read it (version {version}); call requests_get \
                 again and redo the change"
            ))))
        }
        _ => Ok(()),
    }
}

/// Copies an environment's jar into a folder of this process and returns it; `None` (no
/// cookies) when the copy fails.
fn copy_jar(dir: &Path, root: &Path, environment: Option<&str>) -> Option<PathBuf> {
    let scratch = std::env::temp_dir().join(format!("plumbq-mcp-cookies-{}", std::process::id()));
    let jar = cookies::list(dir, root, environment).ok()?;
    cookies::save(&scratch, root, environment, &jar).ok()?;
    Some(scratch)
}
