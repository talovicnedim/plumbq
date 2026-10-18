//! Browser sign-in for the OAuth 2.0 authorization code grant, with PKCE (RFC 7636): open the
//! provider's login page, catch the redirect on a localhost port, and swap the code for tokens.

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::environments::Values;
use crate::oauth::{self, OAuthClient, OAuthGrant};
use crate::{Error, Result, blocking, environments, form_encoding, variables};

/// How long to wait for the user to finish signing in.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
/// A connection that sends nothing (a browser preconnect) must not block the next one.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Signed in</title>\
<body style=\"font-family:system-ui;padding:3rem;text-align:center\">\
<h2>Signed in</h2><p>You can close this tab and go back to the app.</p>";

/// Runs the sign-in for client `name` of environment `env`. `open_url` opens the provider's
/// login page in the user's browser; the token ends up in the keychain.
pub async fn sign_in(
    root: &Path,
    env: &str,
    name: &str,
    open_url: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    let client = environments::auth_client(root, env, name)?;
    if client.grant != OAuthGrant::AuthorizationCode {
        return Err(Error::Invalid(format!(
            "{name} uses client credentials; it gets tokens without signing in"
        )));
    }
    let values = {
        let (root, env) = (root.to_path_buf(), env.to_string());
        blocking::run(move || oauth::values_for(&root, &env)).await?
    };
    let redirect = if client.redirect_url.is_empty() {
        oauth::DEFAULT_REDIRECT.to_string()
    } else {
        client.redirect_url.clone()
    };
    let verifier = random_string();
    let state = random_string();
    let login = login_url(&client, &values, &redirect, &state, &verifier)?;
    check_web_page(name, &login)?;
    let listener = listen(&redirect).await?;
    open_url(&login)?;

    let code = tokio::time::timeout(SIGN_IN_TIMEOUT, wait_for_code(&listener, &state))
        .await
        .map_err(|_| Error::Http(format!("sign-in to {name} timed out")))??;
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("code_verifier", verifier.as_str()),
    ];
    let token = oauth::request_token(root, &client, &values, &form)
        .await?
        .into_token()?;
    let (root, env, name) = (root.to_path_buf(), env.to_string(), name.to_string());
    blocking::run(move || oauth::store_token(&root, &env, &name, &token)).await
}

async fn listen(redirect: &str) -> Result<TcpListener> {
    TcpListener::bind(loopback_addr(redirect)?)
        .await
        .map_err(|err| {
            Error::Http(format!(
                "failed to listen for the sign-in redirect on {redirect}: {err}"
            ))
        })
}

/// The provider's login page, asking for a code sent back to `redirect`.
fn login_url(
    client: &OAuthClient,
    values: &Values,
    redirect: &str,
    state: &str,
    verifier: &str,
) -> Result<String> {
    let fields = [
        ("response_type", "code"),
        (
            "client_id",
            &variables::substitute(&client.client_id, values)?,
        ),
        ("redirect_uri", redirect),
        ("scope", &client.scope),
        ("state", state),
        ("code_challenge", &challenge(verifier)),
        ("code_challenge_method", "S256"),
    ];
    let auth_url = variables::substitute(&client.auth_url, values)?;
    let separator = if auth_url.contains('?') { '&' } else { '?' };
    Ok(format!(
        "{auth_url}{separator}{}",
        form_encoding::form_encode(&fields)
    ))
}

/// Only a web page may be opened: a cloned workspace could point `auth_url` at `file://` or a
/// custom scheme that starts an app.
fn check_web_page(name: &str, url: &str) -> Result<()> {
    if url.starts_with("https://") || url.starts_with("http://") {
        return Ok(());
    }
    Err(Error::Invalid(format!(
        "the authorization URL of {name} must start with https:// or http://"
    )))
}

/// `host:port` to listen on, from a `http://127.0.0.1:port/path` redirect URL. Only loopback
/// addresses are allowed: the app must never listen on the network.
fn loopback_addr(redirect: &str) -> Result<String> {
    let rest = redirect.strip_prefix("http://").ok_or_else(|| {
        Error::Invalid(format!("redirect url {redirect} must start with http://"))
    })?;
    let host_port = rest.split('/').next().unwrap_or_default();
    let (host, port) = host_port
        .rsplit_once(':')
        .ok_or_else(|| Error::Invalid(format!("redirect url {redirect} needs a port")))?;
    if !matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
        return Err(Error::Invalid(format!(
            "redirect url {redirect} must point at localhost"
        )));
    }
    let host = if host == "localhost" {
        "127.0.0.1"
    } else {
        host
    };
    Ok(format!("{host}:{port}"))
}

/// Accepts connections until one carries the authorization code for `state`. Requests without
/// the right state (favicons, other pages poking the port) are answered and ignored.
async fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String> {
    loop {
        let (mut socket, _) = listener.accept().await?;
        let Ok(request) = tokio::time::timeout(READ_TIMEOUT, read_head(&mut socket)).await else {
            continue;
        };
        let result = parse_callback(&request.unwrap_or_default(), state);
        let page = match &result {
            Ok(Some(_)) => DONE_PAGE.to_string(),
            Ok(None) => "<!doctype html><body>Waiting for sign-in…".to_string(),
            Err(err) => format!(
                "<!doctype html><body style=\"font-family:system-ui;padding:3rem\"><h2>Sign-in failed</h2><p>{}</p>",
                escape_html(&err.to_string())
            ),
        };
        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}",
            page.len()
        );
        // The browser closing early doesn't matter; the code is what counts.
        let _ = socket.write_all(reply.as_bytes()).await;
        if let Some(code) = result? {
            return Ok(code);
        }
    }
}

/// Reads until the end of the request head (or 8 KB): one `read` can return only part of it.
async fn read_head(socket: &mut tokio::net::TcpStream) -> std::io::Result<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 8192 {
        let n = socket.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// The code from a callback request line; `None` for anything without our `state`, so a page
/// that guesses the port can't abort the sign-in.
fn parse_callback(request: &str, state: &str) -> Result<Option<String>> {
    let target = request.split_whitespace().nth(1).unwrap_or_default();
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    let params = form_encoding::parse_query(query);
    if params.get("state").map(String::as_str) != Some(state) {
        return Ok(None);
    }
    if let Some(error) = params.get("error") {
        let detail = params.get("error_description").unwrap_or(error);
        return Err(Error::Http(format!("sign-in was refused: {detail}")));
    }
    Ok(params.get("code").cloned())
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 64 URL-safe random characters, from two v4 UUIDs.
fn random_string() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
