//! Bruno's YAML (OpenCollection) request and folder files, Bruno 3 and later.

use serde_json::Value;

use super::{auth_headers, convert, seq, set_body};
use crate::http_file::{Header, Request};
use crate::import::{scalar, text};

pub(super) fn yaml_request(doc: &Value) -> (u32, Request) {
    let http = doc.get("http").unwrap_or(&Value::Null);
    let name = doc.pointer("/info/name").map(scalar).unwrap_or_default();
    let method = text(http, "method").to_uppercase();
    let mut request = Request::new(
        &name,
        if method.is_empty() { "GET" } else { &method },
        &convert(&text(http, "url")),
    );
    request.headers = yaml_headers(doc);
    request.headers.extend(yaml_auth(doc));
    let body = http
        .get("body")
        .or_else(|| doc.get("body"))
        .unwrap_or(&Value::Null);
    let (data, content_type) = match text(body, "type").as_str() {
        "json" => (
            scalar(body.get("data").unwrap_or(&Value::Null)),
            "application/json",
        ),
        "text" => (text(body, "data"), "text/plain"),
        "xml" => (text(body, "data"), "application/xml"),
        "form-urlencoded" | "multipart-form" => (
            yaml_pairs(body.get("data"))
                .iter()
                .map(|h| format!("{}={}", h.name, h.value))
                .collect::<Vec<_>>()
                .join("&"),
            "application/x-www-form-urlencoded",
        ),
        _ => (String::new(), ""),
    };
    set_body(&mut request, Some(convert(&data)), content_type);
    let order = doc.pointer("/info/seq").map(scalar).unwrap_or_default();
    (seq(&order), request)
}

pub(super) fn yaml_headers(doc: &Value) -> Vec<Header> {
    let list = doc.pointer("/http/headers").or_else(|| doc.get("headers"));
    yaml_pairs(list)
}

pub(super) fn yaml_auth(doc: &Value) -> Vec<Header> {
    let Some(auth) = doc
        .pointer("/http/auth")
        .or_else(|| doc.get("auth"))
        .filter(|a| a.is_object())
    else {
        return Vec::new();
    };
    let field = |key: &str| convert(&auth.get(key).map(scalar).unwrap_or_default());
    auth_headers(&text(auth, "type"), &field)
}

/// `[{name, value, disabled?}]`, skipping disabled entries.
pub(super) fn yaml_pairs(list: Option<&Value>) -> Vec<Header> {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| !p.get("disabled").and_then(Value::as_bool).unwrap_or(false))
        .map(|p| Header {
            name: text(p, "name"),
            value: convert(&scalar(p.get("value").unwrap_or(&Value::Null))),
        })
        .collect()
}
