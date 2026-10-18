use std::path::PathBuf;

use plumbq_core::http_client::{self, HttpDefaults};
use plumbq_core::workspace::{self, TreeNode, WorkspaceSummary, WorkspaceWarning};
use plumbq_core::{client_pfx, demo, recent};
use tauri::{AppHandle, State};

use crate::error::{AppError, blocking};
use crate::state::AppState;

/// Opens a folder as the workspace and starts watching it. The folder must be a recent workspace
/// or chosen in a dialog.
#[tauri::command(async)]
#[specta::specta]
pub fn workspace_open(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspaceSummary, AppError> {
    let known = recent::list(state.recent_path())?
        .iter()
        .any(|w| w.root == path);
    let path = if known {
        PathBuf::from(&path)
    } else {
        state.granted(&path)?
    };
    let root = workspace::canonical_root(&path)?;
    workspace::tree(&root)?;
    let opened = workspace::summary(&root);
    state.open(&app, root)?;
    recent::add(state.recent_path(), &opened.root)?;
    Ok(opened)
}

/// Creates a new workspace folder `name` in `parent` (chosen in a dialog) and opens it.
#[tauri::command(async)]
#[specta::specta]
pub fn workspace_create(
    app: AppHandle,
    state: State<'_, AppState>,
    parent: String,
    name: String,
) -> Result<WorkspaceSummary, AppError> {
    let parent = state.granted(&parent)?;
    let root = workspace::canonical_root(&workspace::create(&parent, &name)?)?;
    let created = workspace::summary(&root);
    state.open(&app, root)?;
    recent::add(state.recent_path(), &created.root)?;
    Ok(created)
}

/// Creates (or reopens) the demo workspace in the app's data folder and opens it.
#[tauri::command(async)]
#[specta::specta]
pub fn workspace_open_demo(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspaceSummary, AppError> {
    let data_dir = state
        .recent_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| AppError {
            kind: "io".into(),
            message: "the app data folder is unavailable".into(),
        })?;
    let root = workspace::canonical_root(&demo::create(&data_dir)?)?;
    let opened = workspace::summary(&root);
    state.open(&app, root)?;
    recent::add(state.recent_path(), &opened.root)?;
    Ok(opened)
}

#[tauri::command(async)]
#[specta::specta]
pub fn workspace_current(state: State<'_, AppState>) -> Option<WorkspaceSummary> {
    state.root().ok().map(|root| workspace::summary(&root))
}

#[tauri::command(async)]
#[specta::specta]
pub fn workspace_recent(state: State<'_, AppState>) -> Result<Vec<WorkspaceSummary>, AppError> {
    Ok(recent::list(state.recent_path())?)
}

/// Sets the name shown for the open workspace; an empty name shows the folder's name again.
/// The folder itself keeps its name.
#[tauri::command(async)]
#[specta::specta]
pub fn workspace_rename(
    state: State<'_, AppState>,
    name: String,
) -> Result<WorkspaceSummary, AppError> {
    Ok(workspace::rename(&state.root()?, &name)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn workspace_tree(state: State<'_, AppState>) -> Result<Vec<TreeNode>, AppError> {
    Ok(workspace::tree(&state.root()?)?)
}

/// Files and folders the tree had to skip (not UTF-8, unreadable, a broken `folder.toml`), so
/// the sidebar can say why something is missing.
#[tauri::command(async)]
#[specta::specta]
pub fn workspace_warnings(state: State<'_, AppState>) -> Result<Vec<WorkspaceWarning>, AppError> {
    Ok(workspace::tree_with_warnings(&state.root()?)?.1)
}

/// Stores the password of the workspace's PFX client certificate in the OS keychain; empty
/// removes it. The password is never returned.
#[tauri::command]
#[specta::specta]
pub async fn workspace_set_pfx_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<(), AppError> {
    let root = state.root()?;
    blocking(move || Ok(client_pfx::set_password(&root, &password)?)).await
}

/// Whether a PFX password is stored for the open workspace.
#[tauri::command]
#[specta::specta]
pub async fn workspace_has_pfx_password(state: State<'_, AppState>) -> Result<bool, AppError> {
    let root = state.root()?;
    blocking(move || Ok(client_pfx::has_password(&root))).await
}

/// The values a send uses where no setting says otherwise, for the settings form's hints.
#[tauri::command]
#[specta::specta]
pub fn workspace_http_defaults() -> HttpDefaults {
    http_client::defaults()
}
