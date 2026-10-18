//! The chat's tools: the app's own MCP server, connected in-process, so permissions,
//! approvals and the activity log apply as for any AI app.

use plumbq_core::workspace;
use plumbq_mcp::{PlumbqMcp, prompts};
use rmcp::model::{
    CallToolRequest, CallToolRequestParams, ClientCapabilities, ClientConfig, ClientRequest,
    GetPromptRequestParams, Implementation, ServerResult,
};
use rmcp::service::{PeerRequestOptions, RunningService};
use rmcp::{RoleClient, ServiceExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::types::ToolSpec;
use crate::{Error, Result};

/// Tool output beyond this many characters is cut before it goes to the model.
const MAX_RESULT_CHARS: usize = 12_000;

/// A prompt template the server offers, shown as a quick action.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickAction {
    pub name: String,
    pub description: String,
    /// Argument names, required first.
    pub arguments: Vec<String>,
}

pub struct Tools {
    client: RunningService<RoleClient, ClientConfig>,
}

impl Tools {
    /// Starts `server` in this process and connects to it as `actor`.
    pub async fn connect(server: PlumbqMcp, actor: &str) -> Result<Self> {
        let (server_io, client_io) = tokio::io::duplex(256 * 1024);
        tokio::spawn(async move {
            match server.serve(server_io).await {
                Ok(running) => {
                    let _ = running.waiting().await;
                }
                Err(err) => tracing::warn!("assistant tools didn't start: {err}"),
            }
        });
        Self::client(client_io, actor).await
    }

