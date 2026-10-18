//! Google Gemini `streamGenerateContent`. The key goes in a header, never in the URL, so it
//! can't end up in logs of URLs.

use serde_json::{Value, json};

use crate::Result;
use crate::http;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ChatRole, ToolCall};

pub(crate) async fn chat(
    base_url: &str,
    key: &str,
    request: &ChatRequest<'_>,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<ChatResponse> {
    let url = format!(
        "{}/models/{}:streamGenerateContent?alt=sse",
        base_url.trim_end_matches('/'),
        request.model.trim_start_matches("models/")
    );
    let response = http::client()?
        .post(url)
        .header("x-goog-api-key", key)
        .json(&body(request))
        .send()
        .await
        .map_err(http::send_error)?;
    let response = http::check(response, key).await?;

    let mut out = ChatResponse::default();
    http::sse(response, |data| {
        let event: Value = serde_json::from_str(data)?;
        for part in event["candidates"][0]["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(text) = part["text"].as_str() {
                on_text(text);
                out.text.push_str(text);
            }
            if let Some(call) = part.get("functionCall") {
                let call = tool_call(call, out.tool_calls.len());
                out.tool_calls.push(call);
            }
        }
        Ok(())
    })
    .await?;
    Ok(out)
}

/// Gemini has no call ids; the name and position identify a call.
fn tool_call(call: &Value, position: usize) -> ToolCall {
    let name = call["name"].as_str().unwrap_or_default();
    ToolCall {
        id: format!("{name}-{position}"),
        name: name.to_string(),
        arguments: call
            .get("args")
            .cloned()
            .unwrap_or_else(|| json!({}))
            .to_string(),
    }
}

fn body(request: &ChatRequest<'_>) -> Value {
    let mut body = json!({
        "systemInstruction": {"parts": [{"text": request.system}]},
        "contents": contents(request.messages),
    });
    if !request.tools.is_empty() {
        let declarations: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                let mut d = json!({"name": t.name, "description": t.description});
                // Gemini rejects object schemas without properties.
                if t.input_schema["properties"]
                    .as_object()
                    .is_some_and(|p| !p.is_empty())
                {
                    d["parameters"] = t.input_schema.clone();
                }
                d
            })
            .collect();
        body["tools"] = json!([{"functionDeclarations": declarations}]);
    }
    if let Some(temperature) = request.temperature {
        body["generationConfig"] = json!({"temperature": temperature});
    }
    body
}

fn contents(list: &[ChatMessage]) -> Vec<Value> {
    list.iter()
        .map(|m| match m.role {
            ChatRole::User => json!({"role": "user", "parts": [{"text": m.user_text()}]}),
            ChatRole::Assistant => {
                let mut parts = Vec::new();
                if !m.content.is_empty() {
                    parts.push(json!({"text": m.content}));
                }
                for c in &m.tool_calls {
                    parts.push(json!({"functionCall": {"name": c.name, "args": c.args()}}));
                }
                json!({"role": "model", "parts": parts})
            }
            ChatRole::Tool => json!({"role": "user", "parts": [{"functionResponse": {
                "name": m.tool_name,
                "response": {"content": m.content, "error": m.is_error}
            }}]}),
        })
        .collect()
}

pub(crate) async fn models(base_url: &str, key: &str) -> Result<Vec<String>> {
    let response = http::client()?
        .get(format!("{}/models", base_url.trim_end_matches('/')))
        .header("x-goog-api-key", key)
        .send()
        .await
        .map_err(http::send_error)?;
    let body: Value = http::check(response, key)
        .await?
        .json()
        .await
        .map_err(http::send_error)?;
    Ok(body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            m["supportedGenerationMethods"]
                .as_array()
                .is_some_and(|methods| methods.iter().any(|x| x == "generateContent"))
        })
        .filter_map(|m| {
            m["name"]
                .as_str()
                .map(|n| n.trim_start_matches("models/").to_string())
        })
        .collect())
}
