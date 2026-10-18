//! OAuth 2.0 tokens for `{{$auth.token("name")}}` (the JetBrains HTTP Client syntax; also
//! `{{$auth.token name}}`). Clients are configured per environment:
//!
//! ```toml
//! [auth.github]
//! grant = "authorization_code"   # or "client_credentials"
//! auth_url = "https://github.com/login/oauth/authorize"
//! token_url = "https://github.com/login/oauth/access_token"
//! client_id = "Iv1.abc"
//! client_secret = "{{githubSecret}}"   # a keychain secret, never a literal
//! scope = "read:user"
//! ```
//!
//! Tokens are cached in the OS keychain and refreshed when they expire. Client credentials are
//! fetched automatically; an authorization code grant needs the user to sign in once in the
//! browser (see [`flow`]), so sending fails with [`Error::NeedsAuth`] until then.

mod cache;
pub mod flow;
mod token;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::environments::{self, Values};
use crate::http_file::Request;
use crate::{Error, Result, blocking, send, time, variables};
use cache::cached_token;
pub use cache::sign_out;
pub(crate) use cache::{has_token, store_token};
pub(crate) use token::{TokenAnswer, request_token};

/// Where the browser sends the user back after signing in, unless the client sets its own.
pub(crate) const DEFAULT_REDIRECT: &str = "http://127.0.0.1:53682/callback";

/// Refresh a little before expiry so a token doesn't run out mid-request.
const EXPIRY_MARGIN_SECS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OAuthClient {
    pub name: String,
    pub grant: OAuthGrant,
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    /// Usually `{{someSecret}}`, resolved from the environment when a token is requested.
    pub client_secret: String,
    pub scope: String,
    /// Empty means `default_redirect_url`.
    pub redirect_url: String,
    /// Whether a token is cached on this machine. Read-only; ignored when saving.
    pub signed_in: bool,
    /// The redirect URL used when `redirect_url` is empty. Read-only; ignored when saving.
    pub default_redirect_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
// Matches the `grant = "authorization_code"` spelling of OAuth and the environment files.
#[serde(rename_all = "snake_case")]
pub enum OAuthGrant {
    AuthorizationCode,
    ClientCredentials,
}

/// The on-disk shape inside an environment file (`[auth.<name>]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OAuthClientFile {
    grant: OAuthGrant,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    auth_url: String,
    token_url: String,
    client_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    client_secret: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    scope: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    redirect_url: String,
}

/// A token as cached in the keychain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Token {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix seconds.
    pub expires_at: Option<i64>,
}

impl OAuthClient {
    /// A new authorization code client with nothing filled in yet; not saved.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            grant: OAuthGrant::AuthorizationCode,
            auth_url: String::new(),
            token_url: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            scope: String::new(),
            redirect_url: String::new(),
            signed_in: false,
            default_redirect_url: DEFAULT_REDIRECT.to_string(),
        }
    }

    pub(crate) fn from_file(name: &str, file: OAuthClientFile, signed_in: bool) -> Self {
        Self {
            name: name.to_string(),
            grant: file.grant,
            auth_url: file.auth_url,
            token_url: file.token_url,
            client_id: file.client_id,
            client_secret: file.client_secret,
            scope: file.scope,
            redirect_url: file.redirect_url,
            signed_in,
            default_redirect_url: DEFAULT_REDIRECT.to_string(),
        }
    }

    pub(crate) fn to_file(&self) -> OAuthClientFile {
        OAuthClientFile {
            grant: self.grant,
            auth_url: self.auth_url.clone(),
            token_url: self.token_url.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            scope: self.scope.clone(),
            redirect_url: self.redirect_url.clone(),
        }
    }
}

/// The client name in `$auth.token("github")` or `$auth.token github`.
pub fn client_name(variable: &str) -> Option<&str> {
    let rest = variable.strip_prefix("$auth.token")?;
    let name = rest
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .trim_matches(|c| c == '"' || c == '\'');
    (!name.is_empty()).then_some(name)
}

