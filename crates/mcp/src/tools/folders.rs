use plumbq_core::folders;
use plumbq_core::permissions::{Permission, PermissionContext};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::outline;
use crate::server::{Call, PlumbqMcp, error_text};
use crate::tools::requests::folder_label;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    /// Parent folder path; empty string for the workspace root.
    pub parent: String,
    /// Folder name; becomes a lowercase, dash-separated directory name.
    pub name: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenameParams {
    /// Folder path from workspace_tree.
    pub path: String,
    /// New folder name; it becomes a lowercase, dash-separated folder name.
    pub name: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PathParams {
    /// Folder path from workspace_tree.
    pub path: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetParams {
    /// Folder path from workspace_tree; empty string for the workspace root.
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReorderParams {
    /// Folder path; empty string for the workspace root.
    pub path: String,
    /// Child file and folder names (the last part of ids and paths, e.g. `list-users.http`,
    /// `admin`) in the wanted order. Unlisted children follow.
    pub order: Vec<String>,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = folders_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// A folder's settings from its folder.toml: shared headers, the order of its children
    /// and, for the workspace root, variable defaults and HTTP settings. Read it before
    /// changing headers.
    #[tool(annotations(title = "Read folder settings", read_only_hint = true))]
    async fn folders_get(&self, Parameters(p): Parameters<GetParams>) -> Result<String, String> {
        let root = self.root()?;
        let settings = folders::get_settings(&root, &p.path).map_err(error_text)?;
        Ok(outline::folder(&p.path, &settings))
    }

    /// Creates a folder. Returns its path.
    #[tool(annotations(
        title = "Create folder",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn folders_create(
        &self,
        Parameters(p): Parameters<CreateParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "folders_create",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("create folder {} in {}", p.name, folder_label(&p.parent)),
            detail: String::new(),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let path = folders::create(&root, &p.parent, &p.name).map_err(error_text)?;
        Ok(format!("Created folder {path}"))
    }

    /// Renames a folder. Ids of requests inside it change; call workspace_tree afterwards.
    #[tool(annotations(
        title = "Rename folder",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn folders_rename(
        &self,
        Parameters(p): Parameters<RenameParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "folders_rename",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("rename folder {} to {}", p.path, p.name),
            detail: "Request ids inside it change; their history moves along.".to_string(),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let renamed = folders::rename(&root, &p.path, &p.name).map_err(error_text)?;
        self.follow_history(&root, &renamed.changes);
        Ok(format!(
            "Renamed to {}; ids of the {} requests inside start with {}/ now",
            renamed.path,
            renamed.changes.len(),
            renamed.path
        ))
    }

    /// Copies a folder with all its requests, headers and examples to `<name>-copy`, e.g. as a
    /// starting point for a similar API version. Returns the copy's path.
    #[tool(annotations(
        title = "Duplicate folder",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn folders_duplicate(
        &self,
        Parameters(p): Parameters<PathParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "folders_duplicate",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("duplicate folder {}", p.path),
            detail: String::new(),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let path = folders::duplicate(&root, &p.path).map_err(error_text)?;
        Ok(format!("Duplicated to {path}"))
    }

    /// Removes an empty folder: one holding nothing but folder.toml files and other empty
    /// folders. Anything else (requests, body files, docs) is refused and listed; move it or
    /// ask the user. Never the workspace root.
    #[tool(annotations(
        title = "Remove empty folder",
        read_only_hint = false,
        destructive_hint = true
    ))]
    async fn folders_remove(
        &self,
        Parameters(p): Parameters<PathParams>,
    ) -> Result<String, String> {
        if p.path.trim_matches('/').is_empty() {
            return Err("the workspace root can't be removed".into());
        }
        let root = self.root()?;
        self.authorize(Call {
            tool: "folders_remove",
            permission: Permission::Destructive,
            context: PermissionContext::default(),
            action: format!("remove empty folder {}", p.path),
            detail: String::new(),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let content = folders::content(&root, &p.path).map_err(error_text)?;
        if !content.is_empty() {
            return Err(format!(
                "folder {} isn't empty; removing it would delete:\n{}",
                p.path,
                outline::capped_lines(&content, 30)
            ));
        }
        // Core refuses anything but organizing files too (non-recursive), as a second check.
        folders::remove(&root, &p.path, false).map_err(error_text)?;
        Ok(format!("Removed folder {}", p.path))
    }

    /// Sets the display order of a folder's children, e.g. to sort requests CRUD-style. Returns
    /// a short confirmation.
    #[tool(annotations(
        title = "Reorder folder",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true
    ))]
    async fn folders_reorder(
        &self,
        Parameters(p): Parameters<ReorderParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "folders_reorder",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("reorder {}", folder_label(&p.path)),
            detail: p.order.join("\n"),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        folders::reorder(&root, &p.path, p.order.clone()).map_err(error_text)?;
        Ok(format!(
            "Reordered {}",
            if p.path.is_empty() {
                "workspace root"
            } else {
                &p.path
            }
        ))
    }
}
