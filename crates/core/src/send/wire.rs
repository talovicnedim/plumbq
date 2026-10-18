//! The HTTP exchange of a send: the request on the wire, the body read with a cap, and the
//! cookie jar around it.

use std::path::Path;
use std::time::Instant;

use base64::Engine;
use reqwest::Url;

use super::mask::cross_origin_redirects;
use super::{MAX_BODY_BYTES, Response, SentRequest, basic_auth, prepare};
use crate::environments::Values;
use crate::guard::AgentRedirects;
use crate::http_client;
use crate::http_file::{Header, Request};
use crate::{Error, Result, body_files, cookies, multipart, time};

/// Everything needed to put the request on the wire.
pub(super) struct Wire {
    pub prepared: SentRequest,
    pub jar: Vec<cookies::Cookie>,
    pub body: Vec<u8>,
    pub client: reqwest::Client,
}

/// The blocking part of a send: body files, the cookie jar, and a client whose certificate
/// password may come from the keychain.
pub(super) fn wire(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
    jar_dir: Option<&Path>,
    values: &Values,
    agent: bool,
) -> Result<Wire> {
    let mut prepared = prepare(root, id, request, values)?;
    let jar = match jar_dir {
        Some(dir) => cookies::list(dir, root, environment)?,
        None => Vec::new(),
    };
    add_cookie_header(&mut prepared, &jar);
    let body = body_files::load(
        root,
        id,
        &request.body,
        multipart::content_type(&prepared.headers),
        values,
    )?;
    prepared.body = body.description;
    let settings = http_client::workspace_settings(root)?;
    let mut options = http_client::options(root, &settings, &request.settings)?;
    options.cross_origin_redirects = cross_origin_redirects(&prepared.headers, &body.bytes, values);
    if agent {
        options.agent_redirects = Some(AgentRedirects::for_sends(root, &prepared.url)?);
    }
    // After the secret check: the encoded value no longer shows the secret.
    basic_auth::encode(&mut prepared.headers);
    Ok(Wire {
        prepared,
        jar,
        body: body.bytes,
        client: http_client::build(&options)?,
    })
}

/// Adds jar cookies for the URL. A `Cookie` header written in the request wins: jar cookies
/// with the same name are left out, the others are appended to it.
pub(super) fn add_cookie_header(sent: &mut SentRequest, jar: &[cookies::Cookie]) {
    let Ok(url) = Url::parse(&sent.url) else {
        return;
    };
    let Some(from_jar) = cookies::header_for(jar, &url, time::now().0) else {
        return;
    };
    let Some(existing) = sent
        .headers
        .iter_mut()
        .find(|h| h.name.eq_ignore_ascii_case("cookie"))
    else {
        sent.headers.push(Header {
            name: "Cookie".into(),
            value: from_jar,
        });
        return;
    };
    let explicit: Vec<&str> = existing
        .value
        .split(';')
        .filter_map(|pair| pair.split_once('=').map(|(name, _)| name.trim()))
        .collect();
    let extra: Vec<&str> = from_jar
        .split("; ")
        .filter(|pair| {
            !explicit
                .iter()
                .any(|name| pair.starts_with(&format!("{name}=")))
        })
        .collect();
    if !extra.is_empty() {
        existing.value = format!(
            "{}; {}",
            existing.value.trim_end_matches(';'),
            extra.join("; ")
        );
    }
}

pub(super) fn store_cookies(jar: &mut Vec<cookies::Cookie>, url: &str, headers: &[Header]) {
    let Ok(url) = Url::parse(url) else { return };
    let now = time::now().0;
    for header in headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("set-cookie"))
    {
        if let Some(cookie) = cookies::parse(&header.value, &url, now) {
            cookies::store(jar, cookie, now);
        }
    }
}

/// Sends with a default client; tests and callers without workspace settings.
#[cfg(test)]
pub(super) async fn execute(sent: &SentRequest) -> Result<Response> {
    let options = http_client::options(
        Path::new(""),
        &http_client::HttpSettings::default(),
        &crate::request_settings::RequestSettings::default(),
    )?;
    let (response, _) = execute_with(
        &http_client::build(&options)?,
        sent,
        sent.body.as_bytes().to_vec(),
    )
    .await?;
    Ok(response)
}

