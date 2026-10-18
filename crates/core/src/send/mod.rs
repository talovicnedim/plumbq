//! Sends a request: resolves variables, adds folder headers, applies the production safety
//! rule, and returns the response with every secret value masked.

mod basic_auth;
mod mask;
mod prepare;
mod wire;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::assertions::{self, AssertionResult};
use crate::environments::{self, EnvironmentTag, Values};
use crate::http_file::{Header, Request};
use crate::{Error, Result, blocking, chain, cookies, dynamic, file_vars, oauth, variables};

pub(crate) use basic_auth::readable as basic_credentials;
use mask::{mask_error, mask_response};
pub use prepare::{effective_method, prepare, request_text};
pub use wire::body_bytes;
pub(crate) use wire::describe;
#[cfg(test)]
use wire::execute;
use wire::{Wire, execute_with, store_cookies, wire};

/// Larger bodies are cut off; the UI and agents can't do much with more anyway.
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
/// Methods that don't change data, so they never need confirmation on production.
pub const SAFE_METHODS: [&str; 3] = ["GET", "HEAD", "OPTIONS"];

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<Header>,
    pub body: String,
    /// Body size in bytes as received (before truncation).
    pub size: u32,
    /// The body isn't UTF-8 text; `body` is then empty and the bytes are in `body_base64`.
    #[serde(default)]
    pub binary: bool,
    /// Binary bodies (images, PDFs, …) for previews and "save to file". Empty for text. History
    /// drops it above [`crate::history::MAX_BINARY_BASE64`] to stay small.
    #[serde(default)]
    pub body_base64: String,
    #[serde(default)]
    pub truncated: bool,
    pub duration_ms: u32,
    /// What was actually sent, after variables and folder headers, with secrets masked.
    pub sent: SentRequest,
    /// Results of the request's `# @assert` lines. Missing in history from older versions.
    #[serde(default)]
    pub assertions: Vec<AssertionResult>,
    /// Secrets whose values `body` and `headers` show as `{{name}}`, so chaining puts back these
    /// and never a `{{name}}` the server wrote itself. A name the server also wrote is left
    /// out: the two can't be told apart.
    #[serde(default)]
    pub masked_secrets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SentRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<Header>,
    pub body: String,
}

/// Sends `request`, which may be an unsaved edit of the request at `id`; `id` decides which
/// folder headers apply. `prompts` answers the `{{$prompt name}}` variables (see
/// [`prompts_needed`]); missing answers fail with [`Error::NeedsInput`]. Writes to a production
/// environment fail with [`Error::NeedsConfirmation`] unless `confirmed` is set.
/// `context` says where per-machine data lives (see [`SendContext`]).
pub async fn send(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
    prompts: &BTreeMap<String, String>,
    context: &SendContext,
    confirmed: bool,
) -> Result<Response> {
    let mut values = {
        let (root, environment) = (root.to_path_buf(), environment.map(str::to_string));
        blocking::run(move || environments::values(&root, environment.as_deref())).await?
    };
    // First, before anything with side effects (an OAuth token fetch, reading files, prompts).
    check_production(root, id, request, &values, environment, confirmed)?;
    let overrides = apply_overrides(context, &mut values);
    let env = |name: &str| std::env::var(name).ok();
    let text = request_text(root, id, request)?;
    dynamic::resolve(&text, root, prompts, &env, &mut values)?;
    chain::resolve(
        &text,
        root,
        context.history_dir.as_deref(),
        environment,
        &mut values,
    )?;
    oauth::resolve(&text, root, environment, &mut values).await?;
    file_vars::resolve_into(root, id, request, &text, &overrides, &mut values)?;
    let short = variables::short_secrets(&values.secrets);
    if !short.is_empty() {
        tracing::warn!(
            "not masking secrets shorter than {} characters: {}",
            variables::MIN_MASK_LEN,
            short.join(", ")
        );
    }
    transmit(root, id, request, environment, context, &values)
        .await
        .map_err(|err| mask_error(err, &values))
}

