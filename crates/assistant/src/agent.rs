//! The chat loop: ask the model, run the tools it asks for through [`crate::tools`], give it
//! the results, until it answers.

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::provider;
use crate::settings::ProviderConfig;
use crate::tools::Tools;
use crate::types::{ChatMessage, ChatRequest, ChatRole, ToolCall, ToolSpec};

/// Model calls per question: plenty to organize a workspace, a stop for loops.
pub const MAX_STEPS: usize = 12;

const STOPPED: &str = "Stopped.";
/// How much of a tool's result its card shows; the model still gets all of it.
const EXCERPT_CHARS: usize = 400;

/// What the user is looking at, sent with every question so they don't have to explain it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentContext {
    /// Workspace folder name.
    pub workspace: String,
    pub environment: Option<String>,
    /// The open request's id, when the user attached "this request".
    pub request_id: Option<String>,
    /// A folder path, when the user attached "this folder".
    pub folder: Option<String>,
    /// The open request's latest history entry, when the user attached "last response".
    pub history_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AgentEvent {
    /// A piece of the answer text.
    Text { text: String },
    ToolStarted {
        id: String,
        name: String,
        summary: String,
    },
    ToolFinished {
        id: String,
        ok: bool,
        /// The start of the result, for the tool card.
        excerpt: String,
    },
    /// The turn is over; `messages` is the conversation to keep.
    Done { messages: Vec<ChatMessage> },
}

/// The system prompt: fixed instructions plus the workspace, identical for every turn of a
/// chat. Anything that changes per turn goes into [`turn_context`] instead, because rebuilding
/// the system prompt restarts the provider's prompt cache and invalidates Claude's earlier
/// thinking blocks.
pub fn system_prompt(context: &AgentContext) -> String {
    let mut prompt = String::from(
        "You are the assistant inside Plumbq, a desktop app for HTTP APIs. A workspace is a \
         folder of `.http` files (JetBrains / VS Code REST Client format): `### Name`, then \
         `METHOD url`, headers, a blank line and the body; `# @assert status == 200` lines add \
         checks; `{{name}}` are variables from the selected environment. Use the tools to look \
         at the real workspace before answering; never guess request ids or folder paths \
         (start with workspace_tree or requests_search). Prefer requests_preview before \
         requests_send. Secrets appear as `{{name}}`: never ask the user for secret values and \
         never put literal tokens in files; tell them to set secrets in the app. Before \
         changing files, say briefly what you'll change; changes and writes may need the \
         user's approval, so pass a short `reason`. If something is denied, don't retry it; \
         suggest an alternative. Each user message may start with a <context> block saying \
         what they were looking at when they wrote it. Answer concisely in Markdown, with \
         requests in ```http blocks.\n",
    );
    if !context.workspace.trim().is_empty() {
        prompt.push_str(&format!("\nWorkspace: {}\n", context.workspace));
    }
    prompt
}

/// What the user is looking at right now, sent with their message.
pub fn turn_context(context: &AgentContext) -> String {
    let mut out = String::new();
    let mut line = |label: &str, value: &str| {
        if !value.trim().is_empty() {
            out.push_str(&format!("- {label}: {value}\n"));
        }
    };
    line(
        "environment",
        context.environment.as_deref().unwrap_or("none selected"),
    );
    line(
        "this request (id)",
        context.request_id.as_deref().unwrap_or_default(),
    );
    line("this folder", context.folder.as_deref().unwrap_or_default());
    line(
        "last response (history id of this request)",
        context.history_id.as_deref().unwrap_or_default(),
    );
    out
}

/// One summary line for a tool call, shown on its card.
pub fn describe(call: &ToolCall) -> String {
    let args = call.args();
    [
        "id",
        "request_id",
        "folder",
        "path",
        "query",
        "url",
        "name",
        "environment",
    ]
    .iter()
    .find_map(|key| {
        args.get(key)
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
    })
    .unwrap_or_default()
    .to_string()
}

pub struct Agent<'a> {
    pub config: &'a ProviderConfig,
    pub key: Option<&'a str>,
    pub tools: &'a Tools,
}

