//! Downloading a spec or collection to import, capped in size. An AI agent's download follows
//! only the redirects [`AgentRedirects`] allows.

use std::sync::OnceLock;
use std::time::Duration;

use super::{ImportedCollection, parse};
use crate::guard::AgentRedirects;
use crate::{Error, Result, send};

/// Specs bigger than this are almost certainly not an API description.
pub(super) const MAX_DOWNLOAD_BYTES: usize = 20 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 10;

/// Downloads and parses `url`. `redirects` is set for an AI agent: a redirect they don't allow
/// fails the download.
pub async fn download(url: &str, redirects: Option<AgentRedirects>) -> Result<ImportedCollection> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(Error::Invalid(format!("not an http(s) url: {url}")));
    }
    let client = match redirects {
        Some(rules) => agent_client(rules)?,
        None => shared_client()?,
    };
    let fetch_error = |err: reqwest::Error| {
        Error::Http(format!("failed to download {url}: {}", send::describe(err)))
    };
    let response = client.get(url).send().await.map_err(fetch_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Http(format!("failed to download {url}: {status}")));
    }
    let bytes = read_capped(response, url).await?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    crate::blocking::run(move || parse(&text)).await
}

/// The body, read piece by piece so a huge download stops at [`MAX_DOWNLOAD_BYTES`] instead of
/// filling memory first.
async fn read_capped(mut response: reqwest::Response, url: &str) -> Result<Vec<u8>> {
    let too_large = || Error::Invalid(format!("{url} is too large to import"));
    if response
        .content_length()
        .is_some_and(|len| len > MAX_DOWNLOAD_BYTES as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| Error::Http(format!("failed to download {url}: {err}")))?
    {
        if bytes.len() + chunk.len() > MAX_DOWNLOAD_BYTES {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Shared by every URL import the user starts, built on first use.
fn shared_client() -> Result<reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = builder().build().map_err(setup_error)?;
    Ok(CLIENT.get_or_init(|| client).clone())
}

/// A client for one agent download. Its redirect rules come from the workspace's environments
/// as they are now, so it can't be shared.
fn agent_client(rules: AgentRedirects) -> Result<reqwest::Client> {
    let resolver = rules.resolver();
    let hops = resolver.clone();
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        match attempt.previous().first() {
            Some(_) if !hops.follows(attempt.url()) => {
                let to = attempt
                    .url()
                    .host_str()
                    .unwrap_or("another host")
                    .to_string();
                attempt.error(format!(
                    "it redirects to {to}, which an AI agent may not reach without asking; ask \
                     the user to import it in the app"
                ))
            }
            _ => attempt.follow(),
        }
    });
    builder()
        .dns_resolver(resolver)
        .redirect(policy)
        .build()
        .map_err(setup_error)
}

fn builder() -> reqwest::ClientBuilder {
    crate::http_client::install_crypto();
    reqwest::Client::builder().timeout(TIMEOUT)
}

fn setup_error(err: reqwest::Error) -> Error {
    Error::Http(format!("failed to set up HTTP: {err}"))
}
