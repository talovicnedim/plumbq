use plumbq_core::fs_util;
use plumbq_core::send::{self, Response};
use tauri::AppHandle;

use crate::dialogs;
use crate::error::{AppError, blocking};

/// Asks where to save a response body (suggesting `file_name`) and writes it there: the raw
/// bytes for binary bodies, UTF-8 text otherwise. Returns the path, or `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn responses_export(
    app: AppHandle,
    response: Response,
    file_name: String,
) -> Result<Option<String>, AppError> {
    let ext = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or("txt");
    let Some(path) =
        dialogs::save(&app, "Save response body", &file_name, ("Response", &[ext])).await
    else {
        return Ok(None);
    };
    blocking(move || {
        let bytes = send::body_bytes(&response)?;
        fs_util::write_private(&path, bytes).map_err(|err| AppError {
            kind: "io".into(),
            message: format!("failed to save {}: {err}", path.display()),
        })?;
        Ok(Some(path.display().to_string()))
    })
    .await
}
