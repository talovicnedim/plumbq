use std::collections::BTreeMap;
use std::path::Path;

use plumbq_core::folders::{self, FolderHeaderChange};
use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::text_diff;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::outline;
use crate::server::{Call, PlumbqMcp, error_text};
use crate::tools::requests::folder_label;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HeadersParams {
    /// Folder path; empty string for the workspace root.
    pub path: String,
    /// Headers to add or change, sent with every request in this folder and below, e.g.
    /// {"Accept": "application/json", "Authorization": "Bearer {{token}}"}.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Header names to remove.
    #[serde(default)]
    pub remove: Vec<String>,
    /// Replace all of the folder's shared headers with `headers` instead of merging.
    #[serde(default)]
    pub replace: bool,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = folders_headers_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Adds, changes or removes headers shared by every request in a folder (folder.toml); they
    /// merge unless `replace` is set, and credentials belong in `{{variables}}`, never literal
    /// tokens. Returns the folder's headers afterwards.
    #[tool(annotations(
        title = "Set folder headers",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn folders_set_headers(
        &self,
        Parameters(p): Parameters<HeadersParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let change = FolderHeaderChange {
            set: p.headers,
            remove: p.remove,
            replace: p.replace,
        };
        self.authorize(Call {
            tool: "folders_set_headers",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("change the shared headers of {}", folder_label(&p.path)),
            detail: headers_diff(&root, &p.path, &change)?,
            environment: None,
            reason: p.reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        let saved = folders::set_headers(&root, &p.path, &change).map_err(error_text)?;
        Ok(outline::headers_saved(&saved))
    }
}

/// The folder's shared headers before and after the change, as a diff for the approval dialog.
fn headers_diff(root: &Path, path: &str, change: &FolderHeaderChange) -> Result<String, String> {
    let current = folders::get_settings(root, path)
        .map_err(error_text)?
        .headers;
    let after = folders::merged_headers(current.clone(), change);
    let lines = |headers: &BTreeMap<String, String>| -> String {
        headers.iter().map(|(k, v)| format!("{k}: {v}\n")).collect()
    };
    Ok(text_diff::unified(
        "folder.toml headers",
        &lines(&current),
        &lines(&after),
    ))
}