/// Adds a valid access token for every `$auth.token` variable in `text` to `values.secrets`,
/// so tokens are masked like any secret.
pub async fn resolve(
    text: &str,
    root: &Path,
    environment: Option<&str>,
    values: &mut Values,
) -> Result<()> {
    for variable in variables::used(text) {
        let Some(name) = client_name(&variable) else {
            continue;
        };
        let Some(env) = environment else {
            return Err(Error::Invalid(format!(
                "{{{{{variable}}}}} needs an environment with an OAuth client {name}"
            )));
        };
        let client = environments::auth_client(root, env, name)?;
        let token = valid_token(root, env, &client, values).await?;
        values.secrets.insert(variable, token);
    }
    Ok(())
}

/// The first OAuth client the request uses that needs a browser sign-in: an authorization code
/// client without a valid cached token. Sending fails with [`Error::NeedsAuth`] for it.
pub fn sign_in_needed(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<Option<String>> {
    let Some(env) = environment else {
        return Ok(None);
    };
    let text = send::request_text(root, id, request)?;
    for variable in variables::used(&text) {
        let Some(name) = client_name(&variable) else {
            continue;
        };
        let client = environments::auth_client(root, env, name)?;
        let token = match client.grant {
            OAuthGrant::AuthorizationCode => cached_token(root, env, &client.name)?,
            OAuthGrant::ClientCredentials => continue,
        };
        if needs_sign_in(token.as_ref(), time::now().0) {
            return Ok(Some(client.name));
        }
    }
    Ok(None)
}

/// An expired token counts too: its refresh may be rejected, which also asks for a sign-in.
fn needs_sign_in(token: Option<&Token>, now: i64) -> bool {
    token.is_none_or(|token| is_expired(token, now))
}

async fn valid_token(
    root: &Path,
    env: &str,
    client: &OAuthClient,
    values: &Values,
) -> Result<String> {
    let cached = {
        let (root, env, name) = (root.to_path_buf(), env.to_string(), client.name.clone());
        blocking::run(move || cached_token(&root, &env, &name)).await?
    };
    if let Some(token) = &cached
        && !is_expired(token, time::now().0)
    {
        return Ok(token.access_token.clone());
    }
    let token = renew(root, client, values, cached).await?;
    let access_token = token.access_token.clone();
    let (root, env, name) = (root.to_path_buf(), env.to_string(), client.name.clone());
    blocking::run(move || store_token(&root, &env, &name, &token)).await?;
    Ok(access_token)
}

/// A new token for a missing or expired one. Client credentials always fetch a fresh one, so a
/// stale refresh token can't get them stuck. An authorization code client refreshes, and needs
/// a sign-in when it has no refresh token or the provider refuses it; a network error stays one.
async fn renew(
    root: &Path,
    client: &OAuthClient,
    values: &Values,
    cached: Option<Token>,
) -> Result<Token> {
    if client.grant == OAuthGrant::ClientCredentials {
        let form = [("grant_type", "client_credentials")];
        return request_token(root, client, values, &form)
            .await?
            .into_token();
    }
    let Some(refresh) = cached.and_then(|token| token.refresh_token) else {
        return Err(Error::NeedsAuth(client.name.clone()));
    };
    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh.as_str()),
    ];
    match request_token(root, client, values, &form).await? {
        TokenAnswer::Granted(token) => Ok(keep_refresh_token(token, &refresh)),
        TokenAnswer::Refused(_) => Err(Error::NeedsAuth(client.name.clone())),
    }
}

/// Providers that don't rotate refresh tokens leave `refresh_token` out of a refresh response
/// (Google always does); keep the old one so the next expiry refreshes again.
fn keep_refresh_token(token: Token, old: &str) -> Token {
    Token {
        refresh_token: token.refresh_token.or_else(|| Some(old.to_string())),
        ..token
    }
}

pub(crate) fn is_expired(token: &Token, now: i64) -> bool {
    token
        .expires_at
        .is_some_and(|at| at.saturating_sub(EXPIRY_MARGIN_SECS) <= now)
}

/// The environment's values, for resolving `{{clientSecret}}`-style references.
pub(crate) fn values_for(root: &Path, env: &str) -> Result<Values> {
    environments::values(root, Some(env))
}

#[cfg(test)]
mod tests;
