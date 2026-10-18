//! An agent connected in-process to the server as the app runs it, and a local echo server.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use plumbq_mcp::{Approval, Approver, HttpServer, McpSettings};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::common::temp;

pub struct Agent {
    pub root: PathBuf,
    pub settings: Arc<RwLock<McpSettings>>,
    pub approvals: mpsc::Receiver<Approval>,
    pub client: RunningService<RoleClient, ()>,
}

/// A workspace with a production environment `prod` and a request using it, and an agent
/// connected to the in-app server in-process.
pub async fn agent() -> Agent {
    start(false).await
}

/// Like the chat when the user doesn't share response bodies with it.
pub async fn agent_without_bodies() -> Agent {
    start(true).await
}

async fn start(hide_bodies: bool) -> Agent {
    plumbq_core::http_client::install_crypto();
    // As the app opens it: the real path, not a link to it.
    let root = plumbq_core::workspace::canonical_root(&temp("safety-ws")).unwrap();
    write(
        &root,
        "environments/prod.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://prod-only.test\"\n",
    );
    write(&root, "users/list.http", "GET {{baseUrl}}/users\n");
    let settings = Arc::new(RwLock::new(McpSettings::default()));
    let (approver, approvals) = Approver::channel();
    let server = plumbq_mcp::in_app(HttpServer {
        workspace: Arc::new(RwLock::new(Some(root.clone()))),
        data_dir: temp("safety-data"),
        settings: settings.clone(),
        approver,
        write_lock: Default::default(),
    })
    .with_hidden_bodies(hide_bodies);
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let running = server.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    Agent {
        root,
        settings,
        approvals,
        client: ().serve(client_io).await.unwrap(),
    }
}

/// Answers every request with the request it got, so tests can see what was sent; `GET /json`
/// gets `{"id":"body-value-42"}`.
pub async fn echo_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut got = Vec::new();
            let mut buf = vec![0; 4096];
            while !complete(&got) {
                match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            if got.starts_with(b"GET /json") {
                got = br#"{"id":"body-value-42"}"#.to_vec();
            }
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\
                 connection: close\r\n\r\n",
                got.len()
            );
            let _ = socket.write_all(reply.as_bytes()).await;
            let _ = socket.write_all(&got).await;
        }
    });
    format!("http://{addr}")
}

/// A local server that answers every request with a redirect to `location`.
pub async fn redirect_server(location: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0; 4096];
            let _ = socket.read(&mut buf).await;
            let reply = format!(
                "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\n\
                 connection: close\r\n\r\n"
            );
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    format!("http://{addr}")
}

/// Headers and as much body as `content-length` says.
fn complete(request: &[u8]) -> bool {
    let text = String::from_utf8_lossy(request);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().to_string())
        })
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    body.len() >= length
}

pub fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
