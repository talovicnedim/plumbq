//! Adapters against a scripted fake provider on a local socket, and the agent loop against the
//! real MCP server in-process. No network, no keychain.

// Test helpers outside `#[test]` functions fail loudly too.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod agent;
mod providers;
mod turn;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use plumbq_mcp::{Approval, Approver, HttpServer, McpSettings};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::settings::{ProviderConfig, ProviderKind};
use crate::tools::Tools;
use crate::types::ToolSpec;

pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ac-test-assistant-{name}-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Serves `replies` (SSE bodies, or whole responses starting with `HTTP/`) in order, one per
/// request, and records the request bodies. After the script runs out, the last reply repeats.
async fn fake_provider(replies: Vec<String>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = seen.clone();
    tokio::spawn(async move {
        let mut index = 0;
        while let Ok((mut socket, _)) = listener.accept().await {
            let body = read_body(&mut socket).await;
            recorded
                .lock()
                .unwrap()
                .push(serde_json::from_str(&body).unwrap_or(Value::Null));
            let reply = replies[index.min(replies.len() - 1)].clone();
            index += 1;
            let response = if reply.starts_with("HTTP/") {
                reply
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                )
            };
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    (url, seen)
}

async fn read_body(socket: &mut tokio::net::TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = socket.read(&mut buf).await.unwrap();
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data).to_string();
        if let Some(end) = text.find("\r\n\r\n") {
            let length: usize = text[..end]
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap_or(0))
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                return text[end + 4..].to_string();
            }
        }
        if n == 0 {
            return String::new();
        }
    }
}

fn sse(events: &[Value]) -> String {
    let mut out: String = events.iter().map(|e| format!("data: {e}\n\n")).collect();
    out.push_str("data: [DONE]\n\n");
    out
}

fn openai_text(text: &str) -> String {
    sse(&[json!({"choices": [{"delta": {"content": text}}]})])
}

fn openai_tool(id: &str, name: &str, args: Value) -> String {
    sse(&[json!({"choices": [{"delta": {"tool_calls": [
        {"index": 0, "id": id, "function": {"name": name, "arguments": args.to_string()}}
    ]}}]})])
}

fn config(kind: ProviderKind, url: &str, model: &str) -> ProviderConfig {
    ProviderConfig {
        base_url: url.into(),
        model: model.into(),
        ..ProviderConfig::new(kind, "test")
    }
}

fn spec() -> ToolSpec {
    ToolSpec {
        name: "workspace_tree".into(),
        description: "Show the workspace".into(),
        input_schema: json!({"type": "object", "properties": {}}),
    }
}

struct Workspace {
    root: PathBuf,
    data: PathBuf,
    approvals: mpsc::Receiver<Approval>,
    tools: Tools,
}

async fn workspace() -> Workspace {
    let root = temp_dir("ws");
    std::fs::create_dir_all(root.join("users")).unwrap();
    std::fs::write(
        root.join("users/list.http"),
        "### List users\nGET https://api.test/users\n",
    )
    .unwrap();
    let data = temp_dir("data");
    let (approver, approvals) = Approver::channel();
    let server = plumbq_mcp::in_app(HttpServer {
        workspace: Arc::new(RwLock::new(Some(root.clone()))),
        data_dir: data.clone(),
        settings: Arc::new(RwLock::new(McpSettings::default())),
        approver,
        write_lock: Default::default(),
    });
    let tools = Tools::connect(server, "Chat · test").await.unwrap();
    Workspace {
        root,
        data,
        approvals,
        tools,
    }
}
