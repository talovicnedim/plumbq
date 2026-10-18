use plumbq_core::json_path;

use crate::error::AppError;

/// Filters a JSON response body with a path like `$.items[*].name`; returns pretty JSON.
#[tauri::command(async)]
#[specta::specta]
pub fn json_path_query(body: String, path: String) -> Result<String, AppError> {
    let invalid = |message: String| AppError {
        kind: "invalid".into(),
        message,
    };
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| invalid("the body isn't JSON".into()))?;
    let found = json_path::query(&value, &path)
        .ok_or_else(|| invalid(format!("nothing matches {path}")))?;
    serde_json::to_string_pretty(&found).map_err(|err| invalid(err.to_string()))
}
