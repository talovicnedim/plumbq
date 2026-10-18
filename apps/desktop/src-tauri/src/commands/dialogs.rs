use tauri::{AppHandle, State};

use crate::dialogs::{self, OpenKind};
use crate::state::AppState;

/// Shows a native open dialog and allows later commands to use the chosen path. Returns it, or
/// `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn dialog_open(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: OpenKind,
) -> Result<Option<String>, ()> {
    let Some(path) = dialogs::open(&app, kind).await else {
        return Ok(None);
    };
    let text = path.display().to_string();
    state.grant(path);
    Ok(Some(text))
}
