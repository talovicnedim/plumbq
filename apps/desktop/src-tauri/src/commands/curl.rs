use plumbq_core::curl::{self, CurlSecrets};
use plumbq_core::http_file::Request;
use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize, specta::Type)]
pub struct CurlParsed {
    pub request: Request,
    pub secrets: CurlSecrets,
}

/// Parses a pasted cURL command without saving the request, so the editor can fill itself in.
/// Literal credentials come back as `{{secret}}` placeholders and are stored as secrets of
/// `environment`, if there is one.
#[tauri::command(async)]
#[specta::specta]
pub fn curl_parse(
    state: State<'_, AppState>,
    command: String,
    environment: Option<String>,
) -> Result<CurlParsed, AppError> {
    let pasted = curl::paste(&command)?;
    let secrets = curl::keep_secrets(&state.root()?, environment.as_deref(), &pasted)?;
    Ok(CurlParsed {
        request: pasted.request,
        secrets,
    })
}
