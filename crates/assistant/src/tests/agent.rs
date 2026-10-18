// Test helpers outside `#[test]` functions fail loudly too.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{config, fake_provider, openai_text, openai_tool, workspace};
use crate::agent::{Agent, AgentContext, AgentEvent, MAX_STEPS};
use crate::settings::ProviderKind;
use crate::tools::Tools;
use crate::types::{ChatMessage, ChatRole};

#[tokio::test]
async fn tools_list_and_run_in_process() {
    let ws = workspace().await;
    let specs = ws.tools.specs().await.unwrap();
    assert!(specs.iter().any(|s| s.name == "workspace_tree"));
    assert!(specs.iter().all(|s| s.input_schema["type"] == "object"));
    let (text, ok) = ws
        .tools
        .call("workspace_tree", json!({}), &CancellationToken::new())
        .await
        .unwrap();
    assert!(ok, "{text}");
    assert!(text.contains("users/list.http"));
    assert!(!ws.tools.quick_actions().await.unwrap().is_empty());
}

#[tokio::test]
async fn agent_runs_tools_and_answers() {
    let ws = workspace().await;
    let (url, seen) = fake_provider(vec![
        openai_tool("c1", "workspace_tree", json!({})),
        openai_text("You have one request: List users."),
    ])
    .await;
    let config = config(ProviderKind::Ollama, &url, "llama3");
    let agent = Agent {
        config: &config,
        key: None,
        tools: &ws.tools,
    };
    let mut events = Vec::new();
    let mut messages = vec![ChatMessage::user("what's here?")];
    agent
        .run(
            &AgentContext::default(),
            &mut messages,
            &CancellationToken::new(),
            &mut |e| events.push(e),
        )
        .await
        .unwrap();

    assert_eq!(
        messages.last().unwrap().content,
        "You have one request: List users."
    );
    let result = messages.iter().find(|m| m.role == ChatRole::Tool).unwrap();
    assert!(result.content.contains("users/list.http") && !result.is_error);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolFinished { ok: true, .. }))
    );
    // The second model call got the tool result.
    assert_eq!(seen.lock().unwrap()[1]["messages"][3]["role"], "tool");
    let log = plumbq_mcp::activity::list(&ws.data, 10).unwrap();
    assert_eq!(log[0].actor, "Chat · test");
}

#[tokio::test]
async fn denied_changes_are_reported_to_the_model() {
    let mut ws = workspace().await;
    let (url, _) = fake_provider(vec![
        openai_tool(
            "c1",
            "requests_create",
            json!({"folder": "users", "content": "### Remove all\nDELETE https://api.test/users", "reason": "test"}),
        ),
        openai_text("Okay, I won't."),
    ])
    .await;
    let user = tokio::spawn(async move {
        let approval = ws.approvals.recv().await.unwrap();
        assert_eq!(approval.request.actor, "Chat · test");
        approval.respond.send(false).unwrap();
        ws.approvals
    });
    let config = config(ProviderKind::Ollama, &url, "llama3");
    let agent = Agent {
        config: &config,
        key: None,
        tools: &ws.tools,
    };
    let mut messages = vec![ChatMessage::user("add a request")];
    agent
        .run(
            &AgentContext::default(),
            &mut messages,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .await
        .unwrap();
    user.await.unwrap();

    let result = messages.iter().find(|m| m.role == ChatRole::Tool).unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("denied"), "{}", result.content);
    assert!(!ws.root.join("users/remove-all.http").exists());
}

#[tokio::test]
async fn agent_stops_after_max_steps_and_when_cancelled() {
    let ws = workspace().await;
    let (url, seen) = fake_provider(vec![openai_tool("c1", "workspace_tree", json!({}))]).await;
    let config = config(ProviderKind::Ollama, &url, "llama3");
    let agent = Agent {
        config: &config,
        key: None,
        tools: &ws.tools,
    };
    let mut messages = vec![ChatMessage::user("loop")];
    agent
        .run(
            &AgentContext::default(),
            &mut messages,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .await
        .unwrap();
    assert_eq!(seen.lock().unwrap().len(), MAX_STEPS);
    assert!(messages.last().unwrap().content.contains("Stopped after"));

    let stop = CancellationToken::new();
    stop.cancel();
    let mut messages = vec![ChatMessage::user("hi")];
    agent
        .run(&AgentContext::default(), &mut messages, &stop, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(messages.last().unwrap().content, "Stopped.");
}

#[test]
fn quick_action_arguments_come_from_the_open_request() {
    let names = [
        "id".to_string(),
        "folder".to_string(),
        "environment".to_string(),
    ];
    let args = Tools::quick_action_args(&names, Some("users/admin/list.http#2"));
    assert_eq!(args["id"], "users/admin/list.http#2");
    assert_eq!(args["folder"], "users/admin");
    assert!(!args.contains_key("environment"));
    assert!(Tools::quick_action_args(&names, None).is_empty());
}
