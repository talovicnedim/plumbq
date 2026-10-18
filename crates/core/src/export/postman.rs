//! Postman Collection v2.1 export. What changes on the way: `docs/formats.md` (Postman
//! export).

use std::path::Path;

use serde_json::{Value, json};

use super::{effective_headers, subtree};
use crate::http_file::{Header, Request};
use crate::multipart::{self, BodyShape, PartValue};
use crate::workspace::{self, TreeNode};
use crate::{Result, environments, file_vars, request_meta, requests};

const SCHEMA: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

/// The folder at `folder` (`""` for the whole workspace) as a Postman collection.
pub fn collection(root: &Path, folder: &str) -> Result<Value> {
    let name = match folder.rsplit('/').next().filter(|n| !n.is_empty()) {
        Some(name) => name.to_string(),
        None => root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Workspace".into()),
    };
    let variables: Vec<Value> = workspace::read_settings(root)?
        .variables
        .iter()
        .map(|(key, value)| json!({ "key": key, "value": value }))
        .collect();
    let mut collection = json!({
        "info": {
            "_postman_id": uuid::Uuid::new_v4().to_string(),
            "name": name,
            "schema": SCHEMA,
        },
        "item": items(root, &subtree(root, folder)?)?,
    });
    if !variables.is_empty() {
        collection["variable"] = Value::Array(variables);
    }
    Ok(collection)
}

/// Every environment as a Postman environment file: `(file name, contents)`.
pub fn environments(root: &Path) -> Result<Vec<(String, Value)>> {
    environments::list(root)?
        .into_iter()
        .map(|env| {
            let mut values: Vec<Value> = env
                .variables
                .iter()
                .map(|(key, value)| {
                    json!({ "key": key, "value": postman_text(value), "type": "default", "enabled": true })
                })
                .collect();
            values.extend(env.secrets.iter().map(|secret| {
                json!({ "key": secret.name, "value": "", "type": "secret", "enabled": true })
            }));
            let file = json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "name": env.name,
                "values": values,
                "_postman_variable_scope": "environment",
            });
            Ok((format!("{}.postman_environment.json", env.name), file))
        })
        .collect()
}

fn items(root: &Path, nodes: &[TreeNode]) -> Result<Vec<Value>> {
    nodes
        .iter()
        .map(|node| match node {
            TreeNode::Folder { name, children, .. } => {
                Ok(json!({ "name": name, "item": items(root, children)? }))
            }
            TreeNode::Request { id, name, .. } => {
                let request = requests::get(root, id)?;
                let request = inline_file_variables(root, id, request)?;
                item(root, id, name, &request)
            }
        })
        .collect()
}

/// The request with its file's `@name = value` variables written into URL, headers and body.
fn inline_file_variables(root: &Path, id: &str, mut request: Request) -> Result<Request> {
    let vars = file_vars::collect(root, id, &request)?;
    if vars.is_empty() {
        return Ok(request);
    }
    let inline = |text: &str| -> String {
        let mut text = text.to_string();
        // Later variables may use earlier ones (`@api = {{host}}/v2`): a few passes settle it.
        for _ in 0..vars.len() {
            let before = text.clone();
            for var in &vars {
                text = replace_variable(&text, &var.name, &var.value);
            }
            if text == before {
                break;
            }
        }
        text
    };
    request.url = inline(&request.url);
    request.body = inline(&request.body);
    for header in &mut request.headers {
        header.value = inline(&header.value);
    }
    Ok(request)
}

/// Replaces `{{name}}` (spaces inside the braces allowed) with `value`.
fn replace_variable(text: &str, name: &str, value: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(len) = rest[start + 2..].find("}}") else {
            break;
        };
        let inner = &rest[start + 2..start + 2 + len];
        out.push_str(&rest[..start]);
        match inner.trim() == name {
            true => out.push_str(value),
            false => out.push_str(&rest[start..start + 2 + len + 2]),
        }
        rest = &rest[start + 2 + len + 2..];
    }
    out.push_str(rest);
    out
}

