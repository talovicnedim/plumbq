//! "Copy as code": a request as a snippet for common HTTP clients, built from the same
//! prepared request as "Copy as cURL", so secrets stay `{{name}}`.

mod fetch;
mod go;
mod httpie;
mod laravel;
mod python;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::http_file::{Header, Request};
use crate::multipart::{BodyShape, Part, PartValue};
use crate::send::{self, SentRequest};
use crate::{Result, curl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CodeLanguage {
    Curl,
    Fetch,
    Python,
    Laravel,
    Go,
    Httpie,
}

/// The request at `id` (or its unsaved edit `request`) as code in `language`.
pub fn export(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
    language: CodeLanguage,
) -> Result<String> {
    let sent = curl::prepare_for_export(root, id, request, environment)?;
    Ok(generate(&sent, language))
}

pub fn generate(sent: &SentRequest, language: CodeLanguage) -> String {
    match language {
        CodeLanguage::Curl => curl::to_command(sent),
        CodeLanguage::Fetch => fetch::generate(sent),
        CodeLanguage::Python => python::generate(sent),
        CodeLanguage::Laravel => laravel::generate(sent),
        CodeLanguage::Go => go::generate(sent),
        CodeLanguage::Httpie => httpie::generate(sent),
    }
}

/// Headers to write in code. Multipart drops `Content-Type`: the client sets it with its own
/// boundary. Readable Basic credentials go through [`basic_auth`] instead.
fn code_headers<'a>(sent: &'a SentRequest, shape: &BodyShape) -> Vec<&'a Header> {
    sent.headers
        .iter()
        .filter(|h| {
            !(matches!(shape, BodyShape::Multipart(_))
                && h.name.eq_ignore_ascii_case("content-type"))
        })
        .filter(|h| send::basic_credentials(h).is_none())
        .collect()
}

/// User and password of a readable `Authorization: Basic user:password` header, for the
/// client's own basic auth option: base64 here would hide the `{{name}}` placeholders.
fn basic_auth(sent: &SentRequest) -> Option<(&str, &str)> {
    sent.headers
        .iter()
        .find_map(send::basic_credentials)?
        .split_once(':')
}

fn text_parts(parts: &[Part]) -> impl Iterator<Item = (&str, &str)> {
    parts.iter().filter_map(|p| match &p.value {
        PartValue::Text(text) => Some((p.name.as_str(), text.as_str())),
        PartValue::File(_) => None,
    })
}

fn file_parts(parts: &[Part]) -> impl Iterator<Item = (&Part, &str)> {
    parts.iter().filter_map(|p| match &p.value {
        PartValue::File(path) => Some((p, path.as_str())),
        PartValue::Text(_) => None,
    })
}

fn file_name<'a>(part: &'a Part, path: &'a str) -> &'a str {
    part.filename
        .as_deref()
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path))
}

/// JSON string escaping is valid for JavaScript, Python and Go string literals alike.
fn js_string(text: &str) -> String {
    serde_json::Value::String(text.to_string()).to_string()
}

#[cfg(test)]
mod tests;
