//! Example values for OpenAPI / Swagger: resolves local `$ref`s and builds a small sample body
//! from a JSON schema when the spec has no example.

use serde_json::{Map, Value, json};

/// Nested objects deeper than this are left empty; real APIs rarely need more to be useful.
const MAX_DEPTH: usize = 6;

/// Follows a local `$ref` (`#/components/schemas/User`), or returns the value itself.
/// Returns `None` for refs that point outside the document or nowhere.
pub fn resolve<'a>(doc: &'a Value, value: &'a Value) -> Option<&'a Value> {
    let mut current = value;
    // A few hops at most; protects against refs pointing at each other.
    for _ in 0..8 {
        let Some(reference) = current.get("$ref").and_then(Value::as_str) else {
            return Some(current);
        };
        current = doc.pointer(reference.strip_prefix('#')?)?;
    }
    None
}

/// A sample value for `schema`: its example or default if present, otherwise a skeleton.
pub fn sample(doc: &Value, schema: &Value) -> Value {
    build(doc, schema, 0, &mut Vec::new())
}

fn build(doc: &Value, schema: &Value, depth: usize, seen: &mut Vec<String>) -> Value {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return build_ref(doc, schema, reference, depth, seen);
    }
    if let Some(given) = given_value(schema) {
        return given.clone();
    }
    if let Some(Value::Array(parts)) = schema.get("allOf") {
        let mut merged = Map::new();
        for part in parts {
            if let Value::Object(fields) = build(doc, part, depth + 1, seen) {
                merged.extend(fields);
            }
        }
        return Value::Object(merged);
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(first) = schema
            .get(key)
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        {
            return build(doc, first, depth + 1, seen);
        }
    }
    skeleton(doc, schema, depth, seen)
}

fn build_ref(
    doc: &Value,
    schema: &Value,
    reference: &str,
    depth: usize,
    seen: &mut Vec<String>,
) -> Value {
    // A ref already on the path means a recursive type: stop there.
    if seen.iter().any(|r| r == reference) || depth > MAX_DEPTH {
        return Value::Null;
    }
    let Some(target) = resolve(doc, schema) else {
        return Value::Null;
    };
    seen.push(reference.to_string());
    let value = build(doc, target, depth + 1, seen);
    seen.pop();
    value
}

/// The schema's own example, default, first example or first enum value.
fn given_value(schema: &Value) -> Option<&Value> {
    if let Some(example) = schema.get("example").or_else(|| schema.get("default")) {
        return Some(example);
    }
    ["examples", "enum"]
        .into_iter()
        .find_map(|key| schema.get(key)?.as_array()?.first())
}

fn skeleton(doc: &Value, schema: &Value, depth: usize, seen: &mut Vec<String>) -> Value {
    match schema_type(schema) {
        "object" => {
            let mut fields = Map::new();
            if depth <= MAX_DEPTH
                && let Some(Value::Object(properties)) = schema.get("properties")
            {
                for (name, property) in properties {
                    fields.insert(name.clone(), build(doc, property, depth + 1, seen));
                }
            }
            Value::Object(fields)
        }
        "array" => match schema.get("items") {
            Some(items) if depth <= MAX_DEPTH => json!([build(doc, items, depth + 1, seen)]),
            _ => json!([]),
        },
        "integer" | "number" => json!(0),
        "boolean" => json!(false),
        "string" => json!(string_sample(schema)),
        _ => Value::Null,
    }
}

/// `type` may be missing (objects with properties), a string, or a 3.1-style list.
fn schema_type(schema: &Value) -> &str {
    match schema.get("type") {
        Some(Value::String(kind)) => kind,
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null")
            .unwrap_or_default(),
        _ if schema.get("properties").is_some() => "object",
        _ => "",
    }
}

fn string_sample(schema: &Value) -> &'static str {
    match schema.get("format").and_then(Value::as_str) {
        Some("date-time") => "2026-01-01T00:00:00Z",
        Some("date") => "2026-01-01",
        Some("email") => "user@example.com",
        Some("uuid") => "00000000-0000-0000-0000-000000000000",
        Some("uri" | "url") => "https://example.com",
        _ => "string",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Value {
        json!({
            "components": { "schemas": {
                "User": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "email": { "type": "string", "format": "email" },
                        "role": { "type": "string", "enum": ["admin", "user"] },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "manager": { "$ref": "#/components/schemas/User" }
                    }
                },
                "Named": { "allOf": [
                    { "$ref": "#/components/schemas/User" },
                    { "properties": { "nick": { "type": ["string", "null"], "example": "ada" } } }
                ] }
            } }
        })
    }

    #[test]
    fn builds_skeleton_following_refs_and_stopping_recursion() {
        let doc = doc();
        let value = sample(&doc, &json!({ "$ref": "#/components/schemas/User" }));
        assert_eq!(
            value,
            json!({ "id": 0, "email": "user@example.com", "role": "admin", "tags": ["string"], "manager": null })
        );
    }

    #[test]
    fn merges_all_of_and_prefers_examples() {
        let doc = doc();
        let value = sample(&doc, &json!({ "$ref": "#/components/schemas/Named" }));
        assert_eq!(value["nick"], "ada");
        assert_eq!(value["id"], 0);
    }

    #[test]
    fn resolve_ignores_external_refs() {
        let doc = doc();
        assert!(resolve(&doc, &json!({ "$ref": "other.yaml#/User" })).is_none());
    }
}
