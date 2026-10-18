//! The OAuth token cache: one keychain entry per environment and client, holding the access
//! token, refresh token and expiry as JSON. Never in the workspace.

use std::path::Path;

use crate::oauth::Token;
use crate::{Result, secrets};

fn keychain_name(client: &str) -> String {
    format!("$auth:{client}")
}

pub(crate) fn cached_token(root: &Path, env: &str, client: &str) -> Result<Option<Token>> {
    let Some(json) = secrets::internal::get(root, env, &keychain_name(client))? else {
        return Ok(None);
    };
    // A corrupt cache entry is the same as no token: sign in or fetch again.
    Ok(serde_json::from_str(&json).ok())
}

pub(crate) fn store_token(root: &Path, env: &str, client: &str, token: &Token) -> Result<()> {
    secrets::internal::set(
        root,
        env,
        &keychain_name(client),
        &serde_json::to_string(token)?,
    )
}

/// Whether a token is cached for this client on this machine. A keychain that can't be read
/// (e.g. no secret service on Linux) counts as "not signed in" rather than breaking listings.
pub(crate) fn has_token(root: &Path, env: &str, client: &str) -> Result<bool> {
    match cached_token(root, env, client) {
        Ok(token) => Ok(token.is_some()),
        Err(err) => {
            tracing::warn!("failed to read the token cache for {client}: {err}");
            Ok(false)
        }
    }
}

/// Forgets the cached token; the next send fetches or asks for sign-in again.
pub fn sign_out(root: &Path, env: &str, client: &str) -> Result<()> {
    secrets::internal::remove(root, env, &keychain_name(client))
}
