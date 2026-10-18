use plumbq_core::folders::{self, RenamedFolder};
use plumbq_core::requests::IdChange;
use plumbq_core::workspace::FolderSettings;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
#[specta::specta]
pub fn folders_create(
    state: State<'_, AppState>,
    parent: String,
    name: String,
) -> Result<String, AppError> {
    Ok(folders::create(&state.root()?, &parent, &name)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn folders_rename(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<RenamedFolder, AppError> {
    let root = state.root()?;
    let renamed = folders::rename(&root, &path, &name)?;
    state.follow_history(&root, &renamed.changes);
    Ok(renamed)
}

/// Copies a folder next to itself. Returns the copy's path.
#[tauri::command(async)]
#[specta::specta]
pub fn folders_duplicate(state: State<'_, AppState>, path: String) -> Result<String, AppError> {
    Ok(folders::duplicate(&state.root()?, &path)?)
}

/// The UI confirms first and always removes recursively. Returns the removed request ids.
#[tauri::command(async)]
#[specta::specta]
pub fn folders_remove(state: State<'_, AppState>, path: String) -> Result<Vec<IdChange>, AppError> {
    let root = state.root()?;
    let changes = folders::remove(&root, &path, true)?;
    state.follow_history(&root, &changes);
    Ok(changes)
}

#[tauri::command(async)]
#[specta::specta]
pub fn folders_reorder(
    state: State<'_, AppState>,
    path: String,
    order: Vec<String>,
) -> Result<(), AppError> {
    Ok(folders::reorder(&state.root()?, &path, order)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn folders_get_settings(
    state: State<'_, AppState>,
    path: String,
) -> Result<FolderSettings, AppError> {
    Ok(folders::get_settings(&state.root()?, &path)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn folders_update_settings(
    state: State<'_, AppState>,
    path: String,
    settings: FolderSettings,
) -> Result<(), AppError> {
    Ok(folders::update_settings(&state.root()?, &path, settings)?)
}
