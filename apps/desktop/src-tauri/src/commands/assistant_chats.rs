//! Chat turns and the saved chats of the open workspace.

use std::sync::Arc;

use plumbq_assistant::agent::{AgentContext, AgentEvent};
use plumbq_assistant::chats::{self, Chat, ChatSummary};
use plumbq_assistant::settings;
use plumbq_assistant::turn::{self, Turn};
use plumbq_assistant::types::ChatMessage;
use tauri::State;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use super::assistant::provider_key;
use crate::assistant::AssistantState;
use crate::error::AppError;
use crate::mcp::McpState;
use crate::state::AppState;

/// Runs one turn: the assistant answers the last user message, streaming its progress through
/// `events`, and the chat is saved. Stop and "Turn AI off" end it early with what was said so
/// far.
#[tauri::command]
#[specta::specta]
pub async fn assistant_send(
    state: State<'_, AppState>,
    mcp: State<'_, Arc<McpState>>,
    assistant: State<'_, AssistantState>,
    chat_id: String,
    messages: Vec<ChatMessage>,
    context: AgentContext,
    events: Channel<AgentEvent>,
) -> Result<Chat, AppError> {
    // Registered before the checks, so turning AI off from now on stops this turn too.
    let (turn, stop) = assistant.start(&chat_id);
    let chat = send(&state, &mcp, &chat_id, messages, context, &events, &stop).await;
    assistant.finish(&chat_id, turn);
    chat
}

async fn send(
    state: &AppState,
    mcp: &McpState,
    chat_id: &str,
    messages: Vec<ChatMessage>,
    context: AgentContext,
    events: &Channel<AgentEvent>,
    stop: &CancellationToken,
) -> Result<Chat, AppError> {
    let root = state.root()?;
    let settings = settings::load(mcp.data_dir())?;
    let config = settings.chat_provider(&mcp.settings())?;
    let key = provider_key(config).await?;
    let mut emit = |event: AgentEvent| {
        if let Err(err) = events.send(event) {
            tracing::warn!("failed to stream an assistant event: {err}");
        }
    };
    let turn = Turn {
        server: mcp.http_server(),
        root: &root,
        settings: &settings,
        config,
        key: key.as_deref(),
    };
    let chat = turn::send(
        turn,
        chat_id.to_string(),
        messages,
        context,
        stop,
        &mut emit,
    );
    Ok(chat.await?)
}

#[tauri::command]
#[specta::specta]
pub fn assistant_stop(assistant: State<'_, AssistantState>, chat_id: String) {
    assistant.stop(&chat_id);
}

#[tauri::command(async)]
#[specta::specta]
pub fn assistant_chats_list(
    state: State<'_, AppState>,
    mcp: State<'_, Arc<McpState>>,
) -> Result<Vec<ChatSummary>, AppError> {
    Ok(chats::list(mcp.data_dir(), &state.root()?)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn assistant_chats_get(
    state: State<'_, AppState>,
    mcp: State<'_, Arc<McpState>>,
    id: String,
) -> Result<Chat, AppError> {
    Ok(chats::get(mcp.data_dir(), &state.root()?, &id)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn assistant_chats_remove(
    state: State<'_, AppState>,
    mcp: State<'_, Arc<McpState>>,
    id: String,
) -> Result<(), AppError> {
    Ok(chats::remove(mcp.data_dir(), &state.root()?, &id)?)
}
