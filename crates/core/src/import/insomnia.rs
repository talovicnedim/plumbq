//! Insomnia exports: v4 (JSON, a flat `resources` list linked by `parentId`) and v5 (YAML,
//! a nested `collection`). Insomnia writes variables as `{{ _.name }}`; they become `{{name}}`.

use std::collections::BTreeMap;

use base64::Engine;
use serde_json::Value;

use super::{ImportFormat, ImportedCollection, ImportedEnvironment, ImportedItem, scalar, text};
use crate::Result;
use crate::http_file::{Header, Request};

pub fn detect(doc: &Value) -> bool {
    let kind = text(doc, "type");
    // Design documents (`spec.…`) keep their requests in the same `collection` as collections.
    text(doc, "_type") == "export"
        || kind.starts_with("collection.insomnia.rest/")
        || kind.starts_with("spec.insomnia.rest/")
}

pub fn parse(doc: &Value) -> Result<ImportedCollection> {
    if text(doc, "_type") == "export" {
        Ok(parse_v4(doc))
    } else {
        Ok(parse_v5(doc))
    }
}

fn parse_v4(doc: &Value) -> ImportedCollection {
    let resources: &[Value] = doc
        .get("resources")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let workspace = of_type(resources, "workspace").next();
    let name = workspace.map(|w| text(w, "name")).unwrap_or_default();
    let root_id = workspace.map(|w| text(w, "_id")).unwrap_or_default();

    // The base environment hangs off the workspace; sub-environments inherit its data.
    let base = of_type(resources, "environment").find(|e| text(e, "parentId") == root_id);
    let base_vars = base.map(|e| variables(e.get("data"))).unwrap_or_default();
    let base_id = base.map(|e| text(e, "_id")).unwrap_or_default();
    let mut environments: Vec<ImportedEnvironment> = of_type(resources, "environment")
        .filter(|e| !base_id.is_empty() && text(e, "parentId") == base_id)
        .map(|e| {
            let mut vars = base_vars.clone();
            vars.extend(variables(e.get("data")));
            ImportedEnvironment {
                name: text(e, "name"),
                variables: vars,
                secrets: Vec::new(),
            }
        })
        .collect();
    if environments.is_empty() && !base_vars.is_empty() {
        environments.push(ImportedEnvironment {
            name: name.clone(),
            variables: base_vars,
            secrets: Vec::new(),
        });
    }

    ImportedCollection {
        notes: Vec::new(),
        format: ImportFormat::Insomnia,
        name: if name.is_empty() {
            "Insomnia".into()
        } else {
            name
        },
        headers: BTreeMap::new(),
        items: v4_children(resources, &root_id),
        environments,
        credentials: Vec::new(),
    }
}

fn of_type<'a>(resources: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> {
    resources.iter().filter(move |r| text(r, "_type") == kind)
}

fn v4_children(resources: &[Value], parent: &str) -> Vec<ImportedItem> {
    let mut children: Vec<&Value> = resources
        .iter()
        .filter(|r| text(r, "parentId") == parent)
        .filter(|r| matches!(text(r, "_type").as_str(), "request" | "request_group"))
        .collect();
    // Insomnia orders siblings by `metaSortKey`, smallest first.
    children.sort_by(|a, b| sort_key(a).total_cmp(&sort_key(b)));
    children
        .into_iter()
        .map(|r| match text(r, "_type").as_str() {
            "request" => ImportedItem::Request(to_request(r)),
            _ => ImportedItem::Folder {
                name: text(r, "name"),
                headers: BTreeMap::new(),
                items: v4_children(resources, &text(r, "_id")),
            },
        })
        .collect()
}

fn sort_key(resource: &Value) -> f64 {
    resource
        .get("metaSortKey")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn parse_v5(doc: &Value) -> ImportedCollection {
    let name = text(doc, "name");
    let envs = doc.get("environments").unwrap_or(&Value::Null);
    let base_vars = variables(envs.get("data"));
    let mut environments: Vec<ImportedEnvironment> = envs
        .get("subEnvironments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|e| {
            let mut vars = base_vars.clone();
            vars.extend(variables(e.get("data")));
            ImportedEnvironment {
                name: text(e, "name"),
                variables: vars,
                secrets: Vec::new(),
            }
        })
        .collect();
    if environments.is_empty() && !base_vars.is_empty() {
        environments.push(ImportedEnvironment {
            name: name.clone(),
            variables: base_vars,
            secrets: Vec::new(),
        });
    }
    ImportedCollection {
        notes: Vec::new(),
        format: ImportFormat::Insomnia,
        name: if name.is_empty() {
            "Insomnia".into()
        } else {
            name
        },
        headers: BTreeMap::new(),
        items: v5_items(doc.get("collection")),
        environments,
        credentials: Vec::new(),
    }
}

