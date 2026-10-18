use plumbq_core::history::{self, HistoryEntry, HistorySummary};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
#[specta::specta]
pub fn history_list(
    state: State<'_, AppState>,
    request_id: String,
) -> Result<Vec<HistorySummary>, AppError> {
    Ok(history::list(
        state.history_dir(),
        &state.root()?,
        &request_id,
    )?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn history_get(
    state: State<'_, AppState>,
    request_id: String,
    id: String,
) -> Result<HistoryEntry, AppError> {
    Ok(history::get(
        state.history_dir(),
        &state.root()?,
        &request_id,
        &id,
    )?)
}
