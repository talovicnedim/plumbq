use std::path::Path;

use plumbq_core::export::{docs, postman};
use plumbq_core::fs_util;
use tauri::{AppHandle, State};

use crate::dialogs;
use crate::error::{AppError, blocking};
use crate::state::AppState;

/// Asks where to save, then writes a folder (`""` for everything) as a Postman v2.1 collection
/// and each environment as a Postman environment file next to it. Returns the files written,
/// or `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn export_postman(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: String,
    file_name: String,
) -> Result<Option<Vec<String>>, AppError> {
    let root = state.root()?;
    let filter = ("Postman collection", &["json"][..]);
    let Some(path) = dialogs::save(&app, "Export to Postman", &file_name, filter).await else {
        return Ok(None);
    };
    expect_extension(&path, "json")?;
    blocking(move || {
        let collection = postman::collection(&root, &folder)?;
        let mut written = vec![write(&path, &pretty(&collection)?)?];
        let dir = path.parent().unwrap_or(Path::new("."));
        for (name, file) in postman::environments(&root)? {
            written.push(write(&dir.join(name), &pretty(&file)?)?);
        }
        Ok(Some(written))
    })
    .await
}

/// Asks where to save, then writes the workspace's HTML documentation there. Returns the path,
/// or `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn export_docs(
    app: AppHandle,
    state: State<'_, AppState>,
    environment: Option<String>,
    file_name: String,
) -> Result<Option<String>, AppError> {
    let root = state.root()?;
    let filter = ("HTML", &["html"][..]);
    let Some(path) = dialogs::save(&app, "Generate HTML docs", &file_name, filter).await else {
        return Ok(None);
    };
    expect_extension(&path, "html")?;
    blocking(move || {
        let html = docs::html(&root, environment.as_deref())?;
        Ok(Some(write(&path, &html)?))
    })
    .await
}

/// The dialog filters by extension, but users can type any name; keep to the expected kind.
fn expect_extension(path: &Path, ext: &str) -> Result<(), AppError> {
    match path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    {
        true => Ok(()),
        false => Err(AppError {
            kind: "invalid".into(),
            message: format!("save it as a .{ext} file"),
        }),
    }
}

fn pretty(value: &serde_json::Value) -> Result<String, AppError> {
    serde_json::to_string_pretty(value).map_err(|err| AppError {
        kind: "json".into(),
        message: err.to_string(),
    })
}

fn write(path: &Path, text: &str) -> Result<String, AppError> {
    fs_util::write_atomic(path, text).map_err(|err| AppError {
        kind: "io".into(),
        message: format!("failed to write {}: {err}", path.display()),
    })?;
    Ok(path.display().to_string())
}
