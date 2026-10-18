//! Bruno requests: `.bru` files (and single YAML request files) mapped to requests, plus helpers
//! shared with the folder walker in `dir.rs`. Scripts and tests are not imported.

pub(super) mod dir;
mod lang;
mod yaml;

use std::collections::BTreeMap;
use std::path::Path;

use base64::Engine;
use serde_json::Value;

use super::{ImportFormat, ImportedCollection, ImportedItem};
use crate::http_file::{Header, Request};
use crate::{Error, Result};
use lang::{Block, dict_of, get, text_of};
use yaml::yaml_request;

const METHODS: [&str; 9] = [
    "get", "post", "put", "patch", "delete", "options", "head", "connect", "trace",
];

pub fn parse_bru_file(path: &Path, text: &str) -> Result<ImportedCollection> {
    let (_, request) = bru_request(&lang::parse(text))
        .ok_or_else(|| Error::Invalid(format!("{} is not a Bruno request", path.display())))?;
    Ok(single(request))
}

/// A single Bruno YAML request file has `info` and `http` sections.
pub fn detect_yaml(doc: &Value) -> bool {
    doc.get("info").is_some() && doc.get("http").is_some()
}

pub fn parse_yaml_request(doc: &Value) -> Result<ImportedCollection> {
    Ok(single(yaml_request(doc).1))
}

fn single(request: Request) -> ImportedCollection {
    ImportedCollection {
        format: ImportFormat::Bruno,
        notes: Vec::new(),
        name: request.name.clone(),
        headers: BTreeMap::new(),
        items: vec![ImportedItem::Request(request)],
        environments: Vec::new(),
        credentials: Vec::new(),
    }
}

pub(super) fn bru_request(blocks: &[(String, Block)]) -> Option<(u32, Request)> {
    let (method, http) = blocks.iter().find_map(|(name, block)| match block {
        Block::Dict(entries) if METHODS.contains(&name.as_str()) => {
            Some((name.to_uppercase(), entries))
        }
        _ => None,
    })?;
    let meta = dict_of(blocks, "meta");
    let url = bru_url(blocks, &get(http, "url"));
    let mut request = Request::new(&get(meta, "name"), &method, &url);
    request.headers = dict_of(blocks, "headers")
        .iter()
        .map(|(k, v)| Header {
            name: k.clone(),
            value: convert(v),
        })
        .collect();
    request.headers.extend(bru_auth(blocks, &get(http, "auth")));
    let (body, content_type) = match get(http, "body").as_str() {
        "json" => (text_of(blocks, "body:json"), "application/json"),
        "text" => (text_of(blocks, "body:text"), "text/plain"),
        "xml" => (text_of(blocks, "body:xml"), "application/xml"),
        "graphql" => (Some(graphql(blocks)), "application/json"),
        "formUrlEncoded" | "form-urlencoded" => (
            Some(form(dict_of(blocks, "body:form-urlencoded"))),
            "application/x-www-form-urlencoded",
        ),
        "multipartForm" | "multipart-form" => (
            Some(form(dict_of(blocks, "body:multipart-form"))),
            "application/x-www-form-urlencoded",
        ),
        _ => (None, ""),
    };
    set_body(&mut request, body.map(|b| convert(&b)), content_type);
    Some((seq(&get(meta, "seq")), request))
}

/// The URL with `:name` path params filled in and query params added when it has none.
fn bru_url(blocks: &[(String, Block)], raw: &str) -> String {
    let mut url = convert(raw);
    for (name, value) in dict_of(blocks, "params:path") {
        let replacement = if value.is_empty() {
            format!("{{{{{name}}}}}")
        } else {
            convert(value)
        };
        url = url.replace(&format!(":{name}"), &replacement);
    }
    if url.contains('?') {
        return url;
    }
    let query: Vec<String> = dict_of(blocks, "params:query")
        .iter()
        .map(|(k, v)| format!("{k}={}", convert(v)))
        .collect();
    if query.is_empty() {
        return url;
    }
    format!("{url}?{}", query.join("&"))
}

pub(super) fn bru_auth(blocks: &[(String, Block)], mode: &str) -> Vec<Header> {
    let entries = dict_of(blocks, &format!("auth:{mode}"));
    let field = |key: &str| convert(&get(entries, key));
    auth_headers(mode, &field)
}

/// Auth as headers. Shared by `.bru` and YAML, which name the fields the same way.
pub(super) fn auth_headers(mode: &str, field: &dyn Fn(&str) -> String) -> Vec<Header> {
    let header = |name: &str, value: String| {
        vec![Header {
            name: name.into(),
            value,
        }]
    };
    match mode {
        "bearer" => header("Authorization", format!("Bearer {}", field("token"))),
        "basic" => {
            let pair = format!("{}:{}", field("username"), field("password"));
            // Encoding a template would bake in the braces, so variables stay readable instead.
            let value = match pair.contains("{{") {
                true => pair,
                false => base64::engine::general_purpose::STANDARD.encode(pair),
            };
            header("Authorization", format!("Basic {value}"))
        }
        "apikey" if field("placement") != "queryparams" => header(&field("key"), field("value")),
        _ => Vec::new(),
    }
}

fn graphql(blocks: &[(String, Block)]) -> String {
    let query = text_of(blocks, "body:graphql").unwrap_or_default();
    let variables = text_of(blocks, "body:graphql:vars")
        .and_then(|v| serde_json::from_str::<Value>(&v).ok())
        .unwrap_or(Value::Null);
    serde_json::json!({ "query": query, "variables": variables }).to_string()
}

fn form(entries: &[(String, String)]) -> String {
    entries
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

pub(super) fn set_body(request: &mut Request, body: Option<String>, content_type: &str) {
    let Some(body) = body.filter(|b| !b.is_empty()) else {
        return;
    };
    request.body = body;
    if !request
        .headers
        .iter()
        .any(|h| h.name.eq_ignore_ascii_case("content-type"))
    {
        request.headers.push(Header {
            name: "Content-Type".into(),
            value: content_type.into(),
        });
    }
}

/// `{{process.env.X}}` reads an OS environment variable in Bruno; ours is `{{$processEnv X}}`.
pub(super) fn convert(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{process.env.") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "{{process.env.".len()..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        out.push_str(&format!("{{{{$processEnv {}}}}}", after[..end].trim()));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

pub(super) fn seq(text: &str) -> u32 {
    text.trim().parse().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