fn check_production(
    root: &Path,
    id: &str,
    request: &Request,
    values: &Values,
    environment: Option<&str>,
    confirmed: bool,
) -> Result<()> {
    if values.tag != EnvironmentTag::Production || confirmed {
        return Ok(());
    }
    let method = effective_method(root, id, request)?;
    if SAFE_METHODS.contains(&method.as_str()) {
        return Ok(());
    }
    Err(Error::NeedsConfirmation(format!(
        "{method} on production environment {} needs confirmation",
        environment.unwrap_or_default()
    )))
}

/// Puts the context's values over the environment's. A secret stays a secret when a data row
/// or `--var` sets it, so it is still masked. Returns every name that was set.
fn apply_overrides(context: &SendContext, values: &mut Values) -> BTreeMap<String, String> {
    for (name, value) in &context.variables {
        let target = match values.secrets.contains_key(name) {
            true => &mut values.secrets,
            false => &mut values.variables,
        };
        target.insert(name.clone(), value.clone());
    }
    for (name, value) in &context.secrets {
        values.variables.remove(name);
        values.secrets.insert(name.clone(), value.clone());
    }
    let mut overrides = context.variables.clone();
    overrides.extend(context.secrets.clone());
    overrides
}

/// Sends the request with fully resolved `values`, and stores and masks what comes back.
async fn transmit(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
    context: &SendContext,
    values: &Values,
) -> Result<Response> {
    let jar_dir = context
        .cookie_dir
        .clone()
        .filter(|_| !request.settings.no_cookie_jar);
    let Wire {
        prepared,
        mut jar,
        body,
        client,
    } = {
        let (root, id, request) = (root.to_path_buf(), id.to_string(), request.clone());
        let (environment, jar_dir) = (environment.map(str::to_string), jar_dir.clone());
        let (values, agent) = (values.clone(), context.agent);
        blocking::run(move || {
            wire(
                &root,
                &id,
                &request,
                environment.as_deref(),
                jar_dir.as_deref(),
                &values,
                agent,
            )
        })
        .await?
    };
    let (response, final_url) = execute_with(&client, &prepared, body).await?;
    if let Some(dir) = &jar_dir {
        // Parsed against the URL that answered, so a redirect target can't set cookies for the
        // host that redirected to it.
        store_cookies(&mut jar, &final_url, &response.headers);
        cookies::save(dir, root, environment, &jar)?;
    }
    // Evaluated after masking, so assertion messages never contain secret values.
    let mut response = mask_response(response, values);
    response.assertions = assertions::run(&request.assertions, &response, values);
    Ok(response)
}

/// Where per-machine data lives. `None` turns the feature off for this send.
#[derive(Debug, Clone, Default)]
pub struct SendContext {
    /// Earlier responses, for `{{login.response…}}` references (see [`crate::chain`]).
    pub history_dir: Option<PathBuf>,
    /// Cookie jars (see [`crate::cookies`]).
    pub cookie_dir: Option<PathBuf>,
    /// Variables that win over the environment's, e.g. one row of a data file in a run
    /// (see [`crate::runner`]).
    pub variables: BTreeMap<String, String>,
    /// Like `variables`, but masked like secrets, e.g. the CLI's `--var` values whose name looks
    /// secret.
    pub secrets: BTreeMap<String, String>,
    /// Sent by an AI agent (MCP): redirects to another origin on this machine, the local
    /// network or a production host are not followed.
    pub agent: bool,
}

/// Names of the `{{$prompt name}}` variables the request uses, including in folder headers,
/// so a client can ask for all of them before sending.
pub fn prompts_needed(root: &Path, id: &str, request: &Request) -> Result<Vec<String>> {
    Ok(dynamic::prompts(&request_text(root, id, request)?))
}

#[cfg(test)]
mod binary_tests;

#[cfg(test)]
mod cookie_tests;

#[cfg(test)]
mod mask_tests;

#[cfg(test)]
mod production_tests;

#[cfg(test)]
mod redirect_tests;

#[cfg(test)]
mod tests;
