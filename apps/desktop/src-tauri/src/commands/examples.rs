use plumbq_core::examples::{self, ResponseExample};
use plumbq_core::send::Response;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
#[specta::specta]
pub fn examples_list(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ResponseExample>, AppError> {
    Ok(examples::list(&state.root()?, &id)?)
}

/// Keeps a response as a named example next to the request (`<file>.examples.json`).
#[tauri::command(async)]
#[specta::specta]
pub fn examples_create(
    state: State<'_, AppState>,
    id: String,
    name: String,
    response: Response,
) -> Result<ResponseExample, AppError> {
    Ok(examples::create(&state.root()?, &id, &name, &response)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn examples_remove(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), AppError> {
    Ok(examples::remove(&state.root()?, &id, &name)?)
}
