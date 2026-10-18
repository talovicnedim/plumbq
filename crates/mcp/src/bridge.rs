//! `plumbq-mcp --url <address>`: a stdio MCP server that forwards every call to the app's
//! in-app HTTP server. AI apps that only start local commands (Claude Desktop, Codex) use it,
//! so they need no Node.js bridge and every change still pops up for approval in the app, on
//! the workspace open there. The bridge decides nothing itself: tools, permissions and the
//! activity log are the app's.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ClientCapabilities, ClientConfig,
    GetPromptRequestParams, GetPromptResponse, Implementation, ListPromptsResult, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RunningService, ServiceError};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ErrorData, RoleClient, RoleServer, ServerHandler, ServiceExt};
use tokio::sync::Mutex;

type Remote = RunningService<RoleClient, ClientConfig>;

/// Forwards one stdio client to the app's HTTP server at `url`.
#[derive(Clone)]
pub struct Bridge {
    url: String,
    /// Connected on the first call, as the stdio client (so approvals and the activity log name
    /// the AI app, not the bridge), and again after the app restarted.
    remote: Arc<Mutex<Option<Arc<Remote>>>>,
}

impl Bridge {
    pub fn new(url: String) -> Self {
        Self {
            url,
            remote: Arc::default(),
        }
    }

    async fn remote(&self, context: &RequestContext<RoleServer>) -> Result<Arc<Remote>, ErrorData> {
        let mut slot = self.remote.lock().await;
        if let Some(remote) = slot.as_ref().filter(|r| !r.is_closed()) {
            return Ok(remote.clone());
        }
        let client = context
            .client_info()
            .unwrap_or_else(|| Implementation::new("plumbq-mcp", env!("CARGO_PKG_VERSION")));
        let transport = StreamableHttpClientTransport::from_uri(self.url.clone());
        let remote = ClientConfig::new(ClientCapabilities::default(), client)
            .serve(transport)
            .await
            .map_err(|err| {
                ErrorData::internal_error(
                    format!(
                        "can't reach Plumbq at its MCP address ({err}); ask the user to open \
                         the app and turn on the server (AI → AI apps)"
                    ),
                    None,
                )
            })?;
        let remote = Arc::new(remote);
        *slot = Some(remote.clone());
        Ok(remote)
    }

    /// Forgets a connection that broke (the app quit or restarted), so the next call reconnects.
    async fn forget_if_closed(&self, err: &ServiceError) {
        if matches!(
            err,
            ServiceError::TransportClosed | ServiceError::TransportSend(_)
        ) {
            *self.remote.lock().await = None;
        }
    }

    async fn failed(&self, err: ServiceError) -> ErrorData {
        self.forget_if_closed(&err).await;
        match err {
            ServiceError::McpError(err) => err,
            other => ErrorData::internal_error(format!("Plumbq didn't answer: {other}"), None),
        }
    }
}

impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("plumbq-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(crate::server::INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let remote = self.remote(&context).await?;
        match remote.list_tools(request).await {
            Ok(tools) => Ok(tools),
            Err(err) => Err(self.failed(err).await),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let remote = self.remote(&context).await?;
        match remote.call_tool_once(request).await {
            Ok(response) => Ok(response),
            Err(err) => Err(self.failed(err).await),
        }
    }

    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let remote = self.remote(&context).await?;
        match remote.list_prompts(request).await {
            Ok(prompts) => Ok(prompts),
            Err(err) => Err(self.failed(err).await),
        }
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        let remote = self.remote(&context).await?;
        match remote.get_prompt_once(request).await {
            Ok(prompt) => Ok(prompt),
            Err(err) => Err(self.failed(err).await),
        }
    }
}
