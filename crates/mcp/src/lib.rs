//! Plumbq's MCP server: lets AI agents (Claude, ChatGPT, Cursor, …) read, organize and
//! send the requests of a workspace, asking the user in the app before anything changes.
//!
//! Two ways to run it:
//! - **Inside the desktop app** ([`serve_http`]): Streamable HTTP on `127.0.0.1` at
//!   `/mcp/<token>`, working on the workspace open in the app. The random token in the path is
//!   the only credential, so the same URL works for clients that can't send headers (ChatGPT
//!   connectors behind a tunnel) and for those that can. Approvals are dialogs in the app.
//! - **As `plumbq-mcp --url <address>`** ([`bridge`]): a stdio bridge to that HTTP server,
//!   for AI apps that only start local commands (Claude Desktop, Codex).
//! - **As `plumbq-mcp --workspace <folder>`** (stdio, headless use only) for one workspace
//!   folder without the app's server; approvals go to the running app through request files,
//!   and are denied when nobody answers.

pub mod activity;
pub mod approval;
pub mod apps;
pub mod bridge;
mod config_edit;
mod error;
mod format;
mod outline;
pub mod prompts;
mod server;
pub mod settings;
pub mod setup;
mod tools;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub use approval::{Approval, Approver};
pub use bridge::Bridge;
pub use error::{Error, Result};
pub use server::{PlumbqMcp, Workspace};
pub use settings::McpSettings;

/// Held while a tool changes files. One per app, shared by its HTTP server and every chat, so
/// their read-modify-write of `folder.toml` and environment files can't lose updates.
pub type WriteLock = Arc<tokio::sync::Mutex<()>>;

/// The URL path clients connect to.
pub fn path(token: &str) -> String {
    format!("/mcp/{token}")
}

/// Where a server inside the app finds the user's data.
#[derive(Debug, Clone)]
pub struct HttpServer {
    /// The workspace open in the app.
    pub workspace: Arc<RwLock<Option<PathBuf>>>,
    /// The app data dir: history, cookies and the activity log live here.
    pub data_dir: PathBuf,
    pub settings: Arc<RwLock<McpSettings>>,
    pub approver: Approver,
    pub write_lock: WriteLock,
}

/// The server as the app runs it: on the open workspace, sharing the app's history, cookies,
/// activity log and approval dialogs. Used for the HTTP server and the built-in assistant.
pub fn in_app(server: HttpServer) -> PlumbqMcp {
    let settings_file = settings::path(&server.data_dir);
    PlumbqMcp::new(
        Workspace::Shared(server.workspace),
        server.settings,
        server.approver,
    )
    .with_history_dir(Some(server.data_dir.join("history")))
    .with_cookie_dir(Some(server.data_dir.join("cookies")))
    .with_activity_dir(Some(server.data_dir))
    .with_settings_check(Some(settings_file))
    .with_write_lock(server.write_lock)
}

/// Serves MCP on `listener` until `shutdown` is cancelled.
pub async fn serve_http(
    listener: TcpListener,
    server: HttpServer,
    shutdown: CancellationToken,
) -> Result<()> {
    let token = server
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .token
        .clone();
    let mcp = in_app(server);
    // Host checks guard unauthenticated local servers against DNS rebinding. Here the secret
    // path already does that, and tunnels (needed for ChatGPT) send their own Host.
    let config = StreamableHttpServerConfig::default()
        .with_cancellation_token(shutdown.child_token())
        .disable_allowed_hosts();
    let service: StreamableHttpService<PlumbqMcp, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(mcp.clone()), Arc::default(), config);
    let router = axum::Router::new().nest_service(&path(&token), service);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move { shutdown.cancelled().await })
        .await?;
    Ok(())
}
