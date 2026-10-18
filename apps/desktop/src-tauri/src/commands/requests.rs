use std::collections::BTreeMap;
use std::path::PathBuf;

use plumbq_core::codegen::{self, CodeLanguage};
use plumbq_core::curl::CurlSecrets;
use plumbq_core::history::{self, HistoryEntry};
use plumbq_core::http_file::Request;
use plumbq_core::inspect::{self, RequestView};
use plumbq_core::requests::{IdChange, RequestMove};
use plumbq_core::{curl, request_meta, requests, send};
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, blocking};
use crate::state::AppState;

#[tauri::command(async)]
#[specta::specta]
pub fn requests_get(state: State<'_, AppState>, id: String) -> Result<Request, AppError> {
    Ok(requests::get(&state.root()?, &id)?)
}

#[tauri::command(async)]
#[specta::specta]
pub fn requests_create(
    state: State<'_, AppState>,
    folder: String,
    request: Request,
) -> Result<String, AppError> {
    Ok(requests::create(&state.root()?, &folder, request)?)
}

/// Saves a request. `expected` is the request's `source` as the tab read it: if the file
/// changed on disk since, this fails with kind `conflict` instead of overwriting another
/// request. Returns the request as saved, with its new `source`.
#[tauri::command(async)]
#[specta::specta]
pub fn requests_update(
    state: State<'_, AppState>,
    id: String,
    request: Request,
    expected: Option<String>,
) -> Result<Request, AppError> {
    Ok(requests::update(
        &state.root()?,
        &id,
        request,
        expected.as_deref(),
    )?)
}

/// Removes a request (see `requests_update` for `expected`). Returns the ids that changed, so
/// open tabs can follow.
#[tauri::command(async)]
#[specta::specta]
pub fn requests_remove(
    state: State<'_, AppState>,
    id: String,
    expected: Option<String>,
) -> Result<Vec<IdChange>, AppError> {
    let root = state.root()?;
    let changes = requests::remove(&root, &id, expected.as_deref())?;
    state.follow_history(&root, &changes);
    Ok(changes)
}

#[tauri::command(async)]
#[specta::specta]
pub fn requests_move(
    state: State<'_, AppState>,
    id: String,
    folder: String,
    position: Option<u32>,
    expected: Option<String>,
) -> Result<RequestMove, AppError> {
    let root = state.root()?;
    let moved = requests::move_to(&root, &id, &folder, position, expected.as_deref())?;
    state.follow_history(&root, &moved.changes);
    Ok(moved)
}

/// What the editor shows about an unsaved draft: variable hints, tags, docs, a file body and
/// body lines the `.http` format can't keep.
#[tauri::command]
#[specta::specta]
pub async fn requests_inspect(
    state: State<'_, AppState>,
    id: String,
    request: Request,
    environment: Option<String>,
) -> Result<RequestView, AppError> {
    let root = state.root()?;
    let env = environment;
    blocking(move || Ok(inspect::request(&root, &id, &request, env.as_deref())?)).await
}

/// The comment lines with one `# @tag` line for `tags`.
#[tauri::command]
#[specta::specta]
pub fn requests_tags_update(comments: Vec<String>, tags: Vec<String>) -> Vec<String> {
    request_meta::with_tags(&comments, &tags)
}

/// The comment lines with `docs` as the request's documentation.
#[tauri::command]
#[specta::specta]
pub fn requests_docs_update(comments: Vec<String>, docs: String) -> Vec<String> {
    request_meta::with_docs(&comments, &docs)
}

/// Names of the `{{$prompt name}}` variables to ask for before sending.
#[tauri::command(async)]
#[specta::specta]
pub fn requests_prompts(
    state: State<'_, AppState>,
    id: String,
    request: Request,
) -> Result<Vec<String>, AppError> {
    Ok(send::prompts_needed(&state.root()?, &id, &request)?)
}

