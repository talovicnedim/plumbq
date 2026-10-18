use std::sync::{Arc, RwLock};

use plumbq_core::send::{Response, SentRequest};
use rmcp::handler::server::wrapper::Parameters;

use super::*;
use crate::approval::Approver;
use crate::server::Workspace;
use crate::settings::McpSettings;

fn response(url: &str) -> Response {
    Response {
        status: 200,
        status_text: "OK".into(),
        headers: Vec::new(),
        body: "{}".into(),
        size: 2,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 5,
        sent: SentRequest {
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    }
}

#[tokio::test]
async fn sends_to_hidden_environments_never_show_up() {
    let base = std::env::temp_dir().join(format!("ac-mcp-history-{}", uuid::Uuid::new_v4()));
    let (root, dir) = (base.join("ws"), base.join("history"));
    std::fs::create_dir_all(&root).unwrap();
    history::add(
        &dir,
        &root,
        "a.http",
        Some("dev"),
        response("https://dev.test/"),
    )
    .unwrap();
    let secret = history::add(
        &dir,
        &root,
        "a.http",
        Some("prod"),
        response("https://prod.test/"),
    )
    .unwrap();
    let settings = Arc::new(RwLock::new(McpSettings {
        hidden_environments: vec!["prod".into()],
        ..McpSettings::default()
    }));
    let server = PlumbqMcp::new(Workspace::Fixed(root), settings, Approver::Nobody)
        .with_history_dir(Some(dir));

    let list = server
        .history_list(Parameters(ListParams {
            request_id: "a.http".into(),
        }))
        .await
        .unwrap();
    assert!(list.contains("dev.test"), "{list}");
    assert!(!list.contains("prod"), "{list}");
    let get = server
        .history_get(Parameters(GetParams {
            request_id: "a.http".into(),
            id: secret.id,
        }))
        .await
        .unwrap_err();
    assert!(get.contains("hidden"), "{get}");
    let compare = server
        .history_compare(Parameters(CompareParams {
            request_id: "a.http".into(),
            from: None,
            to: None,
        }))
        .await
        .unwrap_err();
    assert!(compare.contains("at least two"), "{compare}");
    std::fs::remove_dir_all(base).unwrap();
}