/// Sends `sent` with `body` (the raw bytes: file bodies are binary, `sent.body` only describes
/// them). Also returns the URL that answered, after redirects.
pub(super) async fn execute_with(
    client: &reqwest::Client,
    sent: &SentRequest,
    body: Vec<u8>,
) -> Result<(Response, String)> {
    let started = Instant::now();
    let mut response = build_request(client, sent, body)?
        .send()
        .await
        .map_err(|err| Error::Http(describe(err)))?;
    let final_url = response.url().to_string();
    let status = response.status();
    let headers = response_headers(&response);
    // Read before the body: reqwest's size hint shrinks while it is consumed.
    let announced = response.content_length();
    let (kept, received) = read_capped(&mut response).await?;
    let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
    let truncated = received > MAX_BODY_BYTES;
    // The real size when the server says it; otherwise what was read before stopping.
    let size = announced.map_or(received, |len| usize::try_from(len).unwrap_or(usize::MAX));
    let (body, binary, body_base64) = decode_body(&kept, truncated);
    let response = Response {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or_default().to_string(),
        headers,
        body,
        size: u32::try_from(size).unwrap_or(u32::MAX),
        binary,
        body_base64,
        truncated,
        duration_ms,
        sent: sent.clone(),
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    };
    Ok((response, final_url))
}

fn build_request(
    client: &reqwest::Client,
    sent: &SentRequest,
    body: Vec<u8>,
) -> Result<reqwest::RequestBuilder> {
    let method = reqwest::Method::from_bytes(sent.method.as_bytes())
        .map_err(|_| Error::Invalid(format!("invalid method {}", sent.method)))?;
    let mut builder = client.request(method, &sent.url);
    for header in &sent.headers {
        builder = builder.header(&header.name, &header.value);
    }
    if !body.is_empty() {
        builder = builder.body(body);
    }
    Ok(builder)
}

fn response_headers(response: &reqwest::Response) -> Vec<Header> {
    response
        .headers()
        .iter()
        .map(|(name, value)| Header {
            name: name.to_string(),
            value: String::from_utf8_lossy(value.as_bytes()).to_string(),
        })
        .collect()
}

/// Text bodies as text; anything else as base64, with `binary` set.
fn decode_body(kept: &[u8], truncated: bool) -> (String, bool, String) {
    match std::str::from_utf8(kept) {
        Ok(text) => (text.to_string(), false, String::new()),
        // A cut can split a multi-byte character; that alone doesn't make it binary.
        Err(err) if truncated && err.error_len().is_none() => (
            String::from_utf8_lossy(kept).to_string(),
            false,
            String::new(),
        ),
        Err(_) => (
            String::new(),
            true,
            base64::engine::general_purpose::STANDARD.encode(kept),
        ),
    }
}

/// Reads the body in chunks and stops one byte past [`MAX_BODY_BYTES`], so a huge download
/// never fills memory. Returns the kept bytes and how many were received.
pub(super) async fn read_capped(response: &mut reqwest::Response) -> Result<(Vec<u8>, usize)> {
    let mut kept = Vec::new();
    let mut received = 0usize;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| Error::Http(describe(err)))?
    {
        received = received.saturating_add(chunk.len());
        let room = MAX_BODY_BYTES.saturating_sub(kept.len());
        kept.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if received > MAX_BODY_BYTES {
            break;
        }
    }
    Ok((kept, received))
}

/// The raw body bytes: decoded from base64 for binary responses, UTF-8 for text. Used for
/// "save to file".
pub fn body_bytes(response: &Response) -> Result<Vec<u8>> {
    if !response.binary {
        return Ok(response.body.clone().into_bytes());
    }
    base64::engine::general_purpose::STANDARD
        .decode(&response.body_base64)
        .map_err(|err| Error::Invalid(format!("invalid binary body: {err}")))
}

/// reqwest's top-level message ("error sending request") hides the cause; include the chain.
/// The URL is left out: it can carry secrets in its query string.
pub(crate) fn describe(err: reqwest::Error) -> String {
    let err = err.without_url();
    let mut message = err.to_string();
    let mut source = std::error::Error::source(&err);
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message = format!("{message}: {text}");
        }
        source = cause.source();
    }
    message
}