fn v5_items(list: Option<&Value>) -> Vec<ImportedItem> {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| match item.get("children") {
            Some(children) => ImportedItem::Folder {
                name: text(item, "name"),
                headers: BTreeMap::new(),
                items: v5_items(Some(children)),
            },
            None => ImportedItem::Request(to_request(item)),
        })
        .collect()
}

/// The request shape is the same in v4 and v5.
fn to_request(r: &Value) -> Request {
    let method = text(r, "method");
    let mut request = Request::new(
        &text(r, "name"),
        if method.is_empty() { "GET" } else { &method },
        &convert(&text(r, "url")),
    );
    for header in r
        .get("headers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if header.get("disabled").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        request.headers.push(Header {
            name: text(header, "name"),
            value: convert(&text(header, "value")),
        });
    }
    set_body(&mut request, r.get("body").unwrap_or(&Value::Null));
    if let Some(value) = auth(r.get("authentication")) {
        request.headers.push(Header {
            name: "Authorization".into(),
            value,
        });
    }
    request
}

/// Form params or text, with a `Content-Type` from the source's mime type when none is set.
fn set_body(request: &mut Request, body: &Value) {
    let mime = text(body, "mimeType");
    request.body = match body.get("params").and_then(Value::as_array) {
        Some(params) => params
            .iter()
            .filter(|p| p.get("disabled").and_then(Value::as_bool) != Some(true))
            .map(|p| format!("{}={}", text(p, "name"), convert(&text(p, "value"))))
            .collect::<Vec<_>>()
            .join("&"),
        None => convert(&text(body, "text")),
    };
    let has_type = request
        .headers
        .iter()
        .any(|h| h.name.eq_ignore_ascii_case("content-type"));
    if mime.is_empty() || request.body.is_empty() || has_type {
        return;
    }
    // Form fields are written url-encoded, whatever multipart type the source used.
    let mime = if mime.starts_with("multipart/") {
        "application/x-www-form-urlencoded"
    } else {
        &mime
    };
    request.headers.push(Header {
        name: "Content-Type".into(),
        value: mime.to_string(),
    });
}

fn auth(auth: Option<&Value>) -> Option<String> {
    let auth = auth?;
    if auth.get("disabled").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    match text(auth, "type").as_str() {
        "bearer" => {
            let prefix = text(auth, "prefix");
            let prefix = if prefix.is_empty() {
                "Bearer".to_string()
            } else {
                prefix
            };
            Some(format!("{prefix} {}", convert(&text(auth, "token"))))
        }
        "basic" => {
            let pair = format!(
                "{}:{}",
                convert(&text(auth, "username")),
                convert(&text(auth, "password"))
            );
            let value = match pair.contains("{{") {
                true => pair,
                false => base64::engine::general_purpose::STANDARD.encode(pair),
            };
            Some(format!("Basic {value}"))
        }
        _ => None,
    }
}

/// Environment data as flat string variables; nested values are kept as JSON text.
fn variables(data: Option<&Value>) -> BTreeMap<String, String> {
    data.and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(key, value)| (key.clone(), convert(&scalar(value))))
        .collect()
}

/// `{{ _.baseUrl }}` / `{{_.baseUrl}}` → `{{baseUrl}}`. Template tags (`{% … %}`) are kept.
fn convert(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(len) = rest[start + 2..].find("}}") else {
            break;
        };
        let inner = rest[start + 2..start + 2 + len].trim();
        out.push_str(&rest[..start]);
        out.push_str(&format!(
            "{{{{{}}}}}",
            inner.strip_prefix("_.").unwrap_or(inner)
        ));
        rest = &rest[start + 2 + len + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "insomnia_tests.rs"]
mod tests;
