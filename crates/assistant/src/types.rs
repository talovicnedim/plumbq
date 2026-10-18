//! Provider-independent chat types. Each adapter converts these to its wire format and back,
//! so nothing outside `anthropic`, `openai` and `gemini` knows a provider's JSON.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ChatRole {
    User,
    Assistant,
    /// A tool's result, answering the assistant's tool call `tool_call_id`.
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// The arguments as JSON text, kept as text so the type crosses to the UI as is.
    pub arguments: String,
}

impl ToolCall {
    /// The arguments parsed; malformed JSON from a model becomes an empty object.
    pub fn args(&self) -> Value {
        serde_json::from_str(&self.arguments).unwrap_or_else(|_| Value::Object(Default::default()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    /// Tools the assistant asked to run (assistant messages).
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Which call this result answers, and its tool name (tool messages).
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    /// The tool failed or was refused.
    #[serde(default)]
    pub is_error: bool,
    /// What the user was looking at when they sent this (user messages). Sent to the model with
    /// the message, not in the system prompt, so the conversation prefix never changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Anthropic's own content blocks for this assistant turn (JSON), replayed verbatim: Claude's
    /// thinking blocks are signed and must come back unchanged, and the API itself ignores blocks
    /// a different Claude model can't use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_blocks: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(ChatRole::User, content)
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls,
            ..Self::new(ChatRole::Assistant, content)
        }
    }

    pub fn tool_result(call: &ToolCall, content: impl Into<String>, is_error: bool) -> Self {
        Self {
            tool_call_id: Some(call.id.clone()),
            tool_name: Some(call.name.clone()),
            is_error,
            ..Self::new(ChatRole::Tool, content)
        }
    }

    fn new(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            context: None,
            provider_blocks: None,
        }
    }

    /// The text a provider receives for a user message: its context, then what the user wrote.
    pub fn user_text(&self) -> String {
        match self.context.as_deref().filter(|c| !c.trim().is_empty()) {
            Some(context) => format!("<context>\n{context}</context>\n\n{}", self.content),
            None => self.content.clone(),
        }
    }
}

/// A tool the model may call: name, description and JSON Schema of its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub system: &'a str,
    pub messages: &'a [ChatMessage],
    pub tools: &'a [ToolSpec],
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    /// The provider's raw content blocks (JSON), for providers whose turns must be replayed
    /// exactly (Anthropic).
    pub provider_blocks: Option<String>,
}
