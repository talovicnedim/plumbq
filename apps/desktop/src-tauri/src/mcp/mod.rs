//! Runs the MCP server inside the app, so agents work on the workspace open here and every
//! change they want is approved here, in a dialog. Also answers approval requests filed by the
//! stdio server (`plumbq-mcp`) running for another client.

mod approvals;
mod server;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use plumbq_core::approvals::{self as requests, ApprovalAnswer, ApprovalRequest};
use plumbq_core::permissions::{self, PermissionRule};
use plumbq_core::send;
use plumbq_mcp::{Approval, Approver, HttpServer, McpSettings, WriteLock, settings};
use serde::Serialize;
use tauri::{AppHandle, async_runtime};
use tokio::sync::mpsc;

use crate::error::AppError;
use approvals::{Approvals, forward, watch};
use server::Server;

/// What the AI dialog shows about the server.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub settings: McpSettings,
    /// The URL agents connect to, while the server runs.
    pub url: Option<String>,
    /// Why the server isn't running although enabled, e.g. the port is taken.
    pub error: Option<String>,
    /// Why the saved settings couldn't be read. Every AI call is refused until the user saves
    /// the AI settings again; the broken file is never overwritten before that.
    pub settings_error: Option<String>,
    /// Methods that only read data; the permission texts name them.
    pub safe_methods: Vec<String>,
}

pub struct McpState {
    data_dir: PathBuf,
    settings_path: PathBuf,
    settings_error: Mutex<Option<String>>,
    server: Server,
    approvals: Approvals,
}

impl McpState {
    /// Loads settings and starts forwarding approval requests (from the server here and from
    /// the stdio server's request files) to the UI.
    pub fn new(
        app: &AppHandle,
        data_dir: &Path,
        workspace: Arc<RwLock<Option<PathBuf>>>,
    ) -> Arc<Self> {
        let (state, approvals) = Self::load(data_dir, workspace);
        async_runtime::spawn(forward(app.clone(), state.clone(), approvals));
        async_runtime::spawn(watch(app.clone(), state.clone()));
        state
    }

    /// Loads settings. The receiver gets the approval requests of the server in the app.
    fn load(
        data_dir: &Path,
        workspace: Arc<RwLock<Option<PathBuf>>>,
    ) -> (Arc<Self>, mpsc::Receiver<Approval>) {
        let settings_path = settings::path(data_dir);
        let (loaded, settings_error) = match settings::load(&settings_path) {
            Ok(loaded) => (loaded, None),
            Err(err) => {
                tracing::warn!("failed to read MCP settings, AI calls are refused: {err}");
                let message = format!("failed to read {}: {err}", settings_path.display());
                (McpSettings::default(), Some(message))
            }
        };
        let (approver, approvals) = Approver::channel();
        let http = HttpServer {
            workspace,
            data_dir: data_dir.to_path_buf(),
            settings: Arc::new(RwLock::new(loaded)),
            approver,
            write_lock: WriteLock::default(),
        };
        let state = Arc::new(Self {
            data_dir: data_dir.to_path_buf(),
            settings_path,
            settings_error: Mutex::new(settings_error),
            server: Server::new(http),
            approvals: Approvals::new(data_dir.to_path_buf()),
        });
        (state, approvals)
    }

    pub fn status(&self) -> McpStatus {
        let (url, error) = self.server.url_and_error();
        McpStatus {
            settings: self.current(),
            url,
            error,
            settings_error: self
                .settings_error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            safe_methods: send::SAFE_METHODS.iter().map(|m| m.to_string()).collect(),
        }
    }

    /// Starts the server if it is on; for startup.
    pub fn start(&self) {
        self.server.start();
    }

    /// Saves new settings. Only a new address (port, token, on or off) restarts the server:
    /// permissions, read-only and hidden environments apply from the next call, so connected AI
    /// apps keep their sessions. The stdio server reads the same file. Turning AI off also
    /// denies every request still waiting.
    pub async fn update(&self, new: McpSettings) -> Result<McpStatus, AppError> {
        let old = self.save(new)?;
        let new = self.current();
        if new.turned_off {
            self.approvals.deny_all();
        }
        if self.server.needs_restart(&old, &new) {
            self.server.restart().await;
        }
        Ok(self.status())
    }

    pub fn settings(&self) -> McpSettings {
        self.current()
    }

    /// "Turn AI off" is on: no chat, no server, no approvals.
    pub fn ai_off(&self) -> bool {
        self.current().turned_off
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Requests waiting for the user, first come first. While AI is off they are denied
    /// instead, so nothing pops up.
    pub fn pending(&self) -> Vec<ApprovalRequest> {
        if self.ai_off() {
            self.approvals.deny_all();
            return Vec::new();
        }
        self.approvals.pending()
    }

    /// Answers a request; a strong one is approved only with its name in `typed`. "Always
    /// allow" then switches the kind of call to Allow, so the next one of that kind runs
    /// without asking. An answer that fails leaves the rules as they were.
    pub fn answer(
        &self,
        id: &str,
        answer: ApprovalAnswer,
        typed: Option<&str>,
    ) -> Result<McpStatus, AppError> {
        let always_allow = match answer {
            ApprovalAnswer::AlwaysAllow => self
                .approvals
                .find(id)
                .map(|request| requests::check_always_allow(&request))
                .transpose()?,
            ApprovalAnswer::AllowOnce | ApprovalAnswer::Deny => None,
        };
        self.approvals
            .answer(id, answer != ApprovalAnswer::Deny, typed)?;
        if let Some(permission) = always_allow {
            let mut allowed = self.current();
            allowed
                .permissions
                .set_rule(permission, PermissionRule::Allow);
            self.save(allowed)?;
            tracing::info!(target: "audit", "AI may now go on {} without asking", permissions::describe(permission));
        }
        Ok(self.status())
    }

    /// The server as the app runs it, for the built-in assistant: same workspace, policy and
    /// approval dialogs as agents connecting over HTTP, whether or not that server is on.
    pub fn http_server(&self) -> HttpServer {
        self.server.http().clone()
    }

    /// Checks and saves `new`, then uses it. Returns the settings it replaced.
    fn save(&self, new: McpSettings) -> Result<McpSettings, AppError> {
        settings::check(&new)?;
        settings::save(&self.settings_path, &new).map_err(|err| AppError {
            kind: "io".into(),
            message: format!("failed to save MCP settings: {err}"),
        })?;
        let old = std::mem::replace(&mut *self.settings_lock(), new);
        *self
            .settings_error
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        Ok(old)
    }

    fn current(&self) -> McpSettings {
        self.server
            .http()
            .settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn settings_lock(&self) -> std::sync::RwLockWriteGuard<'_, McpSettings> {
        self.server
            .http()
            .settings
            .write()
            .unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests;
