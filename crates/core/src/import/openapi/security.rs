use serde_json::Value;

use super::header;
use crate::http_file::Request;
use crate::import::{ImportFormat, text};

enum Placement {
    Header(&'static str, &'static str),
    /// Parameter name and location (`header` or `query`).
    ApiKey(String, String),
}

/// Adds the first security requirement as a header (or query parameter) whose value is a
/// secret variable, and records that secret's name for the environment.
pub(super) fn add_security(
    doc: &Value,
    format: ImportFormat,
    operation: &Value,
    request: &mut Request,
    secrets: &mut Vec<String>,
) {
    let Some(scheme) = first_scheme(doc, format, operation) else {
        return;
    };
    let (secret, placement) = placement(scheme);
    match placement {
        Placement::Header(name, prefix) => {
            request
                .headers
                .push(header(name, &format!("{prefix}{{{{{secret}}}}}")));
        }
        Placement::ApiKey(name, location) if location == "query" => {
            let separator = if request.url.contains('?') { '&' } else { '?' };
            request.url = format!("{}{separator}{name}={{{{{secret}}}}}", request.url);
        }
        Placement::ApiKey(name, _) => request
            .headers
            .push(header(&name, &format!("{{{{{secret}}}}}"))),
    }
    if !secrets.iter().any(|s| s == secret) {
        secrets.push(secret.to_string());
    }
}

fn first_scheme<'a>(doc: &'a Value, format: ImportFormat, operation: &Value) -> Option<&'a Value> {
    let requirement = operation
        .get("security")
        .or_else(|| doc.get("security"))
        .and_then(|s| s.get(0))
        .and_then(Value::as_object);
    let scheme_name = requirement.and_then(|r| r.keys().next())?;
    let schemes = match format {
        ImportFormat::Swagger => "/securityDefinitions",
        _ => "/components/securitySchemes",
    };
    doc.pointer(&format!("{schemes}/{scheme_name}"))
}

/// The secret's name and where its value goes.
fn placement(scheme: &Value) -> (&'static str, Placement) {
    match (
        text(scheme, "type").as_str(),
        text(scheme, "scheme").as_str(),
    ) {
        ("http", "basic") | ("basic", _) => {
            ("basicAuth", Placement::Header("Authorization", "Basic "))
        }
        ("apiKey", _) => (
            "apiKey",
            Placement::ApiKey(text(scheme, "name"), text(scheme, "in")),
        ),
        // Bearer, OAuth 2 and OpenID Connect all end up as a bearer token.
        _ => ("token", Placement::Header("Authorization", "Bearer ")),
    }
}
