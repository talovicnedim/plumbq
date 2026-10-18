//! Anthropic Messages API, streamed. Caching, thinking blocks and refusals: see
//! `docs/formats.md` (Anthropic adapter).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::http;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ChatRole, ToolCall};
use crate::{Error, Result};

const VERSION: &str = "2023-06-01";
/// Streaming lifts HTTP timeouts, so give the model room; tool turns rarely come close.
const MAX_TOKENS: u32 = 64_000;
const OFFICIAL_HOST: &str = "api.anthropic.com";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Models that accept `fallbacks: "default"` on the Claude API.
const FALLBACK_MODELS: [&str; 4] = [
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];
const REFUSED: &str = "The model declined this request.";

pub(crate) async fn chat(
    base_url: &str,
    key: &str,
    request: &ChatRequest<'_>,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<ChatResponse> {
    let fallback = uses_fallback(base_url, request.model);
    let mut builder = http::client()?
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .header("x-api-key", key)
        .header("anthropic-version", VERSION);
    if fallback {
        builder = builder.header("anthropic-beta", FALLBACK_BETA);
    }
    let response = builder
        .json(&body(request, fallback))
        .send()
        .await
        .map_err(http::send_error)?;
    let response = http::check(response, key).await?;

    let mut turn = Turn::default();
    http::sse(response, |data| {
        let event: Value = serde_json::from_str(data)?;
        turn.apply(&event, on_text)
    })
    .await?;
    Ok(turn.finish(on_text))
}

fn uses_fallback(base_url: &str, model: &str) -> bool {
    base_url.contains(OFFICIAL_HOST) && FALLBACK_MODELS.contains(&model)
}

/// One streamed assistant turn: content blocks rebuilt from the events, in order.
#[derive(Default)]
struct Turn {
    blocks: BTreeMap<u64, Value>,
    /// Tool input JSON as it streams in, per block index.
    inputs: BTreeMap<u64, String>,
    text: String,
    stop_reason: Option<String>,
}

impl Turn {
    fn apply(&mut self, event: &Value, on_text: &mut (dyn FnMut(&str) + Send)) -> Result<()> {
        let index = event["index"].as_u64().unwrap_or(0);
        match event["type"].as_str() {
            Some("content_block_start") => {
                self.blocks.insert(index, event["content_block"].clone());
            }
            Some("content_block_delta") => self.delta(index, &event["delta"], on_text),
            Some("message_delta") => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(reason.to_string());
                }
            }
            Some("error") => {
                let message = event["error"]["message"]
                    .as_str()
                    .unwrap_or("unknown error");
                return Err(Error::Provider(format!("the provider stopped: {message}")));
            }
            _ => {}
        }
        Ok(())
    }

    fn delta(&mut self, index: u64, delta: &Value, on_text: &mut (dyn FnMut(&str) + Send)) {
        let Some(block) = self.blocks.get_mut(&index) else {
            return;
        };
        match delta["type"].as_str() {
            Some("text_delta") => {
                let text = delta["text"].as_str().unwrap_or_default();
                append(block, "text", text);
                on_text(text);
                self.text.push_str(text);
            }
            Some("thinking_delta") => append(
                block,
                "thinking",
                delta["thinking"].as_str().unwrap_or_default(),
            ),
            Some("signature_delta") => append(
                block,
                "signature",
                delta["signature"].as_str().unwrap_or_default(),
            ),
            Some("input_json_delta") => self
                .inputs
                .entry(index)
                .or_default()
                .push_str(delta["partial_json"].as_str().unwrap_or_default()),
            _ => {}
        }
    }

    fn finish(mut self, on_text: &mut (dyn FnMut(&str) + Send)) -> ChatResponse {
        if self.stop_reason.as_deref() == Some("refusal") {
            let note = if self.text.is_empty() {
                REFUSED.to_string()
            } else {
                format!("\n\n{REFUSED}")
            };
            on_text(&note);
            self.text.push_str(&note);
        }
        let mut tool_calls = Vec::new();
        for (index, block) in self.blocks.iter_mut() {
            if block["type"] != "tool_use" {
                continue;
            }
            let raw = self.inputs.remove(index).unwrap_or_default();
            let arguments = if raw.trim().is_empty() {
                "{}".to_string()
            } else {
                raw
            };
            // The replayed block needs an object; arguments that don't parse are reported to the
            // model by the agent instead of being run.
            block["input"] = serde_json::from_str::<Value>(&arguments)
                .ok()
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
            tool_calls.push(ToolCall {
                id: block["id"].as_str().unwrap_or_default().to_string(),
                name: block["name"].as_str().unwrap_or_default().to_string(),
                arguments,
            });
        }
        let blocks: Vec<Value> = self.blocks.into_values().collect();
        ChatResponse {
            text: self.text,
            tool_calls,
            provider_blocks: serde_json::to_string(&blocks).ok(),
        }
    }
}