    async fn client(io: tokio::io::DuplexStream, actor: &str) -> Result<Self> {
        let info = ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new(actor, env!("CARGO_PKG_VERSION")),
        );
        let client = info
            .serve(io)
            .await
            .map_err(|err| Error::Tools(format!("failed to start the assistant's tools: {err}")))?;
        Ok(Self { client })
    }

    /// Every tool the server offers, as provider-independent specs.
    pub async fn specs(&self) -> Result<Vec<ToolSpec>> {
        let tools = self.client.list_all_tools().await.map_err(tools_error)?;
        Ok(tools
            .into_iter()
            .map(|tool| ToolSpec {
                name: tool.name.to_string(),
                description: tool.description.map(|d| d.to_string()).unwrap_or_default(),
                input_schema: Value::Object((*tool.input_schema).clone()),
            })
            .collect())
    }

    /// Runs a tool; the text is what the model sees, `false` when it failed or was refused.
    /// `None` when `stop` fired first: the server is told to cancel the call, so its handler
    /// stops too instead of finishing the change.
    pub async fn call(
        &self,
        name: &str,
        args: Value,
        stop: &CancellationToken,
    ) -> Option<(String, bool)> {
        let Value::Object(arguments) = args else {
            return Some(("tool arguments must be a JSON object".into(), false));
        };
        let params = CallToolRequestParams::new(name.to_string()).with_arguments(arguments);
        let request = ClientRequest::CallToolRequest(CallToolRequest::new(params));
        let mut call = match self
            .client
            .send_cancellable_request(request, PeerRequestOptions::no_options())
            .await
        {
            Ok(call) => call,
            Err(err) => return Some((format!("the tool failed: {err}"), false)),
        };
        tokio::select! {
            response = &mut call.rx => Some(match response {
                Ok(Ok(ServerResult::CallToolResult(result))) => {
                    let text: String = result
                        .content
                        .iter()
                        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    (cap(&text), result.is_error != Some(true))
                }
                Ok(Ok(_)) => ("the tool gave an answer the chat can't read".into(), false),
                Ok(Err(err)) => (format!("the tool failed: {err}"), false),
                Err(_) => ("the tool failed: the connection closed".into(), false),
            }),
            () = stop.cancelled() => {
                let _ = call.cancel(Some("stopped by the user".into())).await;
                None
            }
        }
    }

    pub async fn quick_actions(&self) -> Result<Vec<QuickAction>> {
        let prompts = self.client.list_all_prompts().await.map_err(tools_error)?;
        Ok(prompts
            .into_iter()
            .map(|prompt| {
                let mut arguments = prompt.arguments.unwrap_or_default();
                arguments.sort_by_key(|a| !a.required.unwrap_or(false));
                QuickAction {
                    name: prompt.name,
                    description: prompt.description.unwrap_or_default(),
                    arguments: arguments.into_iter().map(|a| a.name).collect(),
                }
            })
            .collect())
    }

    /// A quick action's text, to send as the user's message.
    pub async fn quick_action(
        &self,
        name: &str,
        args: serde_json::Map<String, Value>,
    ) -> Result<String> {
        let params = GetPromptRequestParams::new(name).with_arguments(args);
        let result = self.client.get_prompt(params).await.map_err(tools_error)?;
        Ok(result
            .messages
            .iter()
            .filter_map(|m| m.content.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n\n"))
    }

    /// Arguments for a quick action from the open request: its id, or the folder it is in.
    pub fn quick_action_args(
        arguments: &[String],
        request_id: Option<&str>,
    ) -> serde_json::Map<String, Value> {
        let Some(id) = request_id else {
            return serde_json::Map::new();
        };
        arguments
            .iter()
            .filter_map(|name| {
                let value = match name.as_str() {
                    prompts::REQUEST_ARG => id,
                    prompts::FOLDER_ARG => workspace::split_rel(id).0,
                    _ => return None,
                };
                Some((name.clone(), Value::String(value.to_string())))
            })
            .collect()
    }

    pub async fn close(self) {
        let _ = self.client.cancel().await;
    }
}

fn tools_error(err: rmcp::ServiceError) -> Error {
    Error::Tools(format!("the assistant's tools failed: {err}"))
}

fn cap(text: &str) -> String {
    match text.char_indices().nth(MAX_RESULT_CHARS) {
        Some((cut, _)) => format!(
            "{}\n… (cut at {MAX_RESULT_CHARS} characters; ask for something narrower)",
            &text[..cut]
        ),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::Duration;

    use rmcp::model::{CallToolResponse, CallToolResult};
    use rmcp::service::RequestContext;
    use rmcp::{ErrorData, RoleServer, ServerHandler};
    use serde_json::json;
    use tokio::sync::mpsc;

    use super::*;

    /// A tool that runs until the client cancels it, and says so.
    #[derive(Clone)]
    struct Probe {
        started: mpsc::Sender<()>,
        cancelled: mpsc::Sender<()>,
    }

    impl ServerHandler for Probe {
        async fn call_tool(
            &self,
            _request: CallToolRequestParams,
            context: RequestContext<RoleServer>,
        ) -> std::result::Result<CallToolResponse, ErrorData> {
            let _ = self.started.send(()).await;
            context.ct.cancelled().await;
            let _ = self.cancelled.send(()).await;
            Ok(CallToolResponse::Complete(CallToolResult::success(vec![])))
        }
    }

    #[tokio::test]
    async fn stop_cancels_the_running_tool_call_on_the_server() {
        let (started, mut started_rx) = mpsc::channel(1);
        let (cancelled, mut cancelled_rx) = mpsc::channel(1);
        let (server_io, client_io) = tokio::io::duplex(64 * 1024);
        let probe = Probe { started, cancelled };
        tokio::spawn(async move {
            let running = probe.serve(server_io).await.unwrap();
            let _ = running.waiting().await;
        });
        let tools = Tools::client(client_io, "Chat · test").await.unwrap();

        let stop = CancellationToken::new();
        let stopping = stop.clone();
        tokio::spawn(async move {
            started_rx.recv().await;
            stopping.cancel();
        });
        assert_eq!(tools.call("wait", json!({}), &stop).await, None);
        tokio::time::timeout(Duration::from_secs(5), cancelled_rx.recv())
            .await
            .expect("the server saw the cancel");
    }
}
