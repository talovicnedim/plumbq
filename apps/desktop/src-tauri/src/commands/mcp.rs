use std::path::PathBuf;
use std::sync::Arc;

use plumbq_core::approvals::{ApprovalAnswer, ApprovalRequest};
use plumbq_mcp::activity::{self, ActivityEntry};
use plumbq_mcp::apps::{self, McpApp, McpAppStatus, McpTarget};
use plumbq_mcp::setup::{self, McpClientSetup};
use plumbq_mcp::{McpSettings, settings};
use serde::Serialize;
use tauri::State;

use crate::assistant::AssistantState;
use crate::error::AppError;
use crate::mcp::{McpState, McpStatus};
use crate::state::AppState;

/// Where the MCP server binary is, for the setup snippets of the apps that start it.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpBinary {
    pub path: String,
    /// False in dev until `cargo build -p plumbq-mcp` has run; bundles always include it.
    pub exists: bool,
}

#[tauri::command(async)]
#[specta::specta]
pub fn mcp_binary() -> Result<McpBinary, AppError> {
    let path = binary_path()?;
    Ok(McpBinary {
        exists: path.is_file(),
        path: path.display().to_string(),
    })
}

/// The server sits next to the app's own executable: in bundles it is a Tauri sidecar
/// (`Contents/MacOS/` on macOS, the install folder on Windows and Linux), in dev it is built
/// into the same `target/debug/` folder.
fn binary_path() -> Result<PathBuf, AppError> {
    let exe = std::env::current_exe().map_err(|err| AppError {
        kind: "io".into(),
        message: format!("failed to locate the app: {err}"),
    })?;
    let name = if cfg!(windows) {
        "plumbq-mcp.exe"
    } else {
        "plumbq-mcp"
    };
    Ok(exe.with_file_name(name))
}

#[tauri::command]
#[specta::specta]
pub fn mcp_status(mcp: State<'_, Arc<McpState>>) -> McpStatus {
    mcp.status()
}

/// Saves the AI settings. "Turn AI off" (`turnedOff`) also stops every running chat turn.
#[tauri::command]
#[specta::specta]
pub async fn mcp_update(
    mcp: State<'_, Arc<McpState>>,
    assistant: State<'_, AssistantState>,
    settings: McpSettings,
) -> Result<McpStatus, AppError> {
    let status = mcp.update(settings).await?;
    if status.settings.turned_off {
        assistant.stop_all();
    }
    Ok(status)
}

/// A new secret URL; clients set up with the old one stop working.
#[tauri::command]
#[specta::specta]
pub async fn mcp_token_regenerate(mcp: State<'_, Arc<McpState>>) -> Result<McpStatus, AppError> {
    let current = mcp.status().settings;
    mcp.update(McpSettings {
        token: settings::new_token(),
        ..current
    })
    .await
}

fn home() -> Result<PathBuf, AppError> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| plumbq_core::Error::NotFound("home folder".into()).into())
}

/// The running server's URL; one-click setup needs it.
fn running_url(mcp: &McpState) -> Result<String, AppError> {
    mcp.status()
        .url
        .ok_or_else(|| plumbq_core::Error::Invalid("turn on the MCP server first".into()).into())
}

/// Which AI apps are on this machine and whether the server is set up in them.
#[tauri::command(async)]
#[specta::specta]
pub fn mcp_apps_list(mcp: State<'_, Arc<McpState>>) -> Result<Vec<McpAppStatus>, AppError> {
    let url = mcp.status().url.unwrap_or_default();
    let binary = binary_path()?;
    let target = McpTarget {
        url: &url,
        binary: &binary,
    };
    Ok(apps::status(&home()?, &target))
}

/// Adds the server to the app's MCP config (one click, no copy-paste). Claude Desktop and
/// Codex get the bundled binary as a bridge to the same server.
#[tauri::command(async)]
#[specta::specta]
pub fn mcp_apps_connect(mcp: State<'_, Arc<McpState>>, app: McpApp) -> Result<(), AppError> {
    let binary = binary_path()?;
    if !app.uses_url() && !binary.is_file() {
        return Err(plumbq_core::Error::NotFound(format!(
            "the MCP server at {} (run `cargo build -p plumbq-mcp` in development)",
            binary.display()
        ))
        .into());
    }
    let url = running_url(&mcp)?;
    let target = McpTarget {
        url: &url,
        binary: &binary,
    };
    Ok(apps::connect(app, &home()?, &target)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn mcp_apps_disconnect(app: McpApp) -> Result<(), AppError> {
    Ok(apps::disconnect(app, &home()?)?)
}

/// Copyable setup for every AI client, for the server running now (empty while it's off).
#[tauri::command(async)]
#[specta::specta]
pub fn mcp_clients_list(
    state: State<'_, AppState>,
    mcp: State<'_, Arc<McpState>>,
) -> Result<Vec<McpClientSetup>, AppError> {
    let Some(url) = mcp.status().url else {
        return Ok(Vec::new());
    };
    let binary = binary_path()?;
    let root = state
        .root()
        .map(|r| r.display().to_string())
        .unwrap_or_default();
    let target = McpTarget {
        url: &url,
        binary: &binary,
    };
    Ok(setup::clients(&target, &root))
}

/// Agent requests waiting for the user, oldest first.
#[tauri::command(async)]
#[specta::specta]
pub fn mcp_approvals_list(mcp: State<'_, Arc<McpState>>) -> Vec<ApprovalRequest> {
    mcp.pending()
}

/// Approving a strong request (production writes, deletes) needs `typed`: the request's
/// `confirmName` as the user typed it. Without it the request keeps waiting. Returns the
/// settings, which "Always allow" changes.
#[tauri::command(async)]
#[specta::specta]
pub fn mcp_approvals_answer(
    mcp: State<'_, Arc<McpState>>,
    id: String,
    answer: ApprovalAnswer,
    typed: Option<String>,
) -> Result<McpStatus, AppError> {
    mcp.answer(&id, answer, typed.as_deref())
}

/// What the chat and AI apps did, newest first; everything the log keeps without a `limit`.
#[tauri::command(async)]
#[specta::specta]
pub fn mcp_activity_list(
    mcp: State<'_, Arc<McpState>>,
    limit: Option<u32>,
) -> Result<Vec<ActivityEntry>, AppError> {
    Ok(activity::list(mcp.data_dir(), limit.unwrap_or(u32::MAX))?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn mcp_activity_clear(mcp: State<'_, Arc<McpState>>) -> Result<(), AppError> {
    Ok(activity::clear(mcp.data_dir())?)
}
