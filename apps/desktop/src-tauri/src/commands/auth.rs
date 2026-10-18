use plumbq_core::Error;
use plumbq_core::http_file::Request;
use plumbq_core::oauth::{self, OAuthClient, flow};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, blocking};
use crate::state::AppState;

/// Signs in to an OAuth client in the browser (authorization code + PKCE). Resolves once the
/// token is in the keychain, or fails after five minutes.
#[tauri::command]
#[specta::specta]
pub async fn auth_sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    environment: String,
    name: String,
) -> Result<(), AppError> {
    let root = state.root()?;
    // Core only passes web pages (`http://`, `https://`).
    let open = |url: &str| {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|err| Error::Http(format!("failed to open the browser: {err}")))
    };
    Ok(flow::sign_in(&root, &environment, &name, open).await?)
}

/// The OAuth client to sign in to before `request` can be sent, if any (after a `needsAuth`).
#[tauri::command]
#[specta::specta]
pub async fn auth_sign_in_needed(
    state: State<'_, AppState>,
    id: String,
    request: Request,
    environment: Option<String>,
) -> Result<Option<String>, AppError> {
    let root = state.root()?;
    let env = environment;
    blocking(move || Ok(oauth::sign_in_needed(&root, &id, &request, env.as_deref())?)).await
}

/// A new OAuth client with the defaults filled in; not saved until its environment is.
#[tauri::command]
#[specta::specta]
pub fn auth_client_new(name: String) -> OAuthClient {
    OAuthClient::new(&name)
}

/// Forgets the cached token of an OAuth client (in the keychain).
#[tauri::command]
#[specta::specta]
pub async fn auth_sign_out(
    state: State<'_, AppState>,
    environment: String,
    name: String,
) -> Result<(), AppError> {
    let root = state.root()?;
    blocking(move || Ok(oauth::sign_out(&root, &environment, &name)?)).await
}
