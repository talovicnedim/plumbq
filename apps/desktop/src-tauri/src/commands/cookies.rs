use plumbq_core::cookies::{self, Cookie};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

/// The cookie jar of the open workspace for `environment` (`None` = no environment).
#[tauri::command(async)]
#[specta::specta]
pub fn cookies_list(
    state: State<'_, AppState>,
    environment: Option<String>,
) -> Result<Vec<Cookie>, AppError> {
    let root = state.root()?;
    Ok(cookies::list(
        state.cookie_dir(),
        &root,
        environment.as_deref(),
    )?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn cookies_clear(
    state: State<'_, AppState>,
    environment: Option<String>,
) -> Result<(), AppError> {
    let root = state.root()?;
    Ok(cookies::clear(
        state.cookie_dir(),
        &root,
        environment.as_deref(),
    )?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn cookies_remove(
    state: State<'_, AppState>,
    environment: Option<String>,
    domain: String,
    name: String,
) -> Result<(), AppError> {
    let root = state.root()?;
    let env = environment.as_deref();
    Ok(cookies::remove(
        state.cookie_dir(),
        &root,
        env,
        &domain,
        &name,
    )?)
}
