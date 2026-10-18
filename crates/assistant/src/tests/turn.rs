// Test helpers outside `#[test]` functions fail loudly too.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use plumbq_mcp::{Approver, HttpServer, McpSettings};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{config, fake_provider, openai_text, openai_tool, temp_dir};
use crate::agent::AgentContext;
use crate::chats;
use crate::settings::{AssistantSettings, ProviderKind};
use crate::turn::{self, Turn};
use crate::types::{ChatMessage, ChatRole};

fn workspace(name: &str, file: &str) -> PathBuf {
    let root = temp_dir(name);
    std::fs::write(root.join(file), "### Request\nGET https://api.test/\n").unwrap();
    root
}

fn server(open: &Path, settings: McpSettings) -> HttpServer {
    let (approver, _approvals) = Approver::channel();
    HttpServer {
        workspace: Arc::new(RwLock::new(Some(open.to_path_buf()))),
        data_dir: temp_dir("turn-data"),
        settings: Arc::new(RwLock::new(settings)),
        approver,
        write_lock: Default::default(),
    }
}

async fn send(
    server: HttpServer,
    root: &Path,
    url: &str,
    context: AgentContext,
) -> crate::Result<chats::Chat> {
    let config = config(ProviderKind::Ollama, url, "llama3");
    let turn = Turn {
        server,
        root,
        settings: &AssistantSettings::default(),
        config: &config,
        key: None,
    };
    let question = vec![ChatMessage::user("what's here?")];
    let stop = CancellationToken::new();
    turn::send(turn, "c1".into(), question, context, &stop, &mut |_| {}).await
}

#[tokio::test]
async fn a_failed_turn_keeps_the_tool_calls_that_ran() {
    let root = workspace("turn-fail", "list.http");
    let server = server(&root, McpSettings::default());
    let data_dir = server.data_dir.clone();
    let broken =
        "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
    let (url, _) = fake_provider(vec![
        openai_tool("t1", "workspace_tree", json!({})),
        broken.to_string(),
    ])
    .await;

    let err = send(server, &root, &url, AgentContext::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "provider");
    let saved = chats::get(&data_dir, &root, "c1").unwrap();
    let roles: Vec<ChatRole> = saved.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, [ChatRole::User, ChatRole::Assistant, ChatRole::Tool]);
    assert!(saved.messages[2].content.contains("list.http"));
}

#[tokio::test]
async fn a_turn_stays_on_the_workspace_it_started_in() {
    let started_in = workspace("turn-started", "mine.http");
    let opened_later = workspace("turn-later", "other.http");
    let server = server(&opened_later, McpSettings::default());
    let data_dir = server.data_dir.clone();
    let (url, _) = fake_provider(vec![
        openai_tool("t1", "workspace_tree", json!({})),
        openai_text("Done."),
    ])
    .await;

    let chat = send(server, &started_in, &url, AgentContext::default())
        .await
        .unwrap();
    let tree = &chat.messages[2].content;
    assert!(
        tree.contains("mine.http") && !tree.contains("other.http"),
        "{tree}"
    );
    assert!(chats::get(&data_dir, &started_in, "c1").is_ok());
    assert!(chats::get(&data_dir, &opened_later, "c1").is_err());
}

#[tokio::test]
async fn hidden_environments_never_reach_the_model() {
    for hidden_name in ["production", "Production"] {
        let root = workspace("turn-hidden", "list.http");
        let hidden = McpSettings {
            hidden_environments: vec![hidden_name.into()],
            ..McpSettings::default()
        };
        let (url, seen) = fake_provider(vec![openai_text("Okay.")]).await;
        let context = AgentContext {
            environment: Some("production".into()),
            ..AgentContext::default()
        };
        send(server(&root, hidden), &root, &url, context)
            .await
            .unwrap();
        let question = seen.lock().unwrap()[0]["messages"][1]["content"].to_string();
        assert!(
            !question.contains("production"),
            "{hidden_name}: {question}"
        );
        assert!(
            question.contains("environment: none selected"),
            "{question}"
        );
    }
}
