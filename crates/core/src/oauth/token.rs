//! Token endpoint calls: one POST with the client's id, secret and scope, and reading the
//! JSON answer (or the provider's error) into a [`Token`].

use std::path::Path;

use serde::Deserialize;

use super::{OAuthClient, Token};

/// What a token endpoint answered.
#[derive(Debug, PartialEq)]
pub(crate) enum TokenAnswer {
    Granted(Token),
    /// The provider understood the request and said no (an OAuth `error` code, e.g. an expired
    /// refresh token or a wrong secret). Holds the message for the user.
    Refused(String),
}

impl TokenAnswer {
    pub(crate) fn into_token(self) -> Result<Token> {
        match self {
            TokenAnswer::Granted(token) => Ok(token),
            TokenAnswer::Refused(message) => Err(Error::Http(message)),
        }
    }
}
use crate::environments::Values;
use crate::form_encoding::form_encode;
use crate::request_settings::RequestSettings;
use crate::{Error, Result, blocking, http_client, time, variables};

/// POSTs to the token endpoint with `form` plus the client's id, secret and scope. Uses the
/// workspace's proxy, CA and timeout settings, and never follows redirects (that would re-send
/// the client secret to wherever the endpoint points).
pub(crate) async fn request_token(
    root: &Path,
    client: &OAuthClient,
    values: &Values,
    form: &[(&str, &str)],
) -> Result<TokenAnswer> {
    let client_id = variables::substitute(&client.client_id, values)?;
    let client_secret = variables::substitute(&client.client_secret, values)?;
    let mut fields: Vec<(&str, &str)> = form.to_vec();
    fields.push(("client_id", &client_id));
    if !client_secret.is_empty() {
        fields.push(("client_secret", &client_secret));
    }
    if !client.scope.is_empty() && !form.iter().any(|(k, _)| *k == "code") {
        fields.push(("scope", &client.scope));
    }
    let body = form_encode(&fields);

    let http = {
        let root = root.to_path_buf();
        blocking::run(move || token_client(&root)).await?
    };
    let response = http
        .post(variables::substitute(&client.token_url, values)?)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|err| {
            Error::Http(format!(
                "failed to reach the token endpoint of {}: {}",
                client.name,
                crate::send::describe(err)
            ))
        })?;
    let status = response.status();
    let text = response.text().await.map_err(|err| {
        Error::Http(format!(
            "failed to read the token response: {}",
            crate::send::describe(err)
        ))
    })?;
    parse_token_response(&client.name, status.as_u16(), &text, time::now().0)
}

/// A refusal is an OAuth `error` answer below 500; anything else that isn't a token (no JSON, a
/// server error) is an error.
pub(crate) fn parse_token_response(
    name: &str,
    status: u16,
    text: &str,
    now: i64,
) -> Result<TokenAnswer> {
    #[derive(Deserialize)]
    struct Body {
        access_token: Option<String>,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
        error: Option<String>,
        error_description: Option<String>,
    }
    let body: Body = serde_json::from_str(text).map_err(|_| {
        Error::Http(format!(
            "token endpoint of {name} answered {status} without JSON; check token_url"
        ))
    })?;
    if let Some(access_token) = body.access_token.filter(|_| status < 400) {
        return Ok(TokenAnswer::Granted(Token {
            access_token,
            refresh_token: body.refresh_token,
            expires_at: body.expires_in.map(|secs| now.saturating_add(secs.max(0))),
        }));
    }
    let refused = body.error.is_some() && status < 500;
    let message = format!(
        "token request for {name} failed ({status}): {}",
        body.error_description
            .or(body.error)
            .unwrap_or_else(|| "no access_token in the response".into())
    );
    match refused {
        true => Ok(TokenAnswer::Refused(message)),
        false => Err(Error::Http(message)),
    }
}

/// Built from the workspace settings (the certificate password may come from the keychain),
/// without redirects: following one would re-send the client secret elsewhere.
fn token_client(root: &Path) -> Result<reqwest::Client> {
    let settings = http_client::workspace_settings(root)?;
    let mut options = http_client::options(root, &settings, &RequestSettings::default())?;
    options.follow_redirects = false;
    http_client::build(&options)
}