impl Agent<'_> {
    /// Answers the last user message, adding every step to `messages` as it happens: when the
    /// turn fails halfway, `messages` still has the tool calls that ran. `emit` streams
    /// progress; `stop` ends the turn early, keeping what was said so far.
    pub async fn run(
        &self,
        context: &AgentContext,
        messages: &mut Vec<ChatMessage>,
        stop: &CancellationToken,
        emit: &mut (dyn FnMut(AgentEvent) + Send),
    ) -> Result<()> {
        let system = system_prompt(context);
        let specs = self.tools.specs().await?;
        // The context belongs to the message being answered and stays with it afterwards, so
        // earlier turns are never rewritten.
        if let Some(last) = messages.last_mut()
            && last.role == ChatRole::User
            && last.context.is_none()
        {
            last.context = Some(turn_context(context));
        }
        for _ in 0..MAX_STEPS {
            let Some(response) = self.ask(&system, messages, &specs, stop, emit).await? else {
                finish(messages, STOPPED, emit);
                return Ok(());
            };
            messages.push(ChatMessage {
                provider_blocks: response.provider_blocks.clone(),
                ..ChatMessage::assistant(response.text.clone(), response.tool_calls.clone())
            });
            if response.tool_calls.is_empty() {
                emit(AgentEvent::Done {
                    messages: messages.clone(),
                });
                return Ok(());
            }
            for call in &response.tool_calls {
                let Some((text, ok)) = self.run_tool(call, stop, emit).await else {
                    messages.push(ChatMessage::tool_result(call, "Stopped by the user.", true));
                    finish(messages, STOPPED, emit);
                    return Ok(());
                };
                messages.push(ChatMessage::tool_result(call, text, !ok));
            }
        }
        let note = format!("Stopped after {MAX_STEPS} steps. Ask something narrower, or continue.");
        emit(AgentEvent::Text { text: note.clone() });
        finish(messages, &note, emit);
        Ok(())
    }

    /// Runs one tool call and reports it on its card. Returns the result text and whether it
    /// worked; `None` when stopped.
    async fn run_tool(
        &self,
        call: &ToolCall,
        stop: &CancellationToken,
        emit: &mut (dyn FnMut(AgentEvent) + Send),
    ) -> Option<(String, bool)> {
        emit(AgentEvent::ToolStarted {
            id: call.id.clone(),
            name: call.name.clone(),
            summary: describe(call),
        });
        // Streamed tool input can be cut off or malformed; never run a guess.
        let valid =
            serde_json::from_str::<serde_json::Value>(&call.arguments).is_ok_and(|v| v.is_object());
        let outcome = if valid {
            self.tools.call(&call.name, call.args(), stop).await
        } else {
            Some((
                "the tool arguments were not valid JSON; call the tool again with a complete JSON object"
                    .to_string(),
                false,
            ))
        };
        let excerpt = match &outcome {
            Some((text, _)) => excerpt(text),
            None => "Stopped".into(),
        };
        emit(AgentEvent::ToolFinished {
            id: call.id.clone(),
            ok: outcome.as_ref().is_some_and(|(_, ok)| *ok),
            excerpt,
        });
        outcome
    }

    /// One model call; `None` when stopped.
    async fn ask(
        &self,
        system: &str,
        messages: &[ChatMessage],
        specs: &[ToolSpec],
        stop: &CancellationToken,
        emit: &mut (dyn FnMut(AgentEvent) + Send),
    ) -> Result<Option<crate::types::ChatResponse>> {
        let request = ChatRequest {
            model: &self.config.model,
            system,
            messages,
            tools: specs,
            temperature: self.config.temperature,
        };
        let mut on_text = |text: &str| {
            emit(AgentEvent::Text {
                text: text.to_string(),
            })
        };
        tokio::select! {
            response = provider::chat(self.config, self.key, &request, &mut on_text) => {
                response.map(Some)
            }
            () = stop.cancelled() => Ok(None),
        }
    }
}

/// Ends a turn that didn't finish normally with a short assistant note.
fn finish(messages: &mut Vec<ChatMessage>, note: &str, emit: &mut (dyn FnMut(AgentEvent) + Send)) {
    // Providers reject a conversation where a tool call has no result.
    let asked = messages.iter().rposition(|m| !m.tool_calls.is_empty());
    if let Some(index) = asked {
        let answered: Vec<String> = messages[index + 1..]
            .iter()
            .filter_map(|m| m.tool_call_id.clone())
            .collect();
        let missing: Vec<ToolCall> = messages[index]
            .tool_calls
            .iter()
            .filter(|c| !answered.contains(&c.id))
            .cloned()
            .collect();
        for call in &missing {
            messages.push(ChatMessage::tool_result(call, "Stopped by the user.", true));
        }
    }
    messages.push(ChatMessage::assistant(note, Vec::new()));
    emit(AgentEvent::Done {
        messages: messages.clone(),
    });
}

fn excerpt(text: &str) -> String {
    match text.char_indices().nth(EXCERPT_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}
