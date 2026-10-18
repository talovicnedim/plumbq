//! The request as it goes on the wire: variables filled in, folder headers added, and the
//! method a server may act on.

use std::path::Path;

use super::{SAFE_METHODS, SentRequest, basic_auth};
use crate::environments::Values;
use crate::http_file::{Header, Request};
use crate::workspace::{self, parse_request_id, resolve};
use crate::{Result, body_files, file_vars, variables};

/// Headers that ask a server to act as if the request used another method.
const METHOD_OVERRIDES: [&str; 3] = [
    "X-HTTP-Method-Override",
    "X-HTTP-Method",
    "X-Method-Override",
];

/// Every part of a request that can contain variables, folder headers included.
pub fn request_text(root: &Path, id: &str, request: &Request) -> Result<String> {
    let mut parts = vec![request.url.clone(), request.body.clone()];
    parts.extend(body_files::templates(root, id, &request.body));
    parts.extend(request.assertions.iter().cloned());
    parts.extend(file_vars::raw_values(root, id, request)?);
    for header in folder_headers(root, id)?
        .into_iter()
        .chain(request.headers.clone())
    {
        parts.push(header.value);
    }
    Ok(parts.join("\n"))
}

/// The request as it goes over the wire, with real secret values. Readable Basic credentials
/// stay `Basic user:password` here (so "copy as code" can show them); the wire encodes them.
pub fn prepare(root: &Path, id: &str, request: &Request, values: &Values) -> Result<SentRequest> {
    let mut headers = Vec::new();
    for header in folder_headers(root, id)?
        .into_iter()
        .chain(request.headers.clone())
    {
        headers.retain(|h: &Header| !h.name.eq_ignore_ascii_case(&header.name));
        let value = match header.name.eq_ignore_ascii_case("authorization") {
            true => basic_auth::substitute(&header.value, values)?,
            false => variables::substitute(&header.value, values)?,
        };
        headers.push(Header {
            name: header.name,
            value,
        });
    }
    let mut url = variables::substitute(&request.url, values)?;
    if !has_scheme(&url) {
        url = format!("http://{url}");
    }
    Ok(SentRequest {
        method: request.method.to_uppercase(),
        url,
        headers,
        body: variables::substitute(&request.body, values)?,
    })
}

/// The method the server may act on: the request's own, or the value of a method override
/// header (in the request or a folder) when the request's method is safe. The override is read
/// as written, so one made of `{{variables}}` never counts as safe; an empty one is no override.
pub fn effective_method(root: &Path, id: &str, request: &Request) -> Result<String> {
    let method = request.method.to_uppercase();
    if !SAFE_METHODS.contains(&method.as_str()) {
        return Ok(method);
    }
    let overridden = folder_headers(root, id)?
        .into_iter()
        .chain(request.headers.iter().cloned())
        .filter(|h| {
            METHOD_OVERRIDES
                .iter()
                .any(|o| h.name.eq_ignore_ascii_case(o))
        })
        .map(|h| h.value.trim().to_uppercase())
        .find(|m| !m.is_empty() && !SAFE_METHODS.contains(&m.as_str()));
    Ok(overridden.unwrap_or(method))
}

/// Starts with `scheme://` (RFC 3986 scheme characters), so `localhost:3000/cb?next=https://x`
/// has none.
fn has_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once("://") else {
        return false;
    };
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Shared headers from `folder.toml` files, from the workspace root down to the request's
/// folder; closer folders override.
fn folder_headers(root: &Path, id: &str) -> Result<Vec<Header>> {
    let (rel, _) = parse_request_id(id)?;
    let (folder, _) = workspace::split_rel(rel);
    let mut dirs = vec![String::new()];
    let mut current = String::new();
    for part in folder.split('/').filter(|p| !p.is_empty()) {
        current = workspace::join_rel(&current, part);
        dirs.push(current.clone());
    }
    let mut headers = Vec::new();
    for dir in dirs {
        let settings = workspace::read_settings(&resolve(root, &dir)?)?;
        for (name, value) in settings.headers {
            headers.push(Header { name, value });
        }
    }
    Ok(headers)
}
