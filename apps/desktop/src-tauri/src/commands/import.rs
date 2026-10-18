use plumbq_core::import::{self, ImportSummary};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

/// Imports a Postman, OpenAPI, Swagger, Insomnia or HAR file, or a Bruno collection folder,
/// into `folder`. An earlier import of the same collection is updated instead of duplicated.
/// `path` must have been chosen in a dialog or dropped on the window.
#[tauri::command(async)]
#[specta::specta]
pub fn import_file(
    state: State<'_, AppState>,
    folder: String,
    path: String,
) -> Result<ImportSummary, AppError> {
    let path = state.granted(&path)?;
    let summary = import::file(&state.root()?, &folder, &path)?;
    log_summary(&summary);
    Ok(summary)
}

/// Downloads a spec from `url` and imports it like [`import_file`].
#[tauri::command]
#[specta::specta]
pub async fn import_url(
    state: State<'_, AppState>,
    folder: String,
    url: String,
) -> Result<ImportSummary, AppError> {
    let summary = import::url(&state.root()?, &folder, &url).await?;
    log_summary(&summary);
    Ok(summary)
}

fn log_summary(summary: &ImportSummary) {
    tracing::info!(
        "imported into {}: {} added, {} updated, {} stale",
        summary.folder,
        summary.added,
        summary.updated,
        summary.stale.len()
    );
}
