// Test helpers outside `#[test]` functions fail loudly too.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};

use super::{config, fake_provider, spec, sse};
use crate::settings::ProviderKind;
use crate::types::{ChatMessage, ChatRequest};
use crate::{anthropic, openai, provider};

#[tokio::test]
async fn openai_streams_text_and_sends_tool_schemas() {
    let (url, seen) = fake_provider(vec![sse(&[
        json!({"choices": [{"delta": {"content": "Hel"}}]}),
        json!({"choices": [{"delta": {"content": "lo"}}]}),
    ])])
    .await;
    let messages = [ChatMessage::user("hi")];
    let tools = [spec()];
    let request = ChatRequest {
        model: "llama3",
        system: "be brief",
        messages: &messages,
        tools: &tools,
        temperature: None,
    };
    let mut streamed = String::new();
    let config = config(ProviderKind::Ollama, &url, "llama3");
    let reply = provider::chat(&config, None, &request, &mut |t| streamed.push_str(t))
        .await
        .unwrap();

    assert_eq!(reply.text, "Hello");
    assert_eq!(streamed, "Hello");
    let body = &seen.lock().unwrap()[0];
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["tools"][0]["function"]["name"], "workspace_tree");
    assert_eq!(body, &openai::body(&request));
}

#[tokio::test]
async fn anthropic_assembles_streamed_tool_calls() {
    let (url, seen) = fake_provider(vec![sse(&[
        json!({"type": "message_start", "message": {"model": "claude-sonnet-5-5"}}),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "", "signature": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig-1"}}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Looking."}}),
        json!({"type": "content_block_start", "index": 2, "content_block": {"type": "tool_use", "id": "t1", "name": "requests_search", "input": {}}}),
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": "{\"query\":"}}),
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": "\"login\"}"}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}}),
    ])])
    .await;
    let call = crate::types::ToolCall {
        id: "t0".into(),
        name: "workspace_tree".into(),
        arguments: "{}".into(),
    };
    let messages = [
        ChatMessage::user("find login"),
        ChatMessage::assistant("", vec![call.clone()]),
        ChatMessage::tool_result(&call, "tree", false),
    ];
    let tools = [spec()];
    let request = ChatRequest {
        model: "claude-sonnet-5-5",
        system: "s",
        messages: &messages,
        tools: &tools,
        temperature: None,
    };
    let config = config(ProviderKind::Anthropic, &url, "claude-sonnet-5-5");
    let reply = provider::chat(&config, Some("sk-test"), &request, &mut |_| {})
        .await
        .unwrap();

    assert_eq!(reply.text, "Looking.");
    assert_eq!(reply.tool_calls[0].name, "requests_search");
    assert_eq!(reply.tool_calls[0].args()["query"], "login");
    let body = &seen.lock().unwrap()[0];
    assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    // Tool results go back inside a user message.
    assert_eq!(body["messages"][2]["role"], "user");
    assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(body, &anthropic::body(&request, false));
    // Caching: an explicit breakpoint on the system prompt plus automatic caching of the tail.
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(body["cache_control"]["type"], "ephemeral");
    assert_eq!(body["tools"][0]["eager_input_streaming"], true);

    // The turn is kept as Claude sent it, signed thinking block first, and replayed verbatim.
    let blocks: Value = serde_json::from_str(reply.provider_blocks.as_deref().unwrap()).unwrap();
    assert_eq!(blocks[0]["type"], "thinking");
    assert_eq!(blocks[0]["signature"], "sig-1");
    assert_eq!(blocks[2]["input"]["query"], "login");
    let next = [
        ChatMessage::user("find login"),
        ChatMessage {
            provider_blocks: reply.provider_blocks.clone(),
            ..ChatMessage::assistant(reply.text.clone(), reply.tool_calls.clone())
        },
    ];
    let replayed = anthropic::body(
        &ChatRequest {
            messages: &next,
            ..request
        },
        false,
    );
    assert_eq!(replayed["messages"][1]["content"], blocks);
}

#[test]
fn anthropic_prefix_stays_stable_and_context_rides_with_the_message() {
    let mut first = spec();
    first.name = "zeta".into();
    let mut second = spec();
    second.name = "alpha".into();
    let tools = [first, second];
    let mut user = ChatMessage::user("hi");
    user.context = Some("- this request (id): a.http\n".into());
    let messages = [user];
    let request = ChatRequest {
        model: "claude-sonnet-5-5",
        system: "s",
        messages: &messages,
        tools: &tools,
        temperature: None,
    };
    let body = anthropic::body(&request, true);
    assert_eq!(
        body["tools"][0]["name"], "alpha",
        "tools are sorted by name"
    );
    assert_eq!(body["fallbacks"], "default");
    let text = body["messages"][0]["content"].as_str().unwrap();
    assert!(text.starts_with("<context>\n- this request (id): a.http"));
    assert!(text.ends_with("hi"));
}

#[tokio::test]
async fn anthropic_reports_refusals() {
    let (url, _) = fake_provider(vec![sse(&[
        json!({"type": "message_start", "message": {"model": "claude-sonnet-5-5"}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "refusal"}}),
    ])])
    .await;
    let messages = [ChatMessage::user("hi")];
    let request = ChatRequest {
        model: "claude-sonnet-5-5",
        system: "s",
        messages: &messages,
        tools: &[],
        temperature: None,
    };
    let config = config(ProviderKind::Anthropic, &url, "claude-sonnet-5-5");
    let reply = provider::chat(&config, Some("sk-test"), &request, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(reply.text, "The model declined this request.");
}

#[tokio::test]
async fn cloud_providers_need_a_key_and_a_model() {
    let messages = [ChatMessage::user("hi")];
    let request = ChatRequest {
        model: "m",
        system: "s",
        messages: &messages,
        tools: &[],
        temperature: None,
    };
    let claude = config(
        ProviderKind::Anthropic,
        "http://127.0.0.1:9",
        "claude-sonnet-5-5",
    );
    let err = provider::chat(&claude, None, &request, &mut |_| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("needs an API key"));
    let no_model = config(ProviderKind::Ollama, "http://127.0.0.1:9", "");
    assert!(
        provider::chat(&no_model, None, &request, &mut |_| {})
            .await
            .is_err()
    );
}

#[test]
fn temperature_is_sent_only_when_set() {
    let messages = [ChatMessage::user("hi")];
    let mut request = ChatRequest {
        model: "m",
        system: "s",
        messages: &messages,
        tools: &[],
        temperature: None,
    };
    assert!(crate::openai::body(&request).get("temperature").is_none());
    assert!(
        crate::anthropic::body(&request, false)
            .get("temperature")
            .is_none()
    );
    request.temperature = Some(0.5);
    assert_eq!(crate::openai::body(&request)["temperature"], 0.5);
    assert_eq!(crate::anthropic::body(&request, false)["temperature"], 0.5);
}
