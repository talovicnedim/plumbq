use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::{examples, history};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::format;
use crate::server::{Call, PlumbqMcp, error_text};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListParams {
    /// Request id from workspace_tree.
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    /// Request id from workspace_tree.
    pub id: String,
    /// History entry id from history_list: the response to keep.
    pub history_id: String,
    /// Example name, e.g. "200 OK" or "404 unknown user". An example with the same name is
    /// replaced.
    pub name: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = examples_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Saved example responses of a request (status, content type and body), kept as
    /// documentation. Use them to learn a response's shape without sending the request.
    #[tool(annotations(title = "Response examples", read_only_hint = true))]
    async fn examples_list(&self, Parameters(p): Parameters<ListParams>) -> Result<String, String> {
        let root = self.root()?;
        let list = examples::list(&root, &p.id).map_err(error_text)?;
        Ok(format::examples(&list))
    }

    /// Keeps a past response (from history_list) as a named example next to the request
    /// (`<file>.examples.json`, committed with it), e.g. to document success and error cases.
    /// An example with the same name is replaced. Returns the saved name and status.
    #[tool(annotations(
        title = "Create example",
        read_only_hint = false,
        destructive_hint = true
    ))]
    async fn examples_create(
        &self,
        Parameters(p): Parameters<CreateParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let Some(dir) = &self.history_dir else {
            return Err("history is unavailable on this machine".into());
        };
        let entry = history::get(dir, &root, &p.id, &p.history_id).map_err(error_text)?;
        self.authorize(Call {
            tool: "examples_create",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!(
                "save the {} response of {} as example \"{}\"",
                entry.response.status, p.id, p.name
            ),
            detail: format::cap(&entry.response.body),
            environment: entry.environment.clone(),
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let saved = examples::create(&root, &p.id, &p.name, &entry.response).map_err(error_text)?;
        Ok(format!(
            "Saved example \"{}\" ({})",
            saved.name, saved.status
        ))
    }
}