fn item(root: &Path, id: &str, name: &str, request: &Request) -> Result<Value> {
    let mut headers = effective_headers(root, id, &request.headers)?;
    let shape = multipart::shape(&headers, &request.body);
    // Postman writes its own multipart Content-Type with a fresh boundary.
    if matches!(shape, BodyShape::Multipart(_)) {
        headers.retain(|h| !h.name.eq_ignore_ascii_case("content-type"));
    }
    let mut postman = json!({
        "method": request.method.to_uppercase(),
        "header": headers
            .iter()
            .map(|h| json!({ "key": h.name, "value": postman_text(&h.value) }))
            .collect::<Vec<_>>(),
        "url": { "raw": postman_text(&request.url) },
    });
    let folder = workspace::split_rel(workspace::parse_request_id(id)?.0).0;
    let file_body = match shape {
        BodyShape::File(path) => {
            Some(json!({ "mode": "file", "file": { "src": src(folder, &path) } }))
        }
        BodyShape::Multipart(parts) => Some(formdata(folder, &parts)),
        BodyShape::Text => body(request, &headers),
    };
    if let Some(body) = file_body {
        postman["body"] = body;
    }
    let (status_tests, other): (Vec<&String>, Vec<&String>) = request
        .assertions
        .iter()
        .partition(|a| status_code(a).is_some());
    let description = description(request, &other);
    if !description.is_empty() {
        postman["description"] = Value::String(description);
    }
    let mut item = json!({ "name": name, "request": postman });
    if !status_tests.is_empty() {
        item["event"] = status_test_event(&status_tests);
    }
    Ok(item)
}

/// The request's docs, plus the assertions Postman can't run as a list.
fn description(request: &Request, other: &[&String]) -> String {
    let description = request_meta::docs(request);
    if other.is_empty() {
        return description;
    }
    let list: Vec<String> = other.iter().map(|a| format!("- `{a}`")).collect();
    format!("{description}\n\nAssertions:\n{}", list.join("\n"))
        .trim()
        .to_string()
}

/// `status == 200` assertions as Postman test scripts.
fn status_test_event(status_tests: &[&String]) -> Value {
    let exec: Vec<String> = status_tests
        .iter()
        .filter_map(|a| status_code(a).map(|code| (a, code)))
        .map(|(a, code)| {
            format!(
                "pm.test({}, () => pm.response.to.have.status({code}));",
                Value::String(a.to_string())
            )
        })
        .collect();
    json!([{ "listen": "test", "script": { "type": "text/javascript", "exec": exec } }])
}

fn body(request: &Request, headers: &[Header]) -> Option<Value> {
    if request.body.is_empty() {
        return None;
    }
    let content_type = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.to_lowercase())
        .unwrap_or_default();
    if content_type.contains("x-www-form-urlencoded") {
        let fields: Vec<Value> = request
            .body
            .trim()
            .split('&')
            .map(|pair| {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                json!({ "key": key, "value": postman_text(value) })
            })
            .collect();
        return Some(json!({ "mode": "urlencoded", "urlencoded": fields }));
    }
    let json_body =
        content_type.contains("json") || request.body.trim_start().starts_with(['{', '[']);
    let language = if json_body { "json" } else { "text" };
    Some(json!({
        "mode": "raw",
        "raw": postman_text(&request.body),
        "options": { "raw": { "language": language } },
    }))
}

/// Multipart parts as Postman `formdata` rows: text fields, and file fields pointing at the file.
fn formdata(folder: &str, parts: &[multipart::Part]) -> Value {
    let rows: Vec<Value> = parts
        .iter()
        .map(|part| match &part.value {
            PartValue::Text(text) => json!({
                "key": part.name,
                "value": postman_text(text),
                "type": "text",
            }),
            PartValue::File(path) => json!({
                "key": part.name,
                "src": src(folder, path),
                "type": "file",
            }),
        })
        .collect();
    json!({ "mode": "formdata", "formdata": rows })
}

/// A body file path (relative to the `.http` file) as a path relative to the workspace root,
/// which is where Postman users keep the exported collection's files.
fn src(folder: &str, path: &str) -> String {
    let path = path.trim_start_matches("./");
    if Path::new(path).is_absolute() {
        return path.to_string();
    }
    workspace::join_rel(folder, path)
}

/// `status == 201` → 201.
fn status_code(assertion: &str) -> Option<u16> {
    let rest = assertion.trim().strip_prefix("status")?.trim_start();
    rest.strip_prefix("==")?.trim().parse().ok()
}

/// Our dynamic variables in Postman's spelling.
fn postman_text(text: &str) -> String {
    text.replace("{{$uuid}}", "{{$guid}}")
        .replace("{{$random.uuid}}", "{{$guid}}")
}
