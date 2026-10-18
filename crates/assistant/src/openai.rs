//! OpenAI chat completions, which OpenAI, Ollama (`/v1`), LM Studio, OpenRouter and most
//! self-hosted servers speak.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::http;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ChatRole, ToolCall};
use crate::{Error, Result};

pub(crate) async fn chat(
    base_url: &str,
    key: Option<&str>,
    request: &ChatRequest<'_>,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<ChatResponse> {
    let mut builder = http::client()?.post(format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    ));
    if let Some(key) = key {
        builder = builder.bearer_auth(key);
    }
    let response = builder
        .json(&body(request))
        .send()
        .await
        .map_err(http::send_error)?;
    let response = http::check(response, key.unwrap_or_default()).await?;

    let mut out = ChatResponse::default();
    let mut calls = CallPieces::new();
    http::sse(response, |data| {
        let event: Value = serde_json::from_str(data)?;
        if let Some(message) = event["error"]["message"].as_str() {
            return Err(Error::Provider(format!("the provider stopped: {message}")));
        }
        let delta = &event["choices"][0]["delta"];
        if let Some(text) = delta["content"].as_str() {
            on_text(text);
            out.text.push_str(text);
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            add_call_piece(&mut calls, call);
        }
        Ok(())
    })
    .await?;
    out.tool_calls = finish_calls(calls);
    Ok(out)
}

/// Tool calls arrive in pieces, keyed by index: id, name and arguments so far.
type CallPieces = BTreeMap<u64, (String, String, String)>;

fn add_call_piece(calls: &mut CallPieces, call: &Value) {
    let entry = calls
        .entry(call["index"].as_u64().unwrap_or(0))
        .or_default();
    if let Some(id) = call["id"].as_str() {
        entry.0 = id.to_string();
    }
    if let Some(name) = call["function"]["name"].as_str() {
        entry.1.push_str(name);
    }
    if let Some(args) = call["function"]["arguments"].as_str() {
        entry.2.push_str(args);
    }
}

fn finish_calls(calls: CallPieces) -> Vec<ToolCall> {
    calls
        .into_values()
        .enumerate()
        .map(|(i, (id, name, args))| ToolCall {
            id: if id.is_empty() {
                format!("call_{i}")
            } else {
                id
            },
            name,
            arguments: if args.trim().is_empty() {
                "{}".into()
            } else {
                args
            },
        })
        .collect()
}

pub(crate) fn body(request: &ChatRequest<'_>) -> Value {
    let mut messages = vec![json!({"role": "system", "content": request.system})];
    messages.extend(request.messages.iter().map(message));
    let mut body = json!({"model": request.model, "messages": messages, "stream": true});
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.input_schema}}))
            .collect();
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    body
}

fn message(m: &ChatMessage) -> Value {
    match m.role {
        ChatRole::User => json!({"role": "user", "content": m.user_text()}),
        ChatRole::Assistant => {
            let mut v = json!({"role": "assistant", "content": m.content});
            if !m.tool_calls.is_empty() {
                v["tool_calls"] = m
                    .tool_calls
                    .iter()
                    .map(|c| json!({"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
                    .collect();
            }
            v
        }
        ChatRole::Tool => {
            json!({"role": "tool", "tool_call_id": m.tool_call_id, "content": m.content})
        }
    }
}

pub(crate) async fn models(base_url: &str, key: Option<&str>) -> Result<Vec<String>> {
    let mut builder = http::client()?.get(format!("{}/models", base_url.trim_end_matches('/')));
    if let Some(key) = key {
        builder = builder.bearer_auth(key);
    }
    let response = builder.send().await.map_err(http::send_error)?;
    let body: Value = http::check(response, key.unwrap_or_default())
        .await?
        .json()
        .await
        .map_err(http::send_error)?;
    let mut ids: Vec<String> = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect();
    ids.sort();
    Ok(ids)
}
