use plumbq_core::body_files::{self, BodyFile};
use tauri::{AppHandle, State};

use crate::dialogs;
use crate::error::{AppError, blocking};
use crate::state::AppState;

/// Asks for a file to send in request `id`'s body, starting in the workspace. Returns it
/// relative to the `.http` file, or `None` when cancelled; files outside the workspace are
/// refused.
#[tauri::command]
#[specta::specta]
pub async fn body_files_choose(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<BodyFile>, AppError> {
    let root = state.root()?;
    let Some(picked) = dialogs::open_body_file(&app, &root).await else {
        return Ok(None);
    };
    blocking(move || Ok(Some(body_files::reference(&root, &id, &picked)?))).await
}
