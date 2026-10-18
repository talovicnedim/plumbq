mod approval_text;
mod authorize;
mod call_log;
mod context;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, prompt_handler, tool_handler};

use crate::WriteLock;
use crate::approval::Approver;
use crate::settings::McpSettings;
pub(crate) use approval_text::shown;
pub(crate) use authorize::Call;
use call_log::CallRecord;
pub(crate) use call_log::{bodies_hidden, cancellation};
pub(crate) use context::check_version;

/// Tools that send requests but change no files. Offered in read-only mode, where they only
/// send GET, HEAD and OPTIONS. Every other tool is offered there only when it's annotated
/// `read_only_hint = true`, so a new write tool can't slip into read-only mode by accident.
const SEND_TOOLS: [&str; 2] = ["requests_send", "folders_run"];

pub(crate) const INSTRUCTIONS: &str = "Works on one Plumbq workspace: a folder of `.http` request files \
(JetBrains / VS Code REST Client format), folders, and environments with `{{variables}}`. \
Start with workspace_tree (or requests_search) to see request ids and folder paths. Requests are \
created and updated as `.http` text; requests_preview shows what a request would send without \
sending it. Folder order lives in folder.toml and is changed with folders_reorder, requests_move \
or requests_move_many. Secret values are never shown: they appear as `{{name}}` and must be set \
by the user in the app. Changes and requests that change data may need the user's approval in \
the Plumbq app: the call waits until they answer. Pass a short `reason` so they know why. \
If they deny something, don't retry unless they ask you to.";

/// The workspace tools work on: fixed for the stdio server, the app's open one inside the app.
#[derive(Debug, Clone)]
pub enum Workspace {
    Fixed(PathBuf),
    /// Updated by the app when the user opens another workspace; `None` when none is open.
    Shared(Arc<RwLock<Option<PathBuf>>>),
}

#[derive(Clone)]
pub struct PlumbqMcp {
    workspace: Workspace,
    /// Where the app keeps history, so sends from here show up there too.
    pub history_dir: Option<PathBuf>,
    /// Cookie jars shared with the app; `None` sends without cookies.
    pub cookie_dir: Option<PathBuf>,
    /// Besides the workspace, the only folder `import_file` may read from.
    pub import_dir: Option<PathBuf>,
    /// Where the activity log lives (the app data dir); `None` keeps no log.
    activity_dir: Option<PathBuf>,
    settings: Arc<RwLock<McpSettings>>,
    /// The stdio server reads the app's saved settings at the start of every call, so
    /// permission changes in the app apply from the next call.
    settings_file: Option<PathBuf>,
    /// The app's saved settings file, checked before every call in the app (see
    /// `call_settings`); the app's in-memory settings still apply.
    settings_check: Option<PathBuf>,
    /// Started with `--read-only`: write tools are not offered and nothing is asked.
    pub read_only: bool,
    /// Started with `--allow-env`: `$processEnv` works whatever the saved settings say.
    allow_process_env: bool,
    /// Tool output leaves response bodies out (status, headers and sizes only).
    hide_bodies: bool,
    /// Calls come from the built-in chat, not an AI app.
    from_chat: bool,
    approver: Approver,
    /// Tool calls run concurrently; changes to files go one at a time so read-modify-write of
    /// `folder.toml` and environment files can't lose updates.
    write_lock: WriteLock,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl PlumbqMcp {
    pub fn new(
        workspace: Workspace,
        settings: Arc<RwLock<McpSettings>>,
        approver: Approver,
    ) -> Self {
        let tool_router = Self::workspace_router()
            + Self::requests_router()
            + Self::requests_move_router()
            + Self::send_router()
            + Self::folders_router()
            + Self::folders_headers_router()
            + Self::environments_router()
            + Self::examples_router()
            + Self::history_router()
            + Self::import_router()
            + Self::import_curl_router()
            + Self::cookies_router()
            + Self::runner_router();
        Self {
            workspace,
            history_dir: None,
            cookie_dir: None,
            import_dir: None,
            activity_dir: None,
            settings,
            settings_file: None,
            settings_check: None,
            read_only: false,
            allow_process_env: false,
            hide_bodies: false,
            from_chat: false,
            approver,
            write_lock: WriteLock::default(),
            tool_router,
            prompt_router: Self::prompt_router(),
        }
    }

    /// Only reads and safe sends; write tools are not offered at all.
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        for tool in self.tool_router.list_all() {
            if !offered_read_only(&tool) {
                self.tool_router.remove_route(&tool.name);
            }
        }
        self
    }

    pub fn with_history_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.history_dir = dir;
        self
    }

    pub fn with_cookie_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.cookie_dir = dir;
        self
    }

    pub fn with_import_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.import_dir = dir;
        self
    }

    pub fn with_activity_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.activity_dir = dir;
        self
    }

    /// For clients that mustn't see response bodies, e.g. the built-in assistant when the user
    /// turned body sharing off.
    pub fn with_hidden_bodies(mut self, hide: bool) -> Self {
        self.hide_bodies = hide;
        self
    }

    /// Marks the built-in chat's calls in the activity log.
    pub fn for_chat(mut self) -> Self {
        self.from_chat = true;
        self
    }

    pub fn with_process_env(mut self, allow: bool) -> Self {
        self.allow_process_env = allow;
        self
    }

    pub fn with_settings_file(mut self, file: Option<PathBuf>) -> Self {
        self.settings_file = file;
        self
    }

    /// The app's lock, so its servers and chats change files one at a time.
    pub fn with_write_lock(mut self, lock: WriteLock) -> Self {
        self.write_lock = lock;
        self
    }

    pub fn with_settings_check(mut self, file: Option<PathBuf>) -> Self {
        self.settings_check = file;
        self
    }

    /// Tool names currently offered, for logging and tests.
    pub fn tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }
}

fn offered_read_only(tool: &rmcp::model::Tool) -> bool {
    let read_only = tool
        .annotations
        .as_ref()
        .and_then(|a| a.read_only_hint)
        .unwrap_or(false);
    read_only || SEND_TOOLS.contains(&tool.name.as_ref())
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for PlumbqMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("plumbq-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS)
    }

    /// Every call runs with a record of what it did, written to the activity log afterwards.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let tool = request.name.to_string();
        let actor = context
            .client_info()
            .map(|info| info.name)
            .unwrap_or_default();
        let settings = self.call_settings();
        let record = CallRecord {
            actor,
            hide_bodies: self.hide_bodies,
            cancel: context.ct.clone(),
            settings: settings.clone().ok().flatten(),
            ..CallRecord::default()
        };
        let call = ToolCallContext::new(self, request, context);
        let routed = async {
            if let Err(problem) = settings {
                return Ok(CallToolResponse::Complete(CallToolResult::error(vec![
                    ContentBlock::text(problem),
                ])));
            }
            self.tool_router.call(call).await
        };
        let (result, record) = call_log::recorded(record, routed).await;
        self.log_activity(&tool, record, &result);
        result
    }
}

/// Turns a core error into tool error text an agent can act on.
pub fn error_text(err: plumbq_core::Error) -> String {
    use plumbq_core::Error;
    match err {
        Error::NeedsConfirmation(message) => {
            format!("{message}; ask the user to approve it in the Plumbq app, or to send it there")
        }
        Error::NeedsInput(names) => format!(
            "this request asks for values when sent: {}; ask the user for them and pass them in \
             `prompts`",
            names.join(", ")
        ),
        Error::NeedsAuth(client) => format!(
            "the OAuth client {client} needs a browser sign-in first; ask the user to sign in \
             to {client} in the app (Environments), then try again"
        ),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;
