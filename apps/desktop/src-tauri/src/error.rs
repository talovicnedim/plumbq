use serde::Serialize;

/// The error shape every command returns to the UI: `{ kind, message }`.
#[derive(Debug, Serialize, specta::Type)]
pub struct AppError {
    pub kind: String,
    pub message: String,
}

impl AppError {
    /// The user cancelled a send; the UI shows "Cancelled", not an error.
    pub fn cancelled() -> Self {
        Self {
            kind: "cancelled".into(),
            message: "request cancelled".into(),
        }
    }

    pub fn no_workspace() -> Self {
        Self {
            kind: "noWorkspace".into(),
            message: "no workspace is open".into(),
        }
    }
}

/// Runs file and keychain work off the async runtime, so a big write or a keychain prompt never
/// stalls other commands.
pub async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| AppError {
            kind: "app".into(),
            message: format!("background work failed: {err}"),
        })?
}

impl From<plumbq_core::Error> for AppError {
    fn from(err: plumbq_core::Error) -> Self {
        Self {
            kind: err.kind().to_string(),
            message: err.to_string(),
        }
    }
}

impl From<plumbq_assistant::Error> for AppError {
    fn from(err: plumbq_assistant::Error) -> Self {
        Self {
            kind: err.kind().to_string(),
            message: err.to_string(),
        }
    }
}

impl From<plumbq_mcp::Error> for AppError {
    fn from(err: plumbq_mcp::Error) -> Self {
        Self {
            kind: err.kind().to_string(),
            message: err.to_string(),
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        Self {
            kind: "app".into(),
            message: err.to_string(),
        }
    }
}

impl From<notify::Error> for AppError {
    fn from(err: notify::Error) -> Self {
        Self {
            kind: "watch".into(),
            message: format!("failed to watch workspace: {err}"),
        }
    }
}
