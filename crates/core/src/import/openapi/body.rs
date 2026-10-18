use serde_json::Value;

use super::header;
use crate::http_file::Request;
use crate::import::schema::{resolve, sample};

/// OpenAPI 3 `requestBody`: JSON from an example or the schema, else form fields.
pub(super) fn set_body(doc: &Value, request: &mut Request, body: &Value) {
    let Some(content) = body.get("content").and_then(Value::as_object) else {
        return;
    };
    if let Some((_, media)) = content.iter().find(|(kind, _)| kind.contains("json")) {
        let example = media.get("example").cloned().or_else(|| {
            let first = media.get("examples")?.as_object()?.values().next()?;
            resolve(doc, first)?.get("value").cloned()
        });
        let value =
            example.unwrap_or_else(|| sample(doc, media.get("schema").unwrap_or(&Value::Null)));
        set_json_body(request, value);
        return;
    }
    if let Some(media) = content.get("application/x-www-form-urlencoded") {
        set_form_body(doc, request, media);
    }
}

/// Form fields from the schema's properties, with empty values.
fn set_form_body(doc: &Value, request: &mut Request, media: &Value) {
    let schema = media
        .get("schema")
        .and_then(|s| resolve(doc, s))
        .unwrap_or(&Value::Null);
    let fields: Vec<String> = schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, _)| format!("{name}="))
        .collect();
    request.body = fields.join("&");
    request
        .headers
        .push(header("Content-Type", "application/x-www-form-urlencoded"));
}

pub(super) fn set_json_body(request: &mut Request, value: Value) {
    if value.is_null() {
        return;
    }
    request.body = serde_json::to_string_pretty(&value).unwrap_or_default();
    request
        .headers
        .push(header("Content-Type", "application/json"));
}