/// Sends a (possibly unsaved) request and records it in history. Fails with kind
/// `needsInput` while prompt answers are missing, and with `needsConfirmation` for writes to
/// production until called again with `confirmed`.
#[tauri::command]
#[specta::specta]
pub async fn requests_send(
    state: State<'_, AppState>,
    id: String,
    request: Request,
    environment: Option<String>,
    prompts: BTreeMap<String, String>,
    confirmed: bool,
) -> Result<HistoryEntry, AppError> {
    let root = state.root()?;
    let env = environment.as_deref();
    let (number, cancelled) = state.start_send(&id);
    let context = state.send_context();
    let sending = send::send(&root, &id, &request, env, &prompts, &context, confirmed);
    // Dropping the send future aborts the HTTP request; a cancelled send never reaches history.
    let result = tokio::select! {
        result = sending => Some(result),
        _ = cancelled => None,
    };
    state.finish_send(&id, number);
    let response = result.ok_or_else(AppError::cancelled)??;
    tracing::info!("sent {} {id} ({})", request.method, response.status);
    let entry = history::entry(&id, env, response);
    record(state.history_dir().to_path_buf(), root, entry).await
}

/// Saves a send in history, off the async runtime because bodies can be big. A failed write is
/// only logged: the response arrived, and the user should see it.
async fn record(
    history_dir: PathBuf,
    root: PathBuf,
    entry: HistoryEntry,
) -> Result<HistoryEntry, AppError> {
    blocking(move || {
        if let Err(err) = history::save(&history_dir, &root, &entry) {
            tracing::warn!("failed to record history: {err}");
        }
        Ok(entry)
    })
    .await
}

/// "Copy as …": the request as code in `language`, with secrets left as `{{name}}`.
#[tauri::command]
#[specta::specta]
pub async fn requests_export(
    state: State<'_, AppState>,
    id: String,
    request: Request,
    environment: Option<String>,
    language: CodeLanguage,
) -> Result<String, AppError> {
    let root = state.root()?;
    let env = environment;
    blocking(move || {
        Ok(codegen::export(
            &root,
            &id,
            &request,
            env.as_deref(),
            language,
        )?)
    })
    .await
}

/// Cancels the in-flight send of request `id`; its `requests_send` fails with kind `cancelled`.
#[tauri::command]
#[specta::specta]
pub fn requests_cancel(state: State<'_, AppState>, id: String) -> bool {
    state.cancel_send(&id)
}

#[derive(Debug, Serialize, specta::Type)]
pub struct CurlImported {
    pub id: String,
    pub secrets: CurlSecrets,
}

/// Creates a request in `folder` from a pasted cURL command. Literal credentials become
/// `{{secret}}` placeholders and are stored as secrets of `environment`, if there is one.
#[tauri::command(async)]
#[specta::specta]
pub fn requests_import_curl(
    state: State<'_, AppState>,
    folder: String,
    name: String,
    command: String,
    environment: Option<String>,
) -> Result<CurlImported, AppError> {
    let root = state.root()?;
    let pasted = curl::paste(&command)?;
    let secrets = curl::keep_secrets(&root, environment.as_deref(), &pasted)?;
    let request = Request {
        name,
        ..pasted.request
    };
    Ok(CurlImported {
        id: requests::create(&root, &folder, request)?,
        secrets,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use plumbq_core::send::{Response, SentRequest};

    use super::*;

    fn response() -> Response {
        Response {
            status: 200,
            status_text: "OK".into(),
            headers: Vec::new(),
            body: "{}".into(),
            size: 2,
            binary: false,
            body_base64: String::new(),
            truncated: false,
            duration_ms: 12,
            sent: SentRequest {
                method: "GET".into(),
                url: "http://x.test/users".into(),
                headers: Vec::new(),
                body: String::new(),
            },
            assertions: Vec::new(),
            masked_secrets: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_failed_history_write_still_returns_the_response() {
        let blocked = std::env::temp_dir().join(format!(
            "ac-test-desktop-history-{}",
            &plumbq_mcp::settings::new_token()[..12]
        ));
        std::fs::write(&blocked, "a file where the history folder should be").unwrap();
        let entry = history::entry("users/list.http", None, response());
        let recorded = record(blocked, PathBuf::from("/ws"), entry).await.unwrap();
        assert_eq!(recorded.response.status, 200);
    }
}
