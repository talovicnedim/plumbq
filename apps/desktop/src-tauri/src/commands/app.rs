use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::{LOG_FILE, QUIT_CONFIRMED};

/// Quits the app; the UI calls it after checking for unsaved tabs.
#[tauri::command]
#[specta::specta]
pub fn app_quit(app: AppHandle) {
    QUIT_CONFIRMED.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// Where the app writes its log, for "Show log file".
#[tauri::command]
#[specta::specta]
pub fn app_log_file(app: AppHandle) -> Result<String, AppError> {
    let dir = app.path().app_log_dir()?;
    Ok(dir.join(format!("{LOG_FILE}.log")).display().to_string())
}
