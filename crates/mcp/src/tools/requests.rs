use plumbq_core::http_file::{self, Request};
use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::requests::IdChange;
use plumbq_core::{Error, requests, text_diff, workspace};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::server::{Call, PlumbqMcp, check_version, error_text};
use crate::{format, outline};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdParams {
    /// Request id from workspace_tree, e.g. `users/list-users.http`.
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveParams {
    /// Request id from workspace_tree.
    pub id: String,
    /// The `version` requests_get returned. When given, the delete is refused if the request
    /// changed since (so ids that shifted never delete the wrong request).
    pub version: Option<String>,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    /// Folder path from workspace_tree; empty string for the workspace root.
    pub folder: String,
    /// One request in `.http` format, e.g. "### List users\nGET {{baseUrl}}/users\nAccept: application/json".
    /// The `###` line is the request name; `# @assert status == 200` lines (before the request
    /// line) add assertions.
    pub content: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateParams {
    /// Request id from workspace_tree.
    pub id: String,
    /// The full new request in `.http` format (replaces the old one; the file name stays).
    pub content: String,
    /// The `version` requests_get returned. When given, the update is refused if the request
    /// changed on disk since you read it; read it again then.
    pub version: Option<String>,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// Words that must all appear in the name, URL, id or tags (case-insensitive). Empty
    /// matches everything.
    #[serde(default)]
    pub query: String,
    /// Only requests with this tag (`# @tag smoke`).
    pub tag: Option<String>,
    /// Only this method, e.g. `POST`.
    pub method: Option<String>,
}

/// Search results are cut after this many requests.
const MAX_RESULTS: usize = 100;

#[tool_router(router = requests_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// A request as `.http` text (method, URL, headers, body, assertions), with `{{variables}}`
    /// unresolved, after a `version:` line to pass to requests_update / move / remove. Very
    /// large bodies are cut.
    #[tool(annotations(title = "Read request", read_only_hint = true))]
    async fn requests_get(&self, Parameters(p): Parameters<IdParams>) -> Result<String, String> {
        let root = self.root()?;
        let request = requests::get(&root, &p.id).map_err(error_text)?;
        Ok(format!(
            "version: {}\n\n{}",
            requests::version(&request),
            format::cap(&http_file::serialize(&[request]))
        ))
    }

    /// Finds requests by words in their name, URL, id or tags, optionally by tag and method.
    /// Returns one line per request (id, method, name, URL), like workspace_tree. Use it
    /// instead of reading the whole tree of a big workspace.
    #[tool(annotations(title = "Search requests", read_only_hint = true))]
    async fn requests_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let nodes = workspace::tree(&root).map_err(error_text)?;
        let found = requests::search(&nodes, &p.query, p.tag.as_deref(), p.method.as_deref());
        Ok(outline::search_results(&found, MAX_RESULTS))
    }

    /// Creates a request in its own `.http` file in a folder, named after the request. Returns
    /// the new id.
    #[tool(annotations(
        title = "Create request",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn requests_create(
        &self,
        Parameters(p): Parameters<CreateParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let request = parse_one(&p.content)?;
        let new_text = http_file::serialize(std::slice::from_ref(&request));
        self.authorize(Call {
            tool: "requests_create",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!(
                "create request \"{}\" in {}",
                request.name,
                folder_label(&p.folder)
            ),
            detail: text_diff::unified("(new file)", "", &new_text),
            environment: None,
            reason: p.reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        let id = requests::create(&root, &p.folder, request).map_err(error_text)?;
        Ok(format!("Created {id}"))
    }

    /// Replaces a request with new `.http` text, e.g. to rename it, fix its URL or add headers
    /// or assertions. The old content is overwritten. Returns a short confirmation.
    #[tool(annotations(
        title = "Update request",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn requests_update(
        &self,
        Parameters(p): Parameters<UpdateParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let request = parse_one(&p.content)?;
        let old = requests::get(&root, &p.id).map_err(error_text)?;
        check_version(&p.id, p.version.as_deref(), &old)?;
        self.authorize(Call {
            tool: "requests_update",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("update request {}", p.id),
            detail: text_diff::unified(
                &p.id,
                &http_file::serialize(std::slice::from_ref(&old)),
                &http_file::serialize(std::slice::from_ref(&request)),
            ),
            environment: None,
            reason: p.reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        // The request must still be the one the user saw the diff of.
        requests::update(&root, &p.id, request, Some(&old.source)).map_err(error_text)?;
        Ok(format!("Updated {}", p.id))
    }

    /// Deletes a request file. Only when the user asked to delete it. Returns a short
    /// confirmation, with ids that shifted in the same file.
    #[tool(annotations(
        title = "Delete request",
        read_only_hint = false,
        destructive_hint = true
    ))]
    async fn requests_remove(
        &self,
        Parameters(p): Parameters<RemoveParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let old = requests::get(&root, &p.id).map_err(error_text)?;
        check_version(&p.id, p.version.as_deref(), &old)?;
        self.authorize(Call {
            tool: "requests_remove",
            permission: Permission::Destructive,
            context: PermissionContext::default(),
            action: format!("delete request {}", p.id),
            detail: text_diff::unified(
                &p.id,
                &http_file::serialize(std::slice::from_ref(&old)),
                "",
            ),
            environment: None,
            reason: p.reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        let changes = requests::remove(&root, &p.id, Some(&old.source)).map_err(error_text)?;
        self.follow_history(&root, &changes);
        Ok(format!("Deleted {}{}", p.id, shifted(&changes)))
    }
}

pub fn folder_label(folder: &str) -> String {
    match folder {
        "" => "the workspace root".to_string(),
        folder => format!("folder {folder}"),
    }
}

/// Tells the agent which other ids changed, e.g. `#3` becoming `#2` after a split.
pub(crate) fn shifted(changes: &[IdChange]) -> String {
    let others: Vec<String> = changes
        .iter()
        .skip(1)
        .filter_map(|c| c.to.as_ref().map(|to| format!("{} -> {to}", c.from)))
        .collect();
    match others.is_empty() {
        true => String::new(),
        false => format!("\nOther ids in that file changed: {}", others.join(", ")),
    }
}

pub fn parse_one(content: &str) -> Result<Request, String> {
    let mut requests = http_file::parse(content);
    match requests.len() {
        1 => Ok(requests.remove(0)),
        0 => Err(error_text(Error::Invalid(
            "content has no request line like `GET https://…`".into(),
        ))),
        n => Err(format!("content has {n} requests; send exactly one")),
    }
}
