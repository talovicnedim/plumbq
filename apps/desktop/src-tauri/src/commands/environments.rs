use plumbq_core::environments::{self, Environment, EnvironmentTag};
use plumbq_core::workspace::WorkspaceWarning;
use tauri::State;

use crate::error::{AppError, blocking};
use crate::state::AppState;

/// Reads the keychain (which secrets are set), so it runs off the async runtime.
#[tauri::command]
#[specta::specta]
pub async fn environments_list(state: State<'_, AppState>) -> Result<Vec<Environment>, AppError> {
    let root = state.root()?;
    blocking(move || Ok(environments::list(&root)?)).await
}

/// Environment files left out of the list because of their name, so the UI can say why.
#[tauri::command(async)]
#[specta::specta]
pub fn environments_warnings(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceWarning>, AppError> {
    Ok(environments::warnings(&state.root()?)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn environments_create(
    state: State<'_, AppState>,
    name: String,
    tag: EnvironmentTag,
) -> Result<String, AppError> {
    Ok(environments::create(&state.root()?, &name, tag)?)
}

/// Signs out of OAuth clients the environment no longer has (keychain).
#[tauri::command]
#[specta::specta]
pub async fn environments_update(
    state: State<'_, AppState>,
    environment: Environment,
) -> Result<(), AppError> {
    let root = state.root()?;
    blocking(move || Ok(environments::update(&root, environment)?)).await
}

/// Removes the file, then its secrets and OAuth tokens from the keychain.
#[tauri::command]
#[specta::specta]
pub async fn environments_remove(state: State<'_, AppState>, name: String) -> Result<(), AppError> {
    let root = state.root()?;
    blocking(move || Ok(environments::remove(&root, &name)?)).await
}

/// Removes a secret from the environment file, then its value from the keychain. Saving an
/// environment never removes secrets; this is the only way.
#[tauri::command]
#[specta::specta]
pub async fn environments_remove_secret(
    state: State<'_, AppState>,
    environment: String,
    name: String,
) -> Result<(), AppError> {
    let root = state.root()?;
    blocking(move || Ok(environments::remove_secret(&root, &environment, &name)?)).await
}

/// Stores a secret value in the OS keychain. The value is never returned afterwards.
#[tauri::command]
#[specta::specta]
pub async fn environments_set_secret(
    state: State<'_, AppState>,
    environment: String,
    name: String,
    value: String,
) -> Result<(), AppError> {
    let root = state.root()?;
    blocking(move || {
        Ok(environments::set_secret(
            &root,
            &environment,
            &name,
            &value,
        )?)
    })
    .await
}
