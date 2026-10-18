//! OpenAPI 3.x and Swagger 2.0 (JSON or YAML): one request per operation, a folder per tag,
//! and an environment with `baseUrl` plus secret names for the security schemes.

mod body;
mod security;

use std::collections::BTreeMap;

use serde_json::Value;

use super::schema::{resolve, sample};
use super::{ImportFormat, ImportedCollection, ImportedEnvironment, ImportedItem, scalar, text};
use crate::Result;
use crate::http_file::{Header, Request};

const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

pub fn parse(doc: &Value, format: ImportFormat) -> Result<ImportedCollection> {
    let name = doc
        .pointer("/info/title")
        .and_then(Value::as_str)
        .unwrap_or("API")
        .to_string();
    let mut secrets: Vec<String> = Vec::new();
    let mut folders = TagFolders::new(doc);
    for (path, item) in doc
        .get("paths")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let Some(item) = resolve(doc, item) else {
            continue;
        };
        for method in METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let request =
                operation_request(doc, format, path, method, item, operation, &mut secrets);
            folders.add(first_tag(operation), request);
        }
    }
    Ok(ImportedCollection {
        notes: Vec::new(),
        format,
        environments: vec![ImportedEnvironment {
            name: name.clone(),
            variables: BTreeMap::from([("baseUrl".to_string(), base_url(doc, format))]),
            secrets,
        }],
        name,
        headers: BTreeMap::new(),
        items: folders.into_items(),
        credentials: Vec::new(),
    })
}

/// Tag folders in the order of the spec's `tags` list, then as first used; untagged requests
/// come last.
struct TagFolders {
    tagged: Vec<(String, Vec<ImportedItem>)>,
    untagged: Vec<ImportedItem>,
}

impl TagFolders {
    fn new(doc: &Value) -> Self {
        let tagged = doc
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|tag| (text(tag, "name"), Vec::new()))
            .collect();
        Self {
            tagged,
            untagged: Vec::new(),
        }
    }

    fn add(&mut self, tag: Option<&str>, request: Request) {
        let item = ImportedItem::Request(request);
        let Some(tag) = tag else {
            self.untagged.push(item);
            return;
        };
        match self.tagged.iter_mut().find(|(name, _)| name == tag) {
            Some((_, items)) => items.push(item),
            None => self.tagged.push((tag.to_string(), vec![item])),
        }
    }

    fn into_items(self) -> Vec<ImportedItem> {
        let mut items: Vec<ImportedItem> = self
            .tagged
            .into_iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(name, items)| ImportedItem::Folder {
                name,
                headers: BTreeMap::new(),
                items,
            })
            .collect();
        items.extend(self.untagged);
        items
    }
}

fn first_tag(operation: &Value) -> Option<&str> {
    operation
        .get("tags")
        .and_then(Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(Value::as_str)
}

fn base_url(doc: &Value, format: ImportFormat) -> String {
    let url = match format {
        ImportFormat::Swagger => {
            let scheme = doc
                .pointer("/schemes/0")
                .and_then(Value::as_str)
                .unwrap_or("https");
            let host = text(doc, "host");
            match host.is_empty() {
                true => text(doc, "basePath"),
                false => format!("{scheme}://{host}{}", text(doc, "basePath")),
            }
        }
        _ => {
            let server = doc.pointer("/servers/0").unwrap_or(&Value::Null);
            let mut url = text(server, "url");
            for (name, var) in server
                .get("variables")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                url = url.replace(&format!("{{{name}}}"), &text(var, "default"));
            }
            url
        }
    };
    let url = url.trim_end_matches('/').to_string();
    // Relative or missing servers resolve against wherever the API runs locally.
    if url.contains("://") {
        url
    } else {
        format!("http://localhost{url}")
    }
}

fn operation_request(
    doc: &Value,
    format: ImportFormat,
    path: &str,
    method: &str,
    item: &Value,
    operation: &Value,
    secrets: &mut Vec<String>,
) -> Request {
    let upper = method.to_uppercase();
    let name = [text(operation, "summary"), text(operation, "operationId")]
        .into_iter()
        .find(|n| !n.is_empty())
        .unwrap_or_else(|| format!("{upper} {path}"));
    let url = format!(
        "{{{{baseUrl}}}}{}",
        path.replace('{', "{{").replace('}', "}}")
    );
    let mut request = Request::new(&name, &upper, &url);
    add_params(doc, item, operation, &mut request);
    if let Some(body) = operation.get("requestBody").and_then(|b| resolve(doc, b)) {
        body::set_body(doc, &mut request, body);
    }
    if wants_json(operation, doc, format) {
        request.headers.push(header("Accept", "application/json"));
    }
    security::add_security(doc, format, operation, &mut request, secrets);
    request
}

/// Path-level and operation parameters: required query values, headers, and the Swagger 2
/// `body` parameter.
fn add_params(doc: &Value, item: &Value, operation: &Value, request: &mut Request) {
    let params = item
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            operation
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        );
    let mut query = Vec::new();
    for param in params {
        let Some(param) = resolve(doc, param) else {
            continue;
        };
        let value = param_value(doc, param);
        match text(param, "in").as_str() {
            "query" if param.get("required").and_then(Value::as_bool) == Some(true) => {
                query.push(format!("{}={value}", text(param, "name")));
            }
            "header" => request.headers.push(Header {
                name: text(param, "name"),
                value,
            }),
            "body" => body::set_json_body(
                request,
                sample(doc, param.get("schema").unwrap_or(&Value::Null)),
            ),
            _ => {}
        }
    }
    if !query.is_empty() {
        request.url = format!("{}?{}", request.url, query.join("&"));
    }
}

fn param_value(doc: &Value, param: &Value) -> String {
    let schema = param.get("schema").unwrap_or(param);
    let given = [
        param.get("example"),
        schema.get("example"),
        schema.get("default"),
        param.get("default"),
    ];
    if let Some(value) = given.into_iter().flatten().next() {
        return scalar(value);
    }
    match resolve(doc, schema)
        .and_then(|s| s.get("enum"))
        .and_then(|e| e.get(0))
    {
        Some(first) => scalar(first),
        None => String::new(),
    }
}

fn wants_json(operation: &Value, doc: &Value, format: ImportFormat) -> bool {
    if format == ImportFormat::Swagger {
        let produces = operation.get("produces").or_else(|| doc.get("produces"));
        return produces.is_some_and(|p| p.to_string().contains("json"));
    }
    let responses = operation.get("responses").and_then(Value::as_object);
    responses.into_iter().flatten().any(|(_, response)| {
        resolve(doc, response)
            .and_then(|r| r.get("content"))
            .and_then(Value::as_object)
            .is_some_and(|content| content.keys().any(|kind| kind.contains("json")))
    })
}

pub(super) fn header(name: &str, value: &str) -> Header {
    Header {
        name: name.to_string(),
        value: value.to_string(),
    }
}

#[cfg(test)]
mod tests;
