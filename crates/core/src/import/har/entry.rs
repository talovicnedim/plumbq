//! One recorded request turned into a request: headers the browser added are dropped, form
//! parameters become the body.

use serde_json::Value;

use crate::http_file::{Header, Request};
use crate::import::{scalar, text};

/// Set by the browser or the HTTP client itself; meaningless or harmful to replay.
const DROPPED_HEADERS: [&str; 10] = [
    "host",
    "connection",
    "content-length",
    "accept-encoding",
    "upgrade-insecure-requests",
    "priority",
    "te",
    "keep-alive",
    "pragma",
    "cookie",
];

pub(super) fn to_request(request: &Value, method: &str, url: &str) -> Request {
    let mut out = Request::new(&display_name(method, url), method, url);
    out.headers = list(request, "headers")
        .iter()
        .map(|h| Header {
            name: text(h, "name"),
            value: text(h, "value"),
        })
        .filter(|h| keep_header(&h.name))
        .collect();
    if let Some(post) = request.get("postData") {
        out.body = match list(post, "params") {
            [] => text(post, "text"),
            params => params
                .iter()
                .map(|p| {
                    format!(
                        "{}={}",
                        text(p, "name"),
                        scalar(p.get("value").unwrap_or(&Value::Null))
                    )
                })
                .collect::<Vec<_>>()
                .join("&"),
        };
        let mime = text(post, "mimeType");
        let has_type = out
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("content-type"));
        if !has_type && !mime.is_empty() && !out.body.is_empty() {
            out.headers.push(Header {
                name: "Content-Type".into(),
                value: mime,
            });
        }
    }
    out
}

pub(super) fn has_cookie(request: &Value) -> bool {
    list(request, "headers")
        .iter()
        .any(|h| text(h, "name").eq_ignore_ascii_case("cookie"))
}

/// `GET /users/42`.
fn display_name(method: &str, url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = rest
        .find('/')
        .map_or("/", |i| &rest[i..])
        .split(['?', '#'])
        .next()
        .unwrap_or("/");
    format!("{method} {path}")
}

fn keep_header(name: &str) -> bool {
    let lower = name.to_lowercase();
    !lower.starts_with(':')
        && !lower.starts_with("sec-")
        && !DROPPED_HEADERS.contains(&lower.as_str())
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}
