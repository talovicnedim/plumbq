//! The MCP server over real HTTP with a real MCP client: auth by token, the open workspace,
//! approvals and the activity log.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use common::{call, temp, text};
use plumbq_mcp::activity::{self, ActivityOutcome};
use plumbq_mcp::{Approval, Approver, HttpServer, McpSettings};
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientConfig, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{RoleClient, ServiceExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Harness {
    url: String,
    root: PathBuf,
    data_dir: PathBuf,
    workspace: Arc<RwLock<Option<PathBuf>>>,
    settings: Arc<RwLock<McpSettings>>,
    approvals: mpsc::Receiver<Approval>,
    shutdown: CancellationToken,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.shutdown.cancel();
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

async fn start() -> Harness {
    plumbq_core::http_client::install_crypto();
    let root = temp("ws");
    std::fs::create_dir_all(root.join("users")).unwrap();
    std::fs::write(
        root.join("users/list.http"),
        "### List users\nGET https://api.test/users\n",
    )
    .unwrap();
    let data_dir = temp("data");
    let settings = Arc::new(RwLock::new(McpSettings {
        enabled: true,
        ..McpSettings::default()
    }));
    let token = settings.read().unwrap().token.clone();
    let (approver, approvals) = Approver::channel();
    let workspace = Arc::new(RwLock::new(Some(root.clone())));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}{}",
        listener.local_addr().unwrap(),
        plumbq_mcp::path(&token)
    );
    let shutdown = CancellationToken::new();
    tokio::spawn(plumbq_mcp::serve_http(
        listener,
        HttpServer {
            workspace: workspace.clone(),
            data_dir: data_dir.clone(),
            settings: settings.clone(),
            approver,
            write_lock: Default::default(),
        },
        shutdown.clone(),
    ));
    Harness {
        url,
        root,
        data_dir,
        workspace,
        settings,
        approvals,
        shutdown,
    }
}

async fn client(url: &str) -> RunningService<RoleClient, ()> {
    ().serve(StreamableHttpClientTransport::from_uri(url.to_string()))
        .await
        .unwrap()
}

#[tokio::test]
async fn lists_tools_and_prompts_and_reads_the_open_workspace() {
    let harness = start().await;
    let client = client(&harness.url).await;
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.len() > 25, "{} tools", tools.len());
    let prompts = client.list_all_prompts().await.unwrap();
    let names: Vec<_> = prompts.iter().map(|p| p.name.to_string()).collect();
    for name in [
        "organize_workspace",
        "write_assertions",
        "document_api",
        "debug_failing_request",
    ] {
        assert!(
            names.contains(&name.to_string()),
            "{name} missing from {names:?}"
        );
    }

    let tree = call(&client, "workspace_tree", serde_json::json!({})).await;
    assert!(text(&tree).contains("users/list.http"), "{}", text(&tree));

    *harness.workspace.write().unwrap() = None;
    let none = call(&client, "workspace_tree", serde_json::json!({})).await;
    assert_eq!(none.is_error, Some(true));
    assert!(text(&none).contains("no workspace is open"));
}