fn append(block: &mut Value, field: &str, text: &str) {
    let current = block[field].as_str().unwrap_or_default();
    block[field] = Value::String(format!("{current}{text}"));
}

pub(crate) fn body(request: &ChatRequest<'_>, fallback: bool) -> Value {
    let mut body = json!({
        "model": request.model,
        "max_tokens": MAX_TOKENS,
        // Explicit breakpoint at the end of the static prefix (tools render before system).
        "system": [{"type": "text", "text": request.system, "cache_control": {"type": "ephemeral"}}],
        "messages": messages(request.messages),
        // Automatic breakpoint that follows the conversation's latest turn.
        "cache_control": {"type": "ephemeral"},
        "stream": true,
    });
    if !request.tools.is_empty() {
        let mut tools: Vec<_> = request.tools.iter().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        body["tools"] = tools
            .into_iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                    "eager_input_streaming": true,
                })
            })
            .collect();
    }
    if fallback {
        body["fallbacks"] = json!("default");
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    body
}

/// Anthropic wants tool results inside user messages, and consecutive tool results merged.
fn messages(list: &[ChatMessage]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for m in list {
        match m.role {
            ChatRole::User => out.push(json!({"role": "user", "content": m.user_text()})),
            ChatRole::Assistant => {
                out.push(json!({"role": "assistant", "content": assistant_blocks(m)}))
            }
            ChatRole::Tool => {
                let block = json!({"type": "tool_result", "tool_use_id": m.tool_call_id, "content": m.content, "is_error": m.is_error});
                match out.last_mut() {
                    Some(last) if last["role"] == "user" && last["content"].is_array() => {
                        if let Some(blocks) = last["content"].as_array_mut() {
                            blocks.push(block);
                        }
                    }
                    _ => out.push(json!({"role": "user", "content": [block]})),
                }
            }
        }
    }
    out
}

/// The turn exactly as Claude sent it when we have it (thinking blocks included), otherwise
/// rebuilt from text and tool calls (e.g. a turn written by another provider).
fn assistant_blocks(m: &ChatMessage) -> Value {
    if let Some(blocks) = m
        .provider_blocks
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .filter(|blocks| blocks.as_array().is_some_and(|b| !b.is_empty()))
    {
        return blocks;
    }
    let mut blocks = Vec::new();
    if !m.content.is_empty() {
        blocks.push(json!({"type": "text", "text": m.content}));
    }
    for c in &m.tool_calls {
        blocks.push(json!({"type": "tool_use", "id": c.id, "name": c.name, "input": c.args()}));
    }
    Value::Array(blocks)
}

pub(crate) async fn models(base_url: &str, key: &str) -> Result<Vec<String>> {
    let response = http::client()?
        .get(format!("{}/models", base_url.trim_end_matches('/')))
        .header("x-api-key", key)
        .header("anthropic-version", VERSION)
        .send()
        .await
        .map_err(http::send_error)?;
    let body: Value = http::check(response, key)
        .await?
        .json()
        .await
        .map_err(http::send_error)?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect())
}
