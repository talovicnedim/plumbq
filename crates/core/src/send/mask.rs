//! Keeping secret values out of everything a send returns or stores: responses, binary bodies,
//! and error messages.

use std::collections::BTreeMap;

use base64::Engine;

use super::{Response, SentRequest};
use crate::environments::Values;
use crate::http_client::CrossOriginRedirects;
use crate::http_file::Header;
use crate::{Error, variables};

/// Which redirects to another origin the request may follow, by where it carries secrets.
pub(super) fn cross_origin_redirects(
    headers: &[Header],
    body: &[u8],
    values: &Values,
) -> CrossOriginRedirects {
    if carries_secret(headers, values) {
        return CrossOriginRedirects::Stop;
    }
    match contains_secret(&String::from_utf8_lossy(body), values) {
        true => CrossOriginRedirects::StopBodyResends,
        false => CrossOriginRedirects::Follow,
    }
}

/// Whether any header carries a secret value.
fn carries_secret(headers: &[Header], values: &Values) -> bool {
    headers.iter().any(|h| contains_secret(&h.value, values))
}

pub(super) fn contains_secret(text: &str, values: &Values) -> bool {
    values
        .secrets
        .values()
        .any(|s| s.chars().count() >= variables::MIN_MASK_LEN && text.contains(s.as_str()))
}

/// Error messages (e.g. reqwest's, which name the URL) never show secret values.
pub(super) fn mask_error(err: Error, values: &Values) -> Error {
    let mask = |text: String| variables::mask(&text, &values.secrets);
    match err {
        Error::Http(message) => Error::Http(mask(message)),
        Error::Invalid(message) => Error::Invalid(mask(message)),
        other => other,
    }
}

pub(super) fn mask_response(response: Response, values: &Values) -> Response {
    let mask = |text: &str| variables::mask(text, &values.secrets);
    let mask_headers = |headers: &[Header]| -> Vec<Header> {
        headers
            .iter()
            .map(|h| Header {
                name: h.name.clone(),
                value: mask(&h.value),
            })
            .collect()
    };
    let body_base64 = match response.binary {
        true => mask_base64(&response.body_base64, &values.secrets),
        false => response.body_base64.clone(),
    };
    let (body, headers) = (mask(&response.body), mask_headers(&response.headers));
    let masked_secrets = masked_names(&response, &body, &headers, &values.secrets);
    Response {
        headers,
        body,
        body_base64,
        masked_secrets,
        sent: SentRequest {
            url: mask(&response.sent.url),
            headers: mask_headers(&response.sent.headers),
            body: mask(&response.sent.body),
            ..response.sent
        },
        ..response
    }
}

/// Secrets whose `{{name}}` is in the masked body or headers but not in what the server sent.
fn masked_names(
    raw: &Response,
    body: &str,
    headers: &[Header],
    secrets: &BTreeMap<String, String>,
) -> Vec<String> {
    let in_raw = |p: &str| raw.body.contains(p) || raw.headers.iter().any(|h| h.value.contains(p));
    let in_masked = |p: &str| body.contains(p) || headers.iter().any(|h| h.value.contains(p));
    secrets
        .keys()
        .filter(|name| {
            let placeholder = format!("{{{{{name}}}}}");
            in_masked(&placeholder) && !in_raw(&placeholder)
        })
        .cloned()
        .collect()
}

/// Masks secrets in a binary body too: a response that isn't valid UTF-8 can still echo a token.
pub(super) fn mask_base64(encoded: &str, secrets: &BTreeMap<String, String>) -> String {
    let engine = base64::engine::general_purpose::STANDARD;
    match engine.decode(encoded) {
        Ok(bytes) => engine.encode(variables::mask_bytes(&bytes, secrets)),
        // Not ours to fix; dropping it is safer than returning it unmasked.
        Err(_) => String::new(),
    }
}
