//! One chat turn as the app runs it: the chat's tools on the workspace the turn started in, the
//! agent, and the saved chat, which keeps what happened even when the turn fails.

use std::path::Path;
use std::sync::{Arc, RwLock};

use plumbq_mcp::settings::is_hidden;
use plumbq_mcp::{HttpServer, PlumbqMcp};
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::agent::{Agent, AgentContext, AgentEvent};
use crate::chats::{self, Chat};
use crate::settings::{AssistantSettings, ProviderConfig};
use crate::tools::Tools;
use crate::types::ChatMessage;

/// What a turn runs with. Check `settings.chat_provider` before, so a turn never starts while
/// AI is off or without the user's consent.
pub struct Turn<'a> {
    /// The app's MCP server: the chat's tools go through its checks and approvals.
    pub server: HttpServer,
    /// The workspace open when the turn started. The tools act on it and the chat is saved
    /// under it, even if the user opens another workspace meanwhile.
    pub root: &'a Path,
    pub settings: &'a AssistantSettings,
    pub config: &'a ProviderConfig,
    pub key: Option<&'a str>,
}

/// Answers the last user message in `messages` and saves chat `id`. The saved chat has every
/// step that ran, also when the provider fails halfway: tool calls may already have changed
/// files.
pub async fn send(
    turn: Turn<'_>,
    id: String,
    mut messages: Vec<ChatMessage>,
    context: AgentContext,
    stop: &CancellationToken,
    emit: &mut (dyn FnMut(AgentEvent) + Send),
) -> Result<Chat> {
    let context = visible_context(context, &turn);
    let tools = Tools::connect(chat_server(&turn), &turn.config.actor()).await?;
    let agent = Agent {
        config: turn.config,
        key: turn.key,
        tools: &tools,
    };
    let result = agent.run(&context, &mut messages, stop, emit).await;
    tools.close().await;
    let chat = Chat {
        id,
        title: chats::title(&messages),
        updated_at: plumbq_core::time::now_rfc3339(),
        messages,
    };
    chats::save(&turn.server.data_dir, turn.root, &chat)?;
    result?;
    Ok(chat)
}

/// The app's server with the workspace fixed to the turn's.
fn chat_server(turn: &Turn<'_>) -> PlumbqMcp {
    let pinned = HttpServer {
        workspace: Arc::new(RwLock::new(Some(turn.root.to_path_buf()))),
        ..turn.server.clone()
    };
    plumbq_mcp::in_app(pinned)
        .with_hidden_bodies(!turn.settings.share_bodies)
        .for_chat()
}

/// The workspace's name, and the environment only when it isn't hidden from AI.
fn visible_context(mut context: AgentContext, turn: &Turn<'_>) -> AgentContext {
    let hidden = turn
        .server
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .hidden_environments
        .clone();
    context.environment = context.environment.filter(|env| !is_hidden(&hidden, env));
    context.workspace = turn
        .root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    context
}
