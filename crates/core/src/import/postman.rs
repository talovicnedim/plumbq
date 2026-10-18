//! Postman Collection v2.0 / v2.1 (JSON). Postman's `{{variable}}` syntax is the same as ours.

use std::collections::BTreeMap;

use base64::Engine;
use serde_json::Value;

use super::{ImportFormat, ImportedCollection, ImportedEnvironment, ImportedItem, scalar, text};
use crate::Result;
use crate::http_file::{Header, Request};

pub fn detect(doc: &Value) -> bool {
    let schema = doc
        .pointer("/info/schema")
        .and_then(Value::as_str)
        .unwrap_or_default();
    schema.contains("getpostman.com") || (doc.get("info").is_some() && doc.get("item").is_some())
}

pub fn parse(doc: &Value) -> Result<ImportedCollection> {
    let name = doc
        .pointer("/info/name")
        .and_then(Value::as_str)
        .unwrap_or("Postman collection")
        .to_string();
    let variables: BTreeMap<String, String> = list(doc, "variable")
        .iter()
        .filter(|v| !v.get("disabled").and_then(Value::as_bool).unwrap_or(false))
        .map(|v| {
            (
                text(v, "key"),
                scalar(v.get("value").unwrap_or(&Value::Null)),
            )
        })
        .filter(|(key, _)| !key.is_empty())
        .collect();
    let environments = match variables.is_empty() {
        true => Vec::new(),
        false => vec![ImportedEnvironment {
            name: name.clone(),
            variables,
            secrets: Vec::new(),
        }],
    };
    Ok(ImportedCollection {
        notes: Vec::new(),
        format: ImportFormat::Postman,
        headers: auth_headers(doc.get("auth")).into_iter().collect(),
        items: items(doc),
        name,
        environments,
        credentials: declared_credentials(doc),
    })
}

/// The literal API keys of every `apikey` auth in the collection. They may land in a header or
/// query parameter of any name.
fn declared_credentials(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    match value {
        Value::Object(fields) => {
            if let Some(auth) = fields.get("auth")
                && text(auth, "type") == "apikey"
            {
                let key = list(auth, "apikey")
                    .iter()
                    .find(|p| text(p, "key") == "value");
                found.extend(key.map(|p| scalar(p.get("value").unwrap_or(&Value::Null))));
            }
            found.extend(fields.values().flat_map(declared_credentials));
        }
        Value::Array(items) => found.extend(items.iter().flat_map(declared_credentials)),
        _ => {}
    }
    found
}

fn items(parent: &Value) -> Vec<ImportedItem> {
    list(parent, "item")
        .iter()
        .map(|item| match item.get("request") {
            Some(request) => ImportedItem::Request(to_request(&text(item, "name"), request)),
            None => ImportedItem::Folder {
                name: text(item, "name"),
                headers: auth_headers(item.get("auth")).into_iter().collect(),
                items: items(item),
            },
        })
        .collect()
}

fn to_request(name: &str, request: &Value) -> Request {
    // v2.0 allows a request to be just a URL string.
    if let Some(url) = request.as_str() {
        return Request::new(name, "GET", url);
    }
    let method = text(request, "method");
    let mut out = Request::new(
        name,
        if method.is_empty() { "GET" } else { &method },
        &url(request.get("url")),
    );
    for header in list(request, "header") {
        if header.get("disabled").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        out.headers.push(Header {
            name: text(header, "key"),
            value: scalar(header.get("value").unwrap_or(&Value::Null)),
        });
    }
    for (key, value) in auth_headers(request.get("auth")) {
        if key == "?" {
            let separator = if out.url.contains('?') { '&' } else { '?' };
            out.url = format!("{}{separator}{value}", out.url);
        } else {
            out.headers.push(Header { name: key, value });
        }
    }
    if let Some(body) = request.get("body") {
        set_body(&mut out, body);
    }
    out
}

fn url(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(raw)) => raw.clone(),
        Some(url) if url.get("raw").is_some() => text(url, "raw"),
        Some(url) => {
            let host = join(url.get("host"), ".");
            let path = join(url.get("path"), "/");
            let protocol = text(url, "protocol");
            let base = match protocol.is_empty() {
                true => host,
                false => format!("{protocol}://{host}"),
            };
            format!("{base}/{path}")
        }
        None => String::new(),
    }
}

fn join(value: Option<&Value>, separator: &str) -> String {
    match value {
        Some(Value::Array(parts)) => parts.iter().map(scalar).collect::<Vec<_>>().join(separator),
        Some(other) => scalar(other),
        None => String::new(),
    }
}

fn set_body(request: &mut Request, body: &Value) {
    let content_type = match text(body, "mode").as_str() {
        "raw" => {
            request.body = text(body, "raw");
            let language = body
                .pointer("/options/raw/language")
                .and_then(Value::as_str);
            match language {
                Some("json") => "application/json",
                Some("xml") => "application/xml",
                _ => "",
            }
        }
        // Form data becomes url-encoded: `.http` has no simple multipart form; files are dropped.
        mode @ ("urlencoded" | "formdata") => {
            request.body = list(body, mode)
                .iter()
                .filter(|p| p.get("disabled").and_then(Value::as_bool) != Some(true))
                .filter(|p| text(p, "type") != "file")
                .map(|p| format!("{}={}", text(p, "key"), text(p, "value")))
                .collect::<Vec<_>>()
                .join("&");
            "application/x-www-form-urlencoded"
        }
        "graphql" => {
            let graphql = body.get("graphql").unwrap_or(&Value::Null);
            let variables = serde_json::from_str::<Value>(&text(graphql, "variables"))
                .unwrap_or(Value::Object(Default::default()));
            let payload =
                serde_json::json!({ "query": text(graphql, "query"), "variables": variables });
            request.body = serde_json::to_string_pretty(&payload).unwrap_or_default();
            "application/json"
        }
        _ => "",
    };
    let has_type = request
        .headers
        .iter()
        .any(|h| h.name.eq_ignore_ascii_case("content-type"));
    if !content_type.is_empty() && !request.body.is_empty() && !has_type {
        request.headers.push(Header {
            name: "Content-Type".into(),
            value: content_type.into(),
        });
    }
}

/// Postman auth as headers. An API key sent in the query comes back with the name `?` and the
/// `key=value` pair as value.
fn auth_headers(auth: Option<&Value>) -> Vec<(String, String)> {
    let Some(auth) = auth else {
        return Vec::new();
    };
    let kind = text(auth, "type");
    let field = |name: &str| {
        list(auth, &kind)
            .iter()
            .find(|p| text(p, "key") == name)
            .map(|p| scalar(p.get("value").unwrap_or(&Value::Null)))
            .unwrap_or_default()
    };
    match kind.as_str() {
        "bearer" => vec![("Authorization".into(), format!("Bearer {}", field("token")))],
        "basic" => {
            let pair = format!("{}:{}", field("username"), field("password"));
            // Encoding a template would bake in the braces, so variables stay readable instead.
            let value = match pair.contains("{{") {
                true => pair,
                false => base64::engine::general_purpose::STANDARD.encode(pair),
            };
            vec![("Authorization".into(), format!("Basic {value}"))]
        }
        "apikey" if field("in") == "query" => {
            vec![("?".into(), format!("{}={}", field("key"), field("value")))]
        }
        "apikey" => vec![(field("key"), field("value"))],
        _ => Vec::new(),
    }
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "postman_tests.rs"]
mod tests;
