use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::{curl, http_file, requests, text_diff};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::server::{Call, PlumbqMcp, error_text};
use crate::tools::requests::folder_label;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CurlParams {
    /// Folder path; empty string for the workspace root.
    pub folder: String,
    /// Name for the new request.
    pub name: String,
    /// The whole `curl …` command.
    pub command: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = import_curl_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Creates a request from a cURL command. Literal credentials (`-u`, `Authorization`,
    /// API-key headers) become `{{secret}}` placeholders the user sets in the app. Returns
    /// the new id.
    #[tool(annotations(
        title = "Import cURL",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn import_curl(&self, Parameters(p): Parameters<CurlParams>) -> Result<String, String> {
        let root = self.root()?;
        let imported = curl::import(&p.command).map_err(error_text)?;
        let request = plumbq_core::http_file::Request {
            name: p.name,
            ..imported.request
        };
        self.authorize(Call {
            tool: "import_curl",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!(
                "create request \"{}\" in {} from a cURL command",
                request.name,
                folder_label(&p.folder)
            ),
            detail: text_diff::unified(
                "(new file)",
                "",
                &http_file::serialize(std::slice::from_ref(&request)),
            ),
            environment: None,
            reason: p.reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        let id = requests::create(&root, &p.folder, request).map_err(error_text)?;
        if imported.secrets.is_empty() {
            return Ok(format!("Created {id}"));
        }
        Ok(format!(
            "Created {id}. Its credentials were replaced by the secrets {}; ask the user to set \
             them in an environment in the app.",
            imported.secrets.join(", ")
        ))
    }
}
