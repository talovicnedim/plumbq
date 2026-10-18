//! The MCP server's listener: started, stopped, and moved when its address (port or token)
//! changes. Everything else in the settings is read live by the server, so it needs no restart.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use plumbq_mcp::{HttpServer, McpSettings};
use tauri::async_runtime::{self, JoinHandle};
use tokio_util::sync::CancellationToken;

/// How long a restart waits for the old server to let go of its port and connections.
const STOP_WAIT: Duration = Duration::from_secs(3);

pub(super) struct Server {
    http: HttpServer,
    running: Mutex<Running>,
    /// One restart at a time, so two settings changes can't both bind the port.
    restarting: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct Running {
    shutdown: Option<CancellationToken>,
    task: Option<JoinHandle<()>>,
    url: Option<String>,
    error: Option<String>,
}

impl Server {
    pub fn new(http: HttpServer) -> Self {
        Self {
            http,
            running: Mutex::default(),
            restarting: tokio::sync::Mutex::new(()),
        }
    }

    /// The server as the app runs it, also for the built-in chat.
    pub fn http(&self) -> &HttpServer {
        &self.http
    }

    /// The URL agents connect to while it runs, and why it doesn't run although it should.
    pub fn url_and_error(&self) -> (Option<String>, Option<String>) {
        let running = self.lock();
        (running.url.clone(), running.error.clone())
    }

    /// Whether going from `old` to `new` settings needs [`Self::restart`]: the address
    /// changed, or the server should run but doesn't (e.g. its port was taken before).
    pub fn needs_restart(&self, old: &McpSettings, new: &McpSettings) -> bool {
        address(old) != address(new) || (address(new).is_some() && self.lock().url.is_none())
    }

    /// Starts listening if the settings say so. Only for startup, where nothing runs yet.
    pub fn start(&self) {
        let settings = self.settings();
        let mut running = self.lock();
        *running = Running::default();
        let Some((port, token)) = address(&settings) else {
            return;
        };
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        // Bind here, not in the task, so a taken port shows up in the AI dialog right away.
        let listener = match bind(addr) {
            Ok(listener) => listener,
            Err(err) => {
                running.error = Some(format!("can't listen on port {port}: {err}"));
                return;
            }
        };
        let shutdown = CancellationToken::new();
        running.url = Some(format!("http://{addr}{}", plumbq_mcp::path(token)));
        running.task = Some(serve_in_background(
            listener,
            self.http.clone(),
            shutdown.clone(),
        ));
        running.shutdown = Some(shutdown);
        tracing::info!("MCP server listening on {addr}");
    }

    /// Stops the server, waits until it let go of its port, then starts it as the settings now
    /// say. The old listener must be closed first, or binding the same port fails.
    pub async fn restart(&self) {
        let _one_at_a_time = self.restarting.lock().await;
        let (shutdown, task) = {
            let mut running = self.lock();
            (running.shutdown.take(), running.task.take())
        };
        if let Some(shutdown) = shutdown {
            shutdown.cancel();
        }
        if let Some(task) = task
            && tokio::time::timeout(STOP_WAIT, task).await.is_err()
        {
            tracing::warn!("the old MCP server took over {STOP_WAIT:?} to stop");
        }
        self.start();
    }

    fn settings(&self) -> McpSettings {
        self.http
            .settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Running> {
        self.running.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Where the server listens: nowhere while it is off or AI is turned off.
fn address(settings: &McpSettings) -> Option<(u16, &str)> {
    (settings.enabled && !settings.turned_off).then_some((settings.port, settings.token.as_str()))
}

/// Binds synchronously, so a taken port is reported right away. Startup runs on the main
/// thread, which has no Tokio runtime: the listener is handed to Tokio only inside the task.
fn bind(addr: SocketAddr) -> std::io::Result<std::net::TcpListener> {
    let listener = std::net::TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

fn serve_in_background(
    listener: std::net::TcpListener,
    server: HttpServer,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    async_runtime::spawn(async move {
        let served = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => plumbq_mcp::serve_http(listener, server, shutdown).await,
            Err(err) => Err(err.into()),
        };
        if let Err(err) = served {
            tracing::error!("MCP server stopped: {err}");
        }
    })
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