#[tokio::test]
async fn wrong_token_is_rejected() {
    let harness = start().await;
    let wrong = harness.url.replace("/mcp/", "/mcp/x");
    let result = ().serve(StreamableHttpClientTransport::from_uri(wrong)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn changes_wait_for_approval_and_are_logged() {
    let mut harness = start().await;
    let client = client(&harness.url).await;

    let create = {
        let content = "### Get user\nGET https://api.test/users/1\n";
        call(
            &client,
            "requests_create",
            serde_json::json!({ "folder": "users", "content": content, "reason": "for the docs" }),
        )
    };
    let answer = async {
        let asked = harness.approvals.recv().await.unwrap();
        assert_eq!(asked.request.tool, "requests_create");
        assert_eq!(asked.request.reason.as_deref(), Some("for the docs"));
        assert!(
            asked
                .request
                .detail
                .contains("+GET https://api.test/users/1"),
            "{}",
            asked.request.detail
        );
        asked.respond.send(true).unwrap();
    };
    let (created, ()) = tokio::join!(create, answer);
    assert_ne!(created.is_error, Some(true), "{}", text(&created));
    assert!(harness.root.join("users/get-user.http").is_file());

    let remove = call(
        &client,
        "requests_remove",
        serde_json::json!({ "id": "users/list.http" }),
    );
    let deny = async {
        let asked = harness.approvals.recv().await.unwrap();
        assert!(
            asked.request.strong,
            "deleting needs the strong confirmation"
        );
        asked.respond.send(false).unwrap();
    };
    let (removed, ()) = tokio::join!(remove, deny);
    assert_eq!(removed.is_error, Some(true));
    assert!(text(&removed).contains("denied"));
    assert!(
        harness.root.join("users/list.http").is_file(),
        "nothing was deleted"
    );

    harness.settings.write().unwrap().read_only = true;
    let blocked = call(
        &client,
        "requests_remove",
        serde_json::json!({ "id": "users/list.http" }),
    )
    .await;
    assert_eq!(blocked.is_error, Some(true));
    let tree = call(&client, "workspace_tree", serde_json::json!({})).await;
    assert_ne!(tree.is_error, Some(true));

    let log = activity::list(&harness.data_dir, 10).unwrap();
    let outcomes: Vec<_> = log.iter().map(|e| (e.tool.as_str(), e.outcome)).collect();
    assert_eq!(
        outcomes,
        [
            ("workspace_tree", ActivityOutcome::Ran),
            ("requests_remove", ActivityOutcome::Blocked),
            ("requests_remove", ActivityOutcome::Denied),
            ("requests_create", ActivityOutcome::Approved),
        ]
    );
}

/// What Claude Desktop and Codex run: `plumbq-mcp --url <address>`, here over an in-memory
/// pipe instead of stdio.
async fn bridged_client(url: &str, name: &str) -> RunningService<RoleClient, ClientConfig> {
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let bridge = plumbq_mcp::Bridge::new(url.to_string());
    tokio::spawn(async move {
        let running = bridge.serve(server_io).await.unwrap();
        let _ = running.waiting().await;
    });
    ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new(name, "1"),
    )
    .serve(client_io)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_url_bridge_forwards_to_the_app_and_its_approvals() {
    let mut harness = start().await;
    let client = bridged_client(&harness.url, "claude-desktop").await;

    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.len() > 25, "{} tools", tools.len());
    let tree = client
        .call_tool(CallToolRequestParams::new("workspace_tree"))
        .await
        .unwrap();
    assert!(text(&tree).contains("users/list.http"), "{}", text(&tree));

    let content = "### Get user\nGET https://api.test/users/1\n";
    let args = serde_json::json!({ "folder": "users", "content": content, "reason": "docs" });
    let serde_json::Value::Object(args) = args else {
        panic!("args must be an object");
    };
    let create =
        client.call_tool(CallToolRequestParams::new("requests_create").with_arguments(args));
    let answer = async {
        let asked = harness.approvals.recv().await.unwrap();
        assert_eq!(
            asked.request.actor, "claude-desktop",
            "the AI app, not the bridge"
        );
        assert_eq!(asked.request.reason.as_deref(), Some("docs"));
        asked.respond.send(false).unwrap();
    };
    let (created, ()) = tokio::join!(create, answer);
    let created = created.unwrap();
    assert_eq!(created.is_error, Some(true));
    assert!(!harness.root.join("users/get-user.http").exists());
}

#[tokio::test]
async fn the_url_bridge_explains_when_the_app_is_not_running() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp/gone", listener.local_addr().unwrap());
    drop(listener);
    let client = bridged_client(&url, "codex").await;
    let err = client.list_all_tools().await.unwrap_err();
    assert!(err.to_string().contains("can't reach Plumbq"), "{err}");
}
